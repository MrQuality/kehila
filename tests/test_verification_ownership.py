"""Write-ahead service ownership survives loss of the lifecycle process."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch
from retired_bindings import RETIRED

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import verification_ownership as ownership


EMPTY = {'containers': {}, 'networks': {}, 'volumes': {}}
CID, NID = 'b' * 64, 'c' * 64


class ReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'source'
        self.source.mkdir()
        (self.source / 'docker-compose.yml').write_text('services: {}', encoding='utf-8')
        self.path = self.root / 'ownership.json'
        self.runtime = Mock(engine=['podman', '--connection', 'fixture-root'], environment={})

    def create(self):
        receipt = ownership.ServiceOwnership.create(self.path, self.runtime, self.source,
                                                    'a' * 32, [('opensearch',)])
        project = receipt.group(['opensearch'], self.source / 'docker-compose.yml').project
        self.live = {'containers': {CID: {'service': 'opensearch', 'name': project + '_opensearch_1'}},
                     'networks': {NID: {'name': project + '_default'}}, 'volumes': {}}
        return receipt

    def reopen(self):
        return ownership.ServiceOwnership.open(self.path, self.runtime, self.source,
                                               'a' * 32, self.receipt.compose_digest)

    def test_reserved_intent_is_not_deletion_authority(self):
        self.receipt = self.create()
        resources = Mock()
        self.receipt.recover(resources)
        resources.assert_not_called()
        self.assertEqual(resources.mock_calls, [])
        self.assertEqual(json.loads(self.path.read_text())['groups'][0]['state'], 'reserved')

    def test_retired_service_scope_is_refused_without_rewriting_or_recovery(self):
        self.receipt = self.create()
        self.assertIsInstance(self.reopen(), ownership.ServiceOwnership)
        data = json.loads(self.path.read_text())
        data['groups'][0]['services'] = RETIRED['service_group'].copy()
        self.path.write_text(json.dumps(data), encoding='utf-8')
        retained = self.path.read_bytes()
        resources = Mock()
        with self.assertRaisesRegex(ValueError, 'Invalid group authority'):
            self.reopen().recover(resources)
        self.assertEqual(self.path.read_bytes(), retained)
        self.assertEqual(resources.mock_calls, [])
        self.assertEqual(self.runtime.mock_calls, [])

    def test_occupied_preflight_never_arms_or_deletes_existing_resources(self):
        self.receipt = self.create()
        resources = Mock()
        resources.inventory.return_value = self.live
        with self.assertRaisesRegex(ValueError, 'occupied'):
            self.receipt.group(['opensearch'], self.source / 'docker-compose.yml').arm(resources)
        self.assertEqual(json.loads(self.path.read_text())['groups'][0]['state'], 'reserved')
        self.reopen().recover(resources)
        resources.remove.assert_not_called()

    def test_failed_authorization_write_prevents_service_start(self):
        self.receipt = self.create()
        group = self.receipt.group(['opensearch'], self.source / 'docker-compose.yml')
        resources = Mock()
        resources.inventory.return_value = EMPTY
        with patch.object(ownership.os, 'replace', side_effect=OSError('persist failed')):
            with self.assertRaisesRegex(OSError, 'persist failed'):
                group.arm(resources)
        self.assertEqual(json.loads(self.path.read_text())['groups'][0]['state'], 'reserved')
        self.reopen().recover(resources)
        resources.remove.assert_not_called()

    def test_kill_after_creation_before_observation_has_actionable_intent(self):
        self.receipt = self.create()
        resources = Mock()
        resources.inventory.return_value = EMPTY
        self.receipt.group(['opensearch'], self.source / 'docker-compose.yml').arm(resources)
        resources.inventory.return_value = self.live
        resources.remove.side_effect = lambda *args: self.assertEqual(
            json.loads(self.path.read_text())['groups'][0]['resources'], self.live)
        self.reopen().recover(resources)
        resources.remove.assert_called_once()
        self.assertEqual(json.loads(self.path.read_text())['groups'][0]['state'], 'complete')

    def test_replacement_cannot_adopt_recorded_project_or_service_name(self):
        self.receipt = self.create()
        resources = Mock()
        resources.inventory.return_value = EMPTY
        group = self.receipt.group(['opensearch'], self.source / 'docker-compose.yml')
        group.arm(resources)
        resources.inventory.return_value = self.live
        group.observe(resources)
        resources.inventory.return_value = {**self.live, 'containers': {'d' * 64: self.live['containers'][CID]}}
        with self.assertRaisesRegex(ValueError, 'replacement'):
            self.reopen().recover(resources)
        resources.remove.assert_not_called()

    def test_missing_resources_are_idempotent_but_complete_cannot_adopt_new_resources(self):
        self.receipt = self.create()
        resources = Mock()
        resources.inventory.return_value = EMPTY
        group = self.receipt.group(['opensearch'], self.source / 'docker-compose.yml')
        group.arm(resources)
        self.reopen().recover(resources)
        resources.remove.reset_mock()
        self.reopen().recover(resources)
        resources.remove.assert_not_called()
        resources.inventory.return_value = self.live
        with self.assertRaisesRegex(ValueError, 'replacement'):
            self.reopen().recover(resources)
        resources.remove.assert_not_called()

    def test_old_or_wrong_run_route_source_record_is_refused(self):
        self.receipt = self.create()
        for field, value in [('owner', 'd' * 32), ('engine', ['podman']),
                             ('source', str(self.root)), ('compose_sha256', 'e' * 64), ('schema', 2)]:
            original = self.path.read_text()
            changed = json.loads(original)
            changed[field] = value
            self.path.write_text(json.dumps(changed))
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.reopen()
            self.path.write_text(original)
        self.path.write_text(json.dumps({'owner': 'a' * 32}))
        with self.assertRaises(ValueError):
            self.reopen()

    def test_failed_observation_write_prevents_any_recovery_removal(self):
        self.receipt = self.create()
        resources = Mock()
        resources.inventory.return_value = EMPTY
        self.receipt.group(['opensearch'], self.source / 'docker-compose.yml').arm(resources)
        resources.inventory.return_value = self.live
        with patch.object(ownership.os, 'replace', side_effect=OSError('observation failed')):
            with self.assertRaisesRegex(OSError, 'observation failed'):
                self.reopen().recover(resources)
        resources.remove.assert_not_called()

    def test_empty_container_inventory_does_not_hide_retained_volume_failure(self):
        self.receipt = self.create()
        resources = Mock()
        resources.inventory.return_value = EMPTY
        self.receipt.group(['opensearch'], self.source / 'docker-compose.yml').arm(resources)
        resources.verify_empty.side_effect = ValueError('retained volume')
        with self.assertRaisesRegex(ValueError, 'retained volume'):
            self.reopen().recover(resources)
        self.assertEqual(json.loads(self.path.read_text())['groups'][0]['state'], 'armed')

    def test_record_never_lives_inside_source_or_overwrites_an_existing_run(self):
        with self.assertRaises(ValueError):
            ownership.ServiceOwnership.create(self.source / 'record.json', self.runtime,
                                              self.source, 'a' * 32, [('opensearch',)])
        self.create()
        with self.assertRaises(FileExistsError):
            self.create()


if __name__ == '__main__':
    unittest.main()
