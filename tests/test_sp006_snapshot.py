"""Snapshot lifetime and durable-intent decisions; real SQL evidence is SP-006."""
import importlib.util
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('sp006_snapshot_protocol', ROOT / 'experiments/SP-006/protocol.py')
protocol = importlib.util.module_from_spec(spec)
with patch.object(sys, 'path', [str(ROOT / 'scripts'), *sys.path]), patch.dict(sys.modules):
    spec.loader.exec_module(protocol)


class SnapshotLifetimeTests(unittest.TestCase):
    def registered(self):
        state = {'identity': [17, '3/40', 'repeatable read', 'on'], 'commits': 0}

        def execute(statement):
            if "'row'" in statement:
                return json.dumps({'identity': state['identity'], 'row': None})
            if 'pg_locks' in statement:
                return json.dumps(state['identity'])
            if statement.startswith('COMMIT'):
                state['commits'] += 1
            return ''

        source = Mock(execute=Mock(side_effect=execute))
        search = Mock()
        rebuild = protocol.Rebuild(Mock(), Mock(execute=Mock(return_value='t')), search)
        rebuild.fenced = True
        rebuild.register('fixture_new', source)
        search.reset_mock()
        return rebuild, source, search, state

    def test_replacement_source_cannot_complete_registered_epoch(self):
        rebuild, source, search, state = self.registered()
        replacement = Mock(execute=Mock(return_value=''))
        with self.assertRaisesRegex(ValueError, 'snapshot source/epoch mismatch'):
            rebuild.snapshot('fixture_new', replacement)
        replacement.execute.assert_not_called()
        search.publish.assert_not_called()
        self.assertEqual(state['commits'], 0)

    def test_restarted_transaction_on_same_source_cannot_complete_even_an_empty_page(self):
        rebuild, source, search, state = self.registered()
        state['identity'] = [17, '3/41', 'repeatable read', 'on']
        with self.assertRaisesRegex(ValueError, 'registered snapshot transaction changed'):
            rebuild.snapshot('fixture_new', source)
        search.publish.assert_not_called()
        self.assertEqual(state['commits'], 0)

    def test_recovery_uses_persisted_target_and_requires_switching_phase(self):
        for phase in ('switching', 'building'):
            with self.subTest(phase=phase):
                persisted = {'phase': phase, 'next_target': 'fixture_durable'}
                session = Mock(execute=Mock(return_value=json.dumps(persisted)))
                rebuild = protocol.Rebuild(Mock(), session, Mock())
                rebuild.fenced = True
                rebuild.cutover = Mock()
                if phase == 'switching':
                    self.assertEqual(rebuild.recover(), 'fixture_durable')
                    rebuild.cutover.assert_called_once_with('fixture_durable')
                else:
                    with self.assertRaisesRegex(ValueError, 'no switching recovery intent'):
                        rebuild.recover()
                    rebuild.cutover.assert_not_called()


if __name__ == '__main__':
    unittest.main()
