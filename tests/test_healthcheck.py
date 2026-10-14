"""Selective readiness retains every requested TCP/HTTP requirement."""
import importlib.util
import io
from pathlib import Path
import sys
import unittest
from unittest.mock import MagicMock, patch

spec = importlib.util.spec_from_file_location(
    'healthcheck', Path(__file__).resolve().parents[1] / 'scripts/healthcheck.py')
health = importlib.util.module_from_spec(spec)
spec.loader.exec_module(health)


class ReadinessTests(unittest.TestCase):
    def test_nats_only_never_contacts_other_services(self):
        with patch.object(health.socket, 'create_connection') as connect, \
                patch.object(health.urllib.request, 'build_opener') as http:
            self.assertEqual(health.probe('127.0.0.1', ['NATS']), [])
        connect.assert_called_once_with(('127.0.0.1', 4222), timeout=2)
        http.assert_not_called()

    def test_database_phase_checks_both_ports_and_propagates_failure(self):
        with patch.object(health.socket, 'create_connection', side_effect=OSError('closed')) as connect:
            errors = health.probe('127.0.0.1', ['PostgreSQL', 'FerretDB'])
        self.assertEqual(connect.call_count, 2)
        self.assertEqual(len(errors), 2)

    def test_search_still_requires_http_health_after_tcp_success(self):
        response = MagicMock()
        response.__enter__.return_value = io.BytesIO(b'{"status":"red"}')
        with patch.object(health.socket, 'create_connection'), \
                patch.object(health.urllib.request, 'build_opener') as opener:
            opener.return_value.open.return_value = response
            self.assertEqual(health.probe('127.0.0.1', ['OpenSearch']),
                             ['OpenSearch cluster not ready: red'])

    def test_default_scope_retains_all_four_services(self):
        with patch.object(sys, 'argv', ['healthcheck.py']), \
                patch.object(health, 'probe', return_value=[]) as probe:
            self.assertEqual(health.main(), 0)
        probe.assert_called_once_with('127.0.0.1', list(health.PORTS))

    def test_selected_readiness_failure_is_nonzero(self):
        with patch.object(sys, 'argv', ['healthcheck.py', '--services', 'NATS']), \
                patch.object(health, 'probe', return_value=['NATS closed']):
            self.assertEqual(health.main(), 1)
