"""Mocked adapter/control-flow regressions; live SP-006 supplies I/O evidence."""
import email.message
import importlib.util
import io
from pathlib import Path
import sys
import unittest
import urllib.request
import urllib.response
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('sp006_protocol', ROOT / 'experiments/SP-006/protocol.py')
protocol = importlib.util.module_from_spec(spec)
with patch.object(sys, 'path', [str(ROOT / 'scripts'), *sys.path]), patch.dict(sys.modules):
    spec.loader.exec_module(protocol)


class SearchBoundaryTests(unittest.TestCase):
    def test_publish_classifies_applied_superseded_and_exact_duplicates(self):
        body = protocol.document('a', 2, 'value')
        payload = dict(body, payload_sha256=protocol.digest(body))
        cases = [(200, None, 'applied'), (201, None, 'applied'),
                 (409, {'_version': 3}, 'superseded'),
                 (409, {'_version': 2, '_source': payload}, 'duplicate')]
        for status, current, expected in cases:
            with self.subTest(status=status, current=current):
                search = protocol.Search('fixture')
                search.request = Mock(return_value=(status, {}))
                search.read = Mock(return_value=current)
                self.assertEqual(search.publish('fixture_new', body), expected)
                if status != 409:
                    search.read.assert_not_called()

    def test_publish_refuses_conflicting_content_lower_versions_and_failed_delivery(self):
        body = protocol.document('a', 2, 'value')
        for status, current, message in [(409, {'_version': 2, '_source': {}}, 'same-version content mismatch'),
                                         (409, {'_version': 1}, 'unexpected version conflict'),
                                         (503, None, 'leave pending')]:
            with self.subTest(status=status, current=current):
                search = protocol.Search('fixture')
                search.request = Mock(return_value=(status, {}))
                search.read = Mock(return_value=current)
                with self.assertRaisesRegex(ValueError, message):
                    search.publish('fixture_new', body)

    def test_request_accepts_exact_byte_limit_and_rejects_overflow_before_json_decode(self):
        exact = b'{"value":"' + b'x' * (65536 - 12) + b'"}'
        self.assertEqual(len(exact), 65536)
        for raw, over_limit in ((exact, False), (b'x' * 65537, True)):
            with self.subTest(over_limit=over_limit):
                search = protocol.Search('fixture')
                response = urllib.response.addinfourl(io.BytesIO(raw), {}, 'http://127.0.0.1:9200/', 200)
                with patch.object(search.client, 'open', return_value=response):
                    if over_limit:
                        with self.assertRaisesRegex(ValueError, 'oversized search response'):
                            search.request('GET', '/')
                    else:
                        status, result = search.request('GET', '/')
                        self.assertEqual(status, 200)
                        self.assertEqual(result, {'value': 'x' * (65536 - 12)})
                self.assertTrue(response.closed)

    def test_real_opener_rejects_redirect_without_a_second_request(self):
        requests = []

        class FixtureHTTP(urllib.request.BaseHandler):
            handler_order = 100

            def http_open(self, request):
                requests.append(request.full_url)
                headers = email.message.Message()
                headers['Location'] = 'http://outside.invalid/next'
                status = 302 if len(requests) == 1 else 200
                response = urllib.response.addinfourl(io.BytesIO(b'{}'), headers, request.full_url, status)
                response.msg = 'fixture'
                return response

        search = protocol.Search('fixture')
        search.client.add_handler(FixtureHTTP())  # No socket; keep the actual redirect policy.
        self.assertEqual(search.request('GET', '/'), (302, {}))
        self.assertEqual(requests, ['http://127.0.0.1:9200/'])


class CutoverBoundaryTests(unittest.TestCase):
    def coordinator(self, complete='t'):
        def execute(statement):
            if statement.startswith('SELECT snapshot_complete'):
                return complete
            if statement == 'SELECT active FROM sp006.control;':
                return 'fixture_old'
            return 't'
        return Mock(execute=Mock(side_effect=execute))

    def test_unfenced_and_incomplete_cutover_perform_no_search_work(self):
        for fenced, complete in ((False, 't'), (True, 'f')):
            with self.subTest(fenced=fenced, complete=complete):
                search = Mock()
                rebuild = protocol.Rebuild(Mock(), self.coordinator(complete), search)
                rebuild.fenced = fenced
                rebuild.drain = Mock()
                rebuild.verify_documents = Mock()
                with self.assertRaisesRegex(ValueError, 'requires fence|incomplete or lost snapshot'):
                    rebuild.cutover('fixture_new')
                self.assertEqual(search.mock_calls, [])
                rebuild.drain.assert_not_called()
                rebuild.verify_documents.assert_not_called()

    def test_normal_and_already_moved_alias_both_drain_verify_and_refresh(self):
        for actual in ('fixture_old', 'fixture_new'):
            with self.subTest(actual=actual):
                events = []
                search = Mock()
                search.alias_target.side_effect = [actual, 'fixture_new']
                search.refresh.side_effect = lambda target: events.append('refresh')
                search.switch.side_effect = lambda old, new: events.append('switch')
                rebuild = protocol.Rebuild(Mock(), self.coordinator(), search)
                rebuild.fenced = True
                rebuild.drain = Mock(side_effect=lambda target: events.append('drain'))
                rebuild.verify_documents = Mock(side_effect=lambda target: events.append('verify'))
                rebuild.cutover('fixture_new')
                self.assertEqual(events, ['drain', 'verify', 'refresh'] +
                                 (['switch'] if actual == 'fixture_old' else []))
                if actual == 'fixture_new':
                    search.switch.assert_not_called()
                else:
                    search.switch.assert_called_once_with('fixture_old', 'fixture_new')
                self.assertFalse(rebuild.fenced)


if __name__ == '__main__':
    unittest.main()
