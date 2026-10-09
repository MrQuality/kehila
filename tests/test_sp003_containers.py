"""Probe error bookkeeping only; real Podman behavior uses check_probe.py."""
import importlib.util
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[1] / "experiments/SP-003"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


with patch.dict(sys.modules, {"build": load("sp003_build", SOURCE / "build.py")}):
    CONTAINERS = load("sp003_containers", SOURCE / "containers.py")


class ProbeErrorsTests(unittest.TestCase):
    def test_missing_ownership_and_malformed_inspection_return_diagnostics(self):
        for payload in (b'[{"Config":{"Labels":null}}]', b'[]', b'null', b'not-json'):
            with self.subTest(payload=payload):
                with patch.object(CONTAINERS.subprocess, "run",
                                  return_value=SimpleNamespace(returncode=0, stdout=payload)) as run:
                    self.assertTrue(CONTAINERS.cleanup("owned-name"))
                run.assert_called_once_with(["podman", "inspect", "owned-name"],
                                            capture_output=True, timeout=15)

    def test_primary_failure_survives_cleanup_failure(self):
        for failure in (subprocess.TimeoutExpired("podman", 1), OSError("launch"), RuntimeError("exit")):
            with self.subTest(error=type(failure).__name__):
                errors = []
                with patch.object(CONTAINERS, "command", side_effect=failure), \
                     patch.object(CONTAINERS, "cleanup", return_value=["removal failed"]) as cleanup:
                    with self.assertRaises(type(failure)) as raised:
                        CONTAINERS.run_probe("image", "owned-name", [], errors)
                self.assertIs(raised.exception, failure)
                self.assertEqual(errors, ["removal failed"])
                cleanup.assert_called_once_with("owned-name")

    def test_cleanup_failure_blocks_success(self):
        errors = []
        with patch.object(CONTAINERS, "command", return_value=b"output"), \
             patch.object(CONTAINERS, "cleanup", return_value=["ownership refused"]):
            with self.assertRaisesRegex(RuntimeError, "ownership refused"):
                CONTAINERS.run_probe("image", "owned-name", [], errors)
        self.assertEqual(errors, ["ownership refused"])
