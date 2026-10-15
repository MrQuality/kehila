"""Owned services must use a validated local daemon."""
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import test_services as services

class DaemonRoutingTests(unittest.TestCase):
    def test_ambient_daemon_overrides_are_rejected_before_io(self):
        for variable in ('DOCKER_HOST', 'DOCKER_CONTEXT', 'CONTAINER_HOST', 'CONTAINER_CONNECTION'):
            with self.subTest(variable=variable), patch.object(services, 'run') as run:
                with self.assertRaises(ValueError):
                    with services.managed_runtime('docker', environment={variable: 'foreign'}):
                        pass
                run.assert_not_called()

    def test_saved_remote_docker_context_is_rejected_before_container_creation(self):
        for host in ('ssh://remote', 'tcp://127.0.0.1:2375', 'unix://relative', ''):
            record = [{'Name': 'selected', 'Endpoints': {'docker': {'Host': host}}}]
            with self.subTest(host=host), patch.object(services, 'run', return_value=Mock(stdout=json.dumps(record))):
                with self.assertRaises(ValueError):
                    services.local_runtime(['docker', 'compose'], {})

    def test_local_docker_context_is_pinned_for_compose_and_inspection(self):
        for host in ('unix:///var/run/docker.sock', 'npipe:////./pipe/docker_engine'):
            record = [{'Name': 'selected', 'Endpoints': {'docker': {'Host': host}}}]
            with self.subTest(host=host), patch.object(services, 'run', return_value=Mock(stdout=json.dumps(record))) as run:
                runtime = services.local_runtime(['docker', 'compose'], {})
            self.assertEqual(runtime.environment, {'DOCKER_CONTEXT': 'selected'})
            self.assertEqual(runtime.engine, ['docker'])
            self.assertEqual(run.call_args.args[0], ['docker', 'context', 'inspect'])

    def test_malformed_context_metadata_fails_closed(self):
        for raw in ('not json', 'null', '[]', '[{}]', '[{},{}]'):
            with self.subTest(raw=raw), patch.object(services, 'run', return_value=Mock(stdout=raw)):
                with self.assertRaises(ValueError):
                    services.local_runtime(['docker', 'compose'], {})

    def test_podman_without_a_machine_requires_a_native_local_service(self):
        for metadata in ({}, {'host': {}}, {'host': {'serviceIsRemote': True}}):
            with self.subTest(metadata=metadata), patch.object(services, 'run', return_value=Mock(stdout=json.dumps(metadata))):
                with self.assertRaises(ValueError):
                    services.local_runtime(['podman', 'compose'], {})
        with patch.object(services, 'run', return_value=Mock(stdout='{"host":{"serviceIsRemote":false}}')):
            runtime = services.local_runtime(['podman', 'compose'], {})
        self.assertEqual(runtime.compose, ['podman', '--remote=false', 'compose'])
        self.assertEqual(runtime.engine, ['podman', '--remote=false'])
