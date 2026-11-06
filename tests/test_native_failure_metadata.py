"""Closed failure reporting must neither disclose inputs nor mask the failure."""
from contextlib import nullcontext, redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import native_authority_fixture as fixture


class FailureMetadataTests(unittest.TestCase):
    def test_real_journey_uses_optimized_profile_and_retains_exact_selection(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            (work / 'controls').mkdir()
            (work / 'input').write_bytes(b'a\nb\nc\nd\ne\n')

            def cargo(command, **kwargs):
                self.assertEqual(command, ['cargo', 'test', '--release', '--offline', '--locked',
                    '-p', 'kehila_authority', '--lib', fixture.SPECS['auth-unknown'][0],
                    '--', '--ignored', '--exact', '--nocapture'])
                kwargs['stdout'].write(b'test result: ok. 1 passed; 0 failed;\n')
                (work / 'controls/passed').touch()
                return SimpleNamespace(returncode=0)

            with patch.object(fixture, 'WORK', work), \
                    patch.object(fixture.subprocess, 'run', side_effect=cargo):
                fixture.journey(1, 'auth-unknown')

    def test_only_known_labels_and_public_source_locations_are_emitted(self):
        text = ('password=DO_NOT_EMIT bearer=DO_NOT_EMIT\n'
                'owned_job private_label elapsed_us=42\n'
                'owned_job login elapsed_us=123\n'
                'owned_job item_pending elapsed_us=321\n'
                'panicked at /private/DO_NOT_EMIT.rs:9:2\n'
                'panicked at src/coordinator/jobs/tests/unknown_commit.rs:95:13\n')
        result = fixture.failure_metadata('auth-unknown', 101, text, True, False)
        encoded = json.dumps(result)
        self.assertNotIn('DO_NOT_EMIT', encoded)
        self.assertNotIn('private_label', encoded)
        self.assertEqual(result['jobs'], [{'operation': 'login', 'elapsed_us': 123},
                                         {'operation': 'item_pending', 'elapsed_us': 321}])
        self.assertEqual(result['public_panic_location'], {
            'source': 'src/io/kehila_authority/src/coordinator/jobs/tests/unknown_commit.rs',
            'line': 95, 'column': 13})
        self.assertFalse(result['success_summary'])
        self.assertFalse(result['marker_present'])

    def test_unknown_mode_arbitrary_paths_and_message_are_not_echoed(self):
        result = fixture.failure_metadata('secret-mode', 1,
            'panicked at ../../private/secret.rs:2:4\nsecret assertion', False, False)
        self.assertEqual(result['mode'], 'unknown')
        self.assertIsNone(result['public_panic_location'])
        self.assertNotIn('secret', json.dumps(result))

    def test_operation_output_is_bounded_and_reports_truncation(self):
        result = fixture.failure_metadata('auth-unknown', 1,
            'owned_job login elapsed_us=2\n' * 100, True, False)
        self.assertEqual(len(result['jobs']), 64)
        self.assertTrue(result['jobs_truncated'])

    def test_source_enumeration_failure_is_not_silently_misclassified(self):
        with patch.object(Path, 'rglob', side_effect=OSError('private diagnostic')):
            with self.assertRaises(OSError):
                fixture.failure_metadata('auth-unknown', 1, '', True, False)

    def run_journey(self, exit_code, summary, marker, *, reporting_error=False,
                    stream_error=False, source_error=False, success=False):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            (work / 'controls').mkdir()
            (work / 'input').write_bytes(b'a\nb\nc\nd\ne\n')

            def cargo(*_args, **kwargs):
                kwargs['stdout'].write(summary.encode('utf-8'))
                if marker:
                    (work / 'controls/passed').touch()
                return SimpleNamespace(returncode=exit_code)

            output = io.StringIO()
            with patch.object(fixture, 'WORK', work), \
                    patch.object(fixture.subprocess, 'run', side_effect=cargo), \
                    redirect_stderr(output), redirect_stdout(io.StringIO()), \
                    (patch.object(Path, 'rglob', side_effect=OSError('DO_NOT_EMIT'))
                     if source_error else nullcontext()):
                with patch.object(fixture, 'failure_metadata', wraps=fixture.failure_metadata) as report:
                    if reporting_error:
                        report.side_effect = OSError('DO_NOT_EMIT')
                    if stream_error:
                        output.write = lambda *_args: (_ for _ in ()).throw(OSError('DO_NOT_EMIT'))
                    if success:
                        fixture.journey(1, 'auth-unknown')
                        report.assert_not_called()
                    else:
                        with self.assertRaises(RuntimeError) as failure:
                            fixture.journey(1, 'auth-unknown')
            if not success:
                self.assertEqual(str(failure.exception),
                    'Native authority journey failed; external diagnostics retained')
                self.assertTrue(failure.exception.__suppress_context__)
            return output.getvalue()

    def test_exit_summary_and_marker_failures_remain_distinguishable(self):
        good = 'test result: ok. 1 passed; 0 failed;\n'
        for code, summary, marker in ((101, good, True), (0, 'failed', True), (0, good, False)):
            with self.subTest(code=code, summary=summary, marker=marker):
                result = json.loads(self.run_journey(code, summary, marker))
                self.assertEqual(result['cargo_exit'], code)
                self.assertEqual(result['success_summary'], summary == good)
                self.assertEqual(result['marker_present'], marker)

    def test_metadata_error_preserves_primary_failure(self):
        self.assertEqual(self.run_journey(101, 'failed', False, reporting_error=True), '')

    def test_stderr_error_preserves_primary_failure(self):
        self.assertEqual(self.run_journey(101, 'failed', False, stream_error=True), '')

    def test_source_enumeration_error_preserves_primary_failure(self):
        self.assertEqual(self.run_journey(101, 'failed', False, source_error=True), '')

    def test_success_does_not_construct_or_emit_failure_metadata(self):
        self.assertEqual(self.run_journey(0, 'test result: ok. 1 passed; 0 failed;\n',
            True, success=True), '')


if __name__ == '__main__':
    unittest.main()
