"""Safety gates for the opt-in native database verifier."""
import copy
import importlib.util
import subprocess
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    'check_postgresql', Path(__file__).resolve().parents[1] / 'scripts/check_postgresql.py')
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class ContainerBoundaryTests(unittest.TestCase):
    def valid(self):
        return {
            'State': {'Running': True},
            'Config': {'Labels': {'purpose': 'kehila-postgresql-test'}, 'Image': check.IMAGE},
            'HostConfig': {'NetworkMode': 'none', 'PortBindings': {}, 'Privileged': False,
                           'Binds': [], 'Memory': 256 * 1024**2, 'NanoCpus': 10**9,
                           'PidsLimit': 128,
                           'Tmpfs': {'/var/lib/postgresql/data': 'rw,size=256m,nosuid,nodev'}},
            'Mounts': [],
        }

    def test_explicit_ephemeral_container_accepted(self):
        check.validate_container(self.valid())

    def test_unsafe_container_rejected_before_database_work(self):
        cases = [
            ('State', 'Running', False),
            ('Config', 'Labels', {}),
            ('Config', 'Image', 'postgres:latest'),
            ('HostConfig', 'NetworkMode', 'bridge'),
            ('HostConfig', 'PortBindings', {'5432/tcp': [{}]}),
            ('HostConfig', 'Privileged', True),
            ('HostConfig', 'Binds', ['/data:/var/lib/postgresql/data']),
            ('HostConfig', 'Memory', 0),
            ('HostConfig', 'Memory', 512 * 1024**2),
            ('HostConfig', 'NanoCpus', 0),
            ('HostConfig', 'NanoCpus', 2 * 10**9),
            ('HostConfig', 'PidsLimit', -1),
            ('HostConfig', 'PidsLimit', 256),
            ('HostConfig', 'Tmpfs', {}),
            ('HostConfig', 'Tmpfs', {'/var/lib/postgresql/data': 'rw,size=2g'}),
        ]
        for section, key, value in cases:
            with self.subTest(section=section, key=key, value=value):
                info = copy.deepcopy(self.valid())
                info[section][key] = value
                with self.assertRaises(ValueError):
                    check.validate_container(info)
        info = self.valid()
        info['Mounts'] = [{'Type': 'volume', 'Destination': '/var/lib/postgresql/data'}]
        with self.assertRaises(ValueError):
            check.validate_container(info)


class DatabaseRejectionTests(unittest.TestCase):
    def result(self, code, stderr='', stdout=''):
        return subprocess.CompletedProcess([], code, stdout, stderr)

    def invoke(self, completed, **expectation):
        database = check.DatabaseCheck(['podman'], 'kehila-42501')
        with patch.object(database, 'command', return_value=completed):
            return database.sql('SET ROLE kehila_owner;', **expectation)

    def test_accepts_actual_sqlstate_from_stdin_or_file(self):
        for prefix in ('', 'psql:/opt/kehila/install.sql:14: '):
            with self.subTest(prefix=prefix):
                self.invoke(self.result(3, prefix + 'ERROR:  42501: permission denied\n'),
                            expected_state='42501')

    def test_rejects_non_sql_exits_even_with_matching_diagnostic(self):
        for code in (0, 1, 2, 125, 126, 127):
            with self.subTest(code=code), self.assertRaises(ValueError):
                self.invoke(self.result(code, 'ERROR:  42501: permission denied\n'),
                            expected_state='42501')

    def test_container_name_cannot_substitute_for_sqlstate(self):
        with self.assertRaises(ValueError):
            self.invoke(self.result(125, 'Error: container kehila-42501 not found'),
                        expected_state='42501')

    def test_rejects_wrong_or_absent_sqlstate_despite_matching_text(self):
        for stderr in ('ERROR:  42601: syntax error near "42501"\n',
                       'ERROR:  permission denied in container kehila-42501\n',
                       'ERROR:  425010: malformed SQLSTATE\n'):
            with self.subTest(stderr=stderr), self.assertRaises(ValueError):
                self.invoke(self.result(3, stderr), expected_state='42501')

    def test_message_must_match_primary_error_not_context(self):
        self.invoke(self.result(3, 'ERROR:  P0001: fresh dedicated database required\n'),
                    expected_state='P0001', expected_message='fresh dedicated database')
        for stderr in ('ERROR:  P0001: another failure\nCONTEXT: fresh dedicated database\n',
                       'ERROR:  42601: fresh dedicated database\n'):
            with self.subTest(stderr=stderr), self.assertRaises(ValueError):
                self.invoke(self.result(3, stderr), expected_state='P0001',
                            expected_message='fresh dedicated database')

    def test_message_expectation_requires_sqlstate(self):
        with self.assertRaises(ValueError):
            self.invoke(self.result(3, 'ERROR:  P0001: failure\n'), expected_message='failure')

    def test_unexpected_failure_still_fails_and_success_returns_output(self):
        with self.assertRaises(ValueError):
            self.invoke(self.result(3, 'ERROR:  42501: permission denied\n'))
        self.assertEqual(self.invoke(self.result(0, stdout='query result')), 'query result')


if __name__ == '__main__':
    unittest.main()
