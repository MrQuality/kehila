"""False-success controls for the SP-005 psql protocol, without an engine."""
import asyncio
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import sys
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('sp005_concurrency', ROOT / 'experiments/SP-005/concurrency.py')
concurrency = importlib.util.module_from_spec(spec)
spec.loader.exec_module(concurrency)


class ProtocolTests(unittest.IsolatedAsyncioTestCase):
    def session(self, output):
        session = concurrency.Session(SimpleNamespace(deadline=time.monotonic() + 10))
        reader = asyncio.StreamReader(limit=4096)
        reader.feed_data(output.encode('utf-8'))
        reader.feed_eof()
        session.process = SimpleNamespace(stdout=reader)
        session.pending = 'SP005_DONE_test'
        return session

    async def test_exact_primary_state_and_constraint_required(self):
        error = ('ERROR: 23505: duplicate key value violates unique constraint "projects_pkey"\n'
                 'SCHEMA NAME: kehila\nTABLE NAME: projects\nCONSTRAINT NAME: projects_pkey\n')
        await self.session(error + 'SP005_DONE_test\n').finish(('23505', 'projects_pkey'))
        for changed in (error.replace('23505', '25P02'), error.replace('projects_pkey', 'other'),
                        'CONTEXT: ' + error, error + error,
                        error.replace('CONSTRAINT NAME: projects_pkey\n', ''),
                        error.replace('CONSTRAINT NAME: projects_pkey', 'CONSTRAINT NAME: wrong'),
                        error.replace('TABLE NAME: projects', 'TABLE NAME: operations')):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                await self.session(changed + 'SP005_DONE_test\n').finish(('23505', 'projects_pkey'))

    async def test_unexpected_error_missing_marker_and_oversized_line_fail(self):
        for output in ('ERROR: 42501: permission denied\nSP005_DONE_test\n',
                       'a result without acknowledgement\n', 'x' * 5000 + '\nSP005_DONE_test\n'):
            with self.subTest(output=output[:40]), self.assertRaises(ValueError):
                await self.session(output).finish()

    async def test_result_must_reach_exact_marker(self):
        result = await self.session('7\nSP005_DONE_test_extra\nSP005_DONE_test\n').finish()
        self.assertEqual(result, ['7', 'SP005_DONE_test_extra'])

    async def test_late_diagnostics_cannot_repair_a_completed_empty_packet(self):
        delayed = ('SP005_DONE_test\nERROR: 23505: duplicate key\n'
                   'SCHEMA NAME: kehila\nTABLE NAME: projects\n'
                   'CONSTRAINT NAME: projects_pkey\n')
        with self.assertRaisesRegex(ValueError, 'primary uniqueness rejection'):
            await self.session(delayed).finish(('23505', 'projects_pkey'))

    async def test_capture_limit_is_cumulative_across_commands(self):
        session = self.session('ok\nSP005_DONE_test\n')
        session.output_bytes = 1024**2 - 1
        with self.assertRaisesRegex(ValueError, 'diagnostic limit'):
            await session.finish()

    async def test_cleanup_drains_large_output_without_hiding_failure(self):
        session = concurrency.Session(SimpleNamespace(deadline=time.monotonic() + 10))
        session.process = await asyncio.create_subprocess_exec(
            sys.executable, '-c', 'import sys; sys.stdout.write("x" * (2*1024**2)); sys.stdout.flush()',
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT, limit=4096)
        primary = ValueError('original protocol failure')
        await session.__aexit__(ValueError, primary, None)
        self.assertIsNotNone(session.process.returncode)
        self.assertTrue(session.output_overflow)
        self.assertEqual(str(primary), 'original protocol failure')

    def test_case_sql_values_are_closed(self):
        with self.assertRaises(ValueError):
            concurrency.create('actor', "target'; SELECT 1; --", 'token', 'a' * 32)

    def test_observer_disables_psql_format_feedback(self):
        class Database:
            def sql(self, statement):
                feedback = '' if statement.startswith('\\set QUIET on\n') else (
                    'Output format is unaligned.\n')
                return 'SET\nSET\nSET\n' + feedback + 't\n'
        self.assertEqual(concurrency.observe(Database(), 'SELECT true;'), ['t'])
