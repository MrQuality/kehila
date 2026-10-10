"""Owned build-directory lifetimes; compiler invocations are test doubles."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[1] / "experiments/SP-003"
SPEC = importlib.util.spec_from_file_location("sp003_build_lifecycle", SOURCE / "build.py")
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class BuildLifecycleTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.dependencies = Path(temporary.name) / "deps"
        self.dependencies.mkdir()
        artifacts = []
        for name in ("task_contract", "serde_json", "sha2"):
            library = self.dependencies / (name + ".rlib")
            library.touch()
            artifacts.append(json.dumps({"reason": "compiler-artifact", "target": {"name": name},
                                         "filenames": [str(library)]}))
        self.artifacts = "\n".join(artifacts).encode()
        self.failure = None
        self.commands = []
        self.actual_command = BUILD.command
        patcher = patch.object(BUILD, "command", side_effect=self.command)
        patcher.start()
        self.addCleanup(patcher.stop)

    def command(self, args, **kwargs):
        self.commands.append(args)
        if args[0] == "cargo":
            return self.artifacts
        if args[0] == "rustfmt":
            stage = "format"
        elif args[0] == "rustc":
            Path(args[args.index("-o") + 1]).write_bytes(b"partial executable")
            stage = "compile-tests" if "--test" in args else "compile-oracle"
        else:
            stage = "run-tests"
        if self.failure == stage:
            raise RuntimeError(stage)
        return b"8 tests passed\n"

    def assert_no_builds(self):
        directory = self.dependencies / "sp003"
        self.assertFalse(directory.exists() and any(directory.iterdir()))
        self.assertTrue((self.dependencies / "task_contract.rlib").exists())

    def test_binary_lives_until_consumer_finishes(self):
        with BUILD.build() as (binary, output):
            self.assertTrue(binary.is_file())
            self.assertIn("8 tests passed", output)
        self.assertFalse(binary.parent.exists())
        self.assert_no_builds()

    def test_consumer_failure_cleans_up_and_preserves_error(self):
        failure = RuntimeError("consumer failed")
        with self.assertRaises(RuntimeError) as raised:
            with BUILD.build() as (binary, _):
                self.assertTrue(binary.exists())
                raise failure
        self.assertIs(raised.exception, failure)
        self.assert_no_builds()

    def test_partial_compilation_and_test_failure_clean_up(self):
        for stage in ("compile-tests", "run-tests", "compile-oracle"):
            with self.subTest(stage=stage):
                self.failure = stage
                with self.assertRaisesRegex(RuntimeError, stage):
                    with BUILD.build():
                        self.fail("Failed build must not yield an oracle")
                self.assert_no_builds()

    def test_overlapping_owners_do_not_remove_each_other(self):
        with BUILD.build() as (first, _):
            with BUILD.build() as (second, _):
                self.assertNotEqual(first.parent, second.parent)
                self.assertTrue(first.exists() and second.exists())
            self.assertTrue(first.exists())
            self.assertFalse(second.parent.exists())
        self.assert_no_builds()

    def test_windows_sharing_violations_retry_cleanup(self):
        for code in (32, 33):
            with self.subTest(code=code):
                error = PermissionError("temporary lock")
                error.winerror = code
                with patch.object(BUILD.sys, "platform", "win32"), \
                     patch.object(BUILD.shutil, "rmtree", side_effect=[error, None]) as remove, \
                     patch.object(BUILD.time, "sleep") as sleep:
                    BUILD.remove_build_directory(self.dependencies / "owned")
                self.assertEqual(remove.call_count, 2)
                sleep.assert_called_once_with(0.1)

    def test_cleanup_deadline_and_other_errors_fail_visibly(self):
        for code, times in ((32, [0, 6]), (5, [0])):
            with self.subTest(code=code):
                error = PermissionError("cleanup failed")
                error.winerror = code
                with patch.object(BUILD.sys, "platform", "win32"), \
                     patch.object(BUILD.shutil, "rmtree", side_effect=error), \
                     patch.object(BUILD.time, "monotonic", side_effect=times), \
                     patch.object(BUILD.time, "sleep") as sleep:
                    with self.assertRaises(PermissionError) as raised:
                        BUILD.remove_build_directory(self.dependencies / "owned")
                self.assertIs(raised.exception, error)
                sleep.assert_not_called()

    def test_format_failure_prevents_compilation(self):
        self.failure = "format"
        with self.assertRaisesRegex(RuntimeError, "format"):
            with BUILD.build():
                self.fail("Formatting failure must prevent compilation")
        self.assertEqual(self.commands, [["rustfmt", "--edition", "2021", "--check",
                                         str(SOURCE / "main.rs"), str(SOURCE / "codec.rs"),
                                         str(SOURCE / "tests.rs")]])
        self.assert_no_builds()

    def test_command_failure_retains_stdout_diagnostics(self):
        completed = subprocess.CompletedProcess([], 1, b"format diff", b"")
        with patch.object(BUILD.subprocess, "run", return_value=completed):
            # Bypass this class's build-command double to test real error reporting.
            with self.assertRaisesRegex(RuntimeError, "format diff"):
                self.actual_command(["rustfmt"])
