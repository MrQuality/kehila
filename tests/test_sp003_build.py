"""Owned build-directory lifetimes; compiler invocations are test doubles."""
import importlib.util
from contextlib import contextmanager
import json
from pathlib import Path
import subprocess
import tempfile
from unittest.mock import patch
from matrix_fixtures import IsolatedEnvironmentTests


SOURCE = Path(__file__).resolve().parents[1] / "experiments/SP-003"
SPEC = importlib.util.spec_from_file_location("sp003_build_lifecycle", SOURCE / "build.py")
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)
import temp_work


class BuildLifecycleTests(IsolatedEnvironmentTests):
    def setUp(self):
        super().setUp()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.dependencies = Path(temporary.name) / "deps"
        self.dependencies.mkdir()
        self.outputs = Path(temporary.name) / 'outputs'
        self.outputs.mkdir()
        allocate = BUILD.owned_directory

        @contextmanager
        def owned_output(*args, **kwargs):
            with allocate(*args, parent=self.outputs, **kwargs) as root:
                yield root

        output_patch = patch.object(BUILD, 'owned_directory', side_effect=owned_output)
        output_patch.start()
        self.addCleanup(output_patch.stop)
        producer = 'path+file:///source/codec#project_create_codec@0.1.0'
        dependencies = []
        packages = [{'id': producer, 'name': 'project_create_codec',
                     'manifest_path': str(SOURCE.parents[1] / 'src/pure/project_create_codec/Cargo.toml')}]
        artifacts = []
        for name in ("task_contract", "serde_json", "sha2"):
            identity = 'registry+test#' + name + '@1.0.0'
            dependencies.append({'name': name, 'pkg': identity})
            packages.append({'id': identity, 'name': name})
            library = self.dependencies / (name + ".rlib")
            library.touch()
            artifacts.append(json.dumps({"reason": "compiler-artifact", "package_id": identity,
                                         "target": {"name": name, "kind": ["lib"]},
                                         "filenames": [str(library)]}))
        self.metadata = json.dumps({'packages': packages, 'resolve': {'nodes': [
            {'id': producer, 'deps': dependencies},
            *({'id': edge['pkg'], 'deps': []} for edge in dependencies)]}}).encode()
        self.artifacts = "\n".join(artifacts).encode()
        self.failure = None
        self.host = b'host: x86_64-unknown-linux-gnu\n'
        self.commands = []
        self.actual_command = BUILD.command
        patcher = patch.object(BUILD, "command", side_effect=self.command)
        patcher.start()
        self.addCleanup(patcher.stop)

    def command(self, args, **kwargs):
        self.commands.append(args)
        if args == ['rustc', '-vV']:
            return self.host
        if args[0] == "cargo":
            return self.metadata if args[1] == 'metadata' else self.artifacts
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
        self.assertEqual(list(self.outputs.iterdir()), [])
        self.assertTrue((self.dependencies / "task_contract.rlib").exists())

    def test_binary_lives_until_consumer_finishes(self):
        with BUILD.build() as (binary, output):
            self.assertTrue(binary.is_file())
            self.assertNotIn(self.dependencies, binary.parents)
            self.assertIn("8 tests passed", output)
        self.assertFalse(binary.parent.exists())
        self.assert_no_builds()

    def test_split_library_parents_reach_both_compiler_invocations(self):
        records = [json.loads(line) for line in self.artifacts.splitlines()]
        for index, record in enumerate(records):
            record['filenames'] = [str(self.dependencies / str(index) / 'library.rlib')]
        self.artifacts = '\n'.join(json.dumps(record) for record in records).encode()
        with BUILD.build() as (binary, _):
            self.assertNotIn(self.dependencies, binary.parents)
        compilers = [args for args in self.commands if args[0] == 'rustc' and '-o' in args]
        self.assertEqual(len(compilers), 2)
        for args in compilers:
            self.assertEqual([args[index + 1] for index, value in enumerate(args) if value == '-L'],
                             ['dependency=' + str(self.dependencies / str(index)) for index in range(3)])
            self.assertEqual([args[index + 1] for index, value in enumerate(args) if value == '--extern'],
                             [record['target']['name'] + '=' + record['filenames'][0] for record in records])

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
        for code in (5, 32, 33):
            with self.subTest(code=code):
                error = PermissionError("temporary lock")
                error.winerror = code
                with patch.object(temp_work.sys, "platform", "win32"), \
                     patch.object(temp_work.shutil, "rmtree", side_effect=[error, None]) as remove, \
                     patch.object(temp_work.time, "sleep") as sleep:
                    with BUILD.build():
                        pass
                self.assertEqual(remove.call_count, 2)
                sleep.assert_called_once_with(0.25)

    def test_cleanup_deadline_and_other_errors_fail_visibly(self):
        for code, times in ((32, [0, 5]), (87, [0])):
            with self.subTest(code=code):
                error = PermissionError("cleanup failed")
                error.winerror = code
                with patch.object(temp_work.sys, "platform", "win32"), \
                     patch.object(temp_work.shutil, "rmtree", side_effect=error), \
                     patch.object(temp_work.time, "monotonic", side_effect=times), \
                     patch.object(temp_work.time, "sleep") as sleep:
                    with self.assertRaises(PermissionError) as raised:
                        with BUILD.build():
                            pass
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

    def test_missing_or_ambiguous_host_prevents_cargo_resolution(self):
        for host in (b'', b'host: \n', b'host: first\nhost: second\n'):
            with self.subTest(host=host):
                self.host = host
                self.commands.clear()
                with self.assertRaisesRegex(RuntimeError, 'native host target'):
                    with BUILD.build():
                        self.fail('Unknown host admitted dependency resolution')
                self.assertFalse(any(args[0] == 'cargo' for args in self.commands))
                self.assert_no_builds()

    def test_command_failure_retains_stdout_diagnostics(self):
        completed = subprocess.CompletedProcess([], 1, b"format diff", b"")
        with patch.object(BUILD.subprocess, "run", return_value=completed):
            # Bypass this class's build-command double to test real error reporting.
            with self.assertRaisesRegex(RuntimeError, "format diff"):
                self.actual_command(["rustfmt"])
