"""Service recovery validates exact engine resources before destructive actions."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from test_services import ComposeRuntime
from verification_ownership import EMPTY
from verification_service_resources import ComposeResources

CID, NID, FOREIGN = 'b' * 64, 'c' * 64, 'd' * 64
PROJECT = 'kehila-test-' + 'a' * 32
SOURCE = str(Path('synthetic-source').resolve())
GROUP = {'project': PROJECT, 'services': ['opensearch']}


class FakeResources(ComposeResources):
    def __init__(self):
        super().__init__(ComposeRuntime(['podman', 'compose'], {}), seconds=30)
        self.calls = []
        self.containers = {CID: {'Id': CID, 'Name': PROJECT + '_opensearch_1',
            'Config': {'Labels': {'com.docker.compose.project': PROJECT,
                'com.docker.compose.service': 'opensearch',
                'com.docker.compose.project.working_dir': SOURCE,
                'com.docker.compose.project.config_files': str(Path(SOURCE) / 'docker-compose.yml')}},
            'Mounts': [], 'NetworkSettings': {'Networks': {PROJECT + '_default': {}}}}}
        self.networks = {NID: {'id': NID, 'name': PROJECT + '_default',
            'labels': {'com.docker.compose.project': PROJECT},
            'containers': {CID: {'name': PROJECT + '_opensearch_1'}}}}
        self.volumes = {}

    def _invoke(self, arguments, *, check=True):
        self.calls.append(arguments)
        if arguments[0] == 'ps':
            value = '\n'.join(self.containers)
        elif arguments[:2] == ('network', 'ls'):
            value = '\n'.join(net['name'] for net in self.networks.values())
        elif arguments[:2] == ('volume', 'ls'):
            value = ''
        elif arguments[:3] == ('inspect', '--type', 'container'):
            value = json.dumps([self.containers[arguments[-1]]])
        elif arguments[:2] == ('network', 'inspect'):
            value = json.dumps([next(net for net in self.networks.values()
                                     if arguments[-1] in (net['id'], net['name']))])
        elif arguments[:2] == ('volume', 'inspect'):
            value = json.dumps([self.volumes[arguments[-1]]])
        elif arguments[:2] == ('volume', 'exists'):
            return subprocess.CompletedProcess(arguments, 0 if arguments[-1] in self.volumes else 1, '', '')
        elif arguments[:3] == ('rm', '--force', '--volumes'):
            del self.containers[arguments[-1]]
            for net in self.networks.values():
                net['containers'].pop(arguments[-1], None)
            value = ''
        elif arguments[:2] == ('network', 'rm'):
            del self.networks[arguments[-1]]
            value = ''
        else:
            raise AssertionError(arguments)
        return subprocess.CompletedProcess(arguments, 0, value, '')


class ResourceTests(unittest.TestCase):
    def test_exact_container_then_network_removal_is_verified_and_idempotent(self):
        resources = FakeResources()
        observed = resources.inventory(GROUP, SOURCE)
        resources.remove(GROUP, SOURCE, observed)
        destructive = [call for call in resources.calls if call[0] == 'rm' or call[:2] == ('network', 'rm')]
        self.assertEqual(destructive, [('rm', '--force', '--volumes', CID), ('network', 'rm', NID)])
        self.assertEqual(resources.inventory(GROUP, SOURCE), EMPTY)
        resources.remove(GROUP, SOURCE, observed)

    def test_foreign_source_or_service_or_short_identity_cannot_authorize_removal(self):
        for label, value in [('com.docker.compose.project.working_dir', SOURCE + '-other'),
                             ('com.docker.compose.project.config_files', 'other.yml'),
                             ('com.docker.compose.service', 'nats')]:
            resources = FakeResources()
            resources.containers[CID]['Config']['Labels'][label] = value
            with self.subTest(label=label), self.assertRaises(ValueError):
                resources.inventory(GROUP, SOURCE)
        resources = FakeResources()
        resources.containers[CID]['Id'] = CID[:12]
        with self.assertRaises(ValueError):
            resources.inventory(GROUP, SOURCE)

    def test_foreign_endpoint_or_host_mount_is_refused(self):
        resources = FakeResources()
        resources.networks[NID]['containers'][FOREIGN] = {'name': 'other-work'}
        with self.assertRaises(ValueError):
            resources.inventory(GROUP, SOURCE)
        resources = FakeResources()
        resources.containers[CID]['Mounts'] = [{'Type': 'bind', 'Source': '/other', 'Destination': '/data'}]
        with self.assertRaises(ValueError):
            resources.inventory(GROUP, SOURCE)

    def test_reinspection_cannot_adopt_a_replacement_after_planning(self):
        resources = FakeResources()
        observed = resources.inventory(GROUP, SOURCE)
        replacement = resources.containers.pop(CID)
        replacement['Id'] = FOREIGN
        resources.containers[FOREIGN] = replacement
        resources.networks[NID]['containers'] = {FOREIGN: {}}
        with self.assertRaisesRegex(ValueError, 'replacement'):
            resources.remove(GROUP, SOURCE, observed)
        self.assertFalse(any(call[0] == 'rm' for call in resources.calls))

    def test_duplicate_service_is_not_a_partial_start(self):
        resources = FakeResources()
        second = copy.deepcopy(resources.containers[CID])
        second['Id'] = FOREIGN
        resources.containers[FOREIGN] = second
        with self.assertRaises(ValueError):
            resources.inventory(GROUP, SOURCE)

    def test_named_volume_is_refused_and_anonymous_retention_does_not_pass(self):
        name = 'e' * 64
        resources = FakeResources()
        resources.containers[CID]['Mounts'] = [{'Type': 'volume', 'Name': name, 'Destination': '/data'}]
        resources.volumes[name] = {'Name': name, 'Anonymous': False, 'CreatedAt': 'synthetic-time'}
        with self.assertRaisesRegex(ValueError, 'named volumes'):
            resources.inventory(GROUP, SOURCE)
        resources.volumes[name]['Anonymous'] = True
        observed = resources.inventory(GROUP, SOURCE)
        with self.assertRaisesRegex(ValueError, 'volume retention'):
            resources.remove(GROUP, SOURCE, observed)
        self.assertFalse(any(call[:2] == ('volume', 'rm') for call in resources.calls))

    def test_one_absolute_cleanup_deadline_clamps_commands(self):
        resources = ComposeResources(ComposeRuntime(['podman', 'compose'], {}), seconds=5)
        with patch('verification_service_resources.time.monotonic', return_value=resources.deadline + 1), \
                patch('verification_service_resources.subprocess.run') as command:
            with self.assertRaises(TimeoutError):
                resources.inventory(GROUP, SOURCE)
        command.assert_not_called()


if __name__ == '__main__':
    unittest.main()
