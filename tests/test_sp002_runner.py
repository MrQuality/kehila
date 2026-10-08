"""Regression checks for experiment orchestration; no database claims."""

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "experiments/SP-002"
SPEC = importlib.util.spec_from_file_location("sp002_manage", SOURCE / "manage.py")
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class Sp002RunnerTests(unittest.TestCase):
    def test_artifact_directory_and_secret_creation_are_restricted(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "new-run"
            RUNNER.create_artifact_directory(root)
            secret = root / "synthetic.txt"
            RUNNER.write_secret(secret, "test-only\n")
            self.assertEqual(secret.read_bytes(), b"test-only\n")
            if os.name == "nt":
                environment = os.environ.copy()
                environment["KEHILA_TEST_SECRET"] = str(secret)
                result = subprocess.run(
                    ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", """
$ErrorActionPreference = 'Stop'
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User
$rules = @((Get-Acl -LiteralPath $env:KEHILA_TEST_SECRET).GetAccessRules(
    $true, $true, [System.Security.Principal.SecurityIdentifier]))
if ($rules.Count -ne 1 -or $rules[0].IdentityReference -ne $sid -or
    $rules[0].AccessControlType -ne 'Allow' -or -not $rules[0].IsInherited) { exit 1 }
"""],
                    env=environment, capture_output=True, timeout=15,
                )
                self.assertEqual(result.returncode, 0, "Secret must inherit only current-user access")
            else:
                self.assertEqual(root.stat().st_mode & 0o777, 0o700)
                self.assertEqual(secret.stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError):
                RUNNER.write_secret(secret, "replacement")
            with self.assertRaises(FileExistsError):
                RUNNER.create_artifact_directory(root)

    def test_unsupported_platform_fails_explicitly(self):
        with patch.object(RUNNER.sys, "platform", "darwin"):
            with self.assertRaisesRegex(RuntimeError, "Windows and Linux"):
                RUNNER.validate_runtime()

    def test_optimized_execution_rejected_before_setup(self):
        result = subprocess.run(
            [sys.executable, "-O", str(SOURCE / "manage.py"), "--help"],
            capture_output=True,
            text=True,
            timeout=15,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("optimized Python", result.stderr)

    def test_cargo_artifact_uses_reported_path_not_target_convention(self):
        message = dict(
            reason="compiler-artifact",
            target=dict(name="task_contract", kind=["lib"]),
            filenames=["custom-build/debug/deps/libtask_contract-exact.rlib"],
        )
        unrelated = dict(
            reason="compiler-artifact",
            target=dict(name="other", kind=["lib"]),
            filenames=["target/debug/deps/libother.rlib"],
        )
        data = "\n".join(json.dumps(x) for x in [unrelated, message]).encode()
        self.assertEqual(RUNNER.cargo_library(data), Path(message["filenames"][0]))

    def test_cargo_artifact_ambiguity_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "exactly one"):
            RUNNER.cargo_library(b"")
        record = dict(
            reason="compiler-artifact",
            target=dict(name="task_contract", kind=["lib"]),
            filenames=[
                "build/libtask_contract-one.rlib",
                "build/libtask_contract-two.rlib",
            ],
        )
        with self.assertRaisesRegex(RuntimeError, "exactly one"):
            RUNNER.cargo_library(json.dumps(record).encode())

    def test_missing_baseline_has_actionable_error(self):
        with patch.object(RUNNER, "command", side_effect=RuntimeError("bad object")):
            with self.assertRaisesRegex(RuntimeError, "fetch.*baseline"):
                RUNNER.validate_baseline()

    def test_phase_timeout_retains_both_output_streams(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            error = subprocess.TimeoutExpired(
                ["python", "case.py"],
                300,
                output=b"case reached\n",
                stderr=b"diagnostic\n",
            )
            with patch.object(RUNNER.subprocess, "run", side_effect=error):
                with self.assertRaises(subprocess.TimeoutExpired):
                    RUNNER.run_phase(root, "case.py")
            self.assertEqual(
                (root / "case.py.log").read_bytes(), b"case reached\ndiagnostic\n"
            )

    def test_setup_deadline_terminates_a_stalled_worker(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "stalled.py").write_text(
                "import time; print('setup started', flush=True); time.sleep(120)",
                encoding="utf-8",
            )
            with self.assertRaises(subprocess.TimeoutExpired):
                RUNNER.run_phase(root, "stalled.py", timeout=1)
            self.assertIn(b"setup started", (root / "stalled.py.log").read_bytes())

    def test_cleanup_stop_failure_recorded_and_bounded(self):
        inspect = subprocess.CompletedProcess(
            [],
            0,
            stdout=json.dumps(
                [dict(Config=dict(Labels=dict(purpose="kehila-sp002")))]
            ).encode(),
            stderr=b"",
        )
        stop = subprocess.CompletedProcess([], 125, stdout=b"", stderr=b"stop failed")
        with tempfile.TemporaryDirectory() as temp:
            with patch.object(
                RUNNER.subprocess, "run", side_effect=[inspect, stop]
            ) as run:
                errors = RUNNER.cleanup_resources(Path(temp), ["owned-fixture"])
            self.assertTrue(errors)
            self.assertIn("owned-fixture", errors[0])
            self.assertTrue(
                all(call.kwargs.get("timeout") for call in run.call_args_list)
            )
            self.assertIn("stop failed", (Path(temp) / "cleanup.json").read_text())

    def test_imports_have_no_runtime_fixture_or_case_side_effects(self):
        # Stub only the optional driver's import surface. No storage is simulated.
        code = """
import sys, types
sys.path.insert(0, sys.argv[1])
sys.modules['psycopg'] = types.ModuleType('psycopg')
sys.modules['psycopg.types'] = types.ModuleType('psycopg.types')
module = types.ModuleType('psycopg.types.json')
module.Jsonb = object
sys.modules[module.__name__] = module
import common, schema_cases, protocol_cases, recovery_cases
import additional_cases, concurrent_workload, maximum_key
import payload_guard_cases
assert common.CONF == {}
assert common.RESULTS == []
try:
    common.conn()
except RuntimeError as error:
    if 'initialize' not in str(error):
        raise
else:
    raise RuntimeError('Uninitialized connection allowed')
"""
        result = subprocess.run(
            [sys.executable, "-c", code, str(SOURCE)],
            capture_output=True,
            text=True,
            timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_invariants_and_lazy_diagnostics_remain_active_under_optimization(self):
        code = """
import sys, types
sys.path.insert(0, sys.argv[1])
sys.modules['psycopg'] = types.ModuleType('psycopg')
sys.modules['psycopg.types'] = types.ModuleType('psycopg.types')
module = types.ModuleType('psycopg.types.json')
module.Jsonb = object
sys.modules[module.__name__] = module
import common
steps = []
common.require(steps.append('action') is None, lambda: steps.append('diagnostic'))
if steps != ['action']:
    raise RuntimeError('Action skipped or diagnostic evaluated on success')
try:
    common.require(False, lambda: 'failure detail')
except AssertionError as error:
    if str(error) != 'failure detail':
        raise RuntimeError('Failure diagnostic lost')
else:
    raise RuntimeError('Invariant disabled')
try:
    common.initialize()
except RuntimeError as error:
    if 'optimized Python' not in str(error):
        raise
else:
    raise RuntimeError('Optimized case initialization allowed')
"""
        result = subprocess.run(
            [sys.executable, "-O", "-c", code, str(SOURCE)],
            capture_output=True,
            text=True,
            timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_proposed_guard_matches_executable_regression(self):
        guard = (
            (SOURCE / "payload-identity-guard.sql").read_text(encoding="utf-8").strip()
        )
        proposal = (ROOT / "docs/architecture/M1-project-create-storage.md").read_text(
            encoding="utf-8"
        )
        self.assertIn(guard, proposal)


if __name__ == "__main__":
    unittest.main()
