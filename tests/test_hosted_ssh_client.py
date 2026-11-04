"""Local owner failure-path controls; no hosted jobs or real SSH are launched."""
import importlib.util
from pathlib import Path
import tempfile
import sys
import unittest
from unittest.mock import patch
from datetime import datetime, timezone

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'experiments/hosted_ssh_probe'))
try:
    CLIENT_SPEC = importlib.util.spec_from_file_location('hosted_ssh_client',
        Path(__file__).resolve().parents[1] / 'experiments/hosted_ssh_probe/client.py')
    client = importlib.util.module_from_spec(CLIENT_SPEC)
    CLIENT_SPEC.loader.exec_module(client)
finally:
    sys.path.pop(0)


class LocalOwnerTests(unittest.TestCase):
    def test_unknown_phase_is_rejected_before_any_remote_mutation(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(client.shutil, 'which', return_value='/usr/bin/ssh'):
            experiment = client.Experiment(Path(temporary), 'a' * 40, 'openssl')
            with patch.object(experiment, 'api') as api, self.assertRaises(ValueError):
                experiment.launch('unknown')
            api.assert_not_called()

    def test_admission_only_never_launches_live_and_cleanup_failure_remains_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            storage = Path(temporary) / 'external'
            arguments = ['client.py', '--storage', str(storage), '--sha', 'a' * 40,
                         '--openssl', 'openssl', '--admission-only']
            with patch.object(client.sys, 'argv', arguments), \
                    patch.object(client, 'Experiment') as constructor, patch('builtins.print'):
                owner = constructor.return_value
                owner.wait_complete.return_value = {'conclusion': 'success'}
                owner.cleanup.return_value = ['artifact_cleanup']
                self.assertEqual(client.main(), 1)
            owner.launch.assert_called_once_with('admission')
            owner.receive.assert_not_called()
            owner.connect.assert_not_called()

    def test_local_artifact_deletion_survives_run_cleanup_failure(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(client.shutil, 'which', return_value='/usr/bin/ssh'):
            experiment = client.Experiment(Path(temporary), 'a' * 40, 'openssl')
            experiment.runs = [123]
            with patch.object(experiment, 'api', side_effect=RuntimeError), \
                    patch.object(experiment, 'delete_artifacts') as delete:
                failures = experiment.cleanup()
            self.assertEqual(failures, ['run_cleanup'])
            delete.assert_called_once_with(123)

    def test_local_key_cleanup_survives_artifact_cleanup_failure(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(client.shutil, 'which', return_value='/usr/bin/ssh'):
            storage = Path(temporary)
            for name in ('recipient.key', 'client_key'):
                (storage / name).write_text('synthetic private material')
            experiment = client.Experiment(storage, 'a' * 40, 'openssl')
            experiment.runs = [123]
            with patch.object(experiment, 'api', return_value={'status': 'completed'}), \
                    patch.object(experiment, 'delete_artifacts', side_effect=RuntimeError):
                failures = experiment.cleanup()
            self.assertEqual(failures, ['artifact_cleanup'])
            self.assertFalse((storage / 'recipient.key').exists())
            self.assertFalse((storage / 'client_key').exists())

    def test_expired_descriptor_never_starts_ssh(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(client.shutil, 'which', return_value='/usr/bin/ssh'):
            experiment = client.Experiment(Path(temporary), 'a' * 40, 'openssl')
            experiment.config = {'nonce': 'b' * 64}
            with patch.object(experiment, 'command') as command:
                self.assertFalse(experiment.connect(dict(address='8.8.8.8', host_public_key='ssh-ed25519 AAAA', deadline=0)))
            command.assert_not_called()

    def test_accepted_variable_write_with_lost_response_is_reconciled(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(client.shutil, 'which', return_value='/usr/bin/ssh'):
            experiment = client.Experiment(Path(temporary), 'a' * 40, 'openssl')
            experiment.config = {}
            variables = {}
            def api(path, *, method='GET', fields=None, **_kwargs):
                if path.startswith('/git/matching-refs/'):
                    return []
                if method == 'POST' and path == '/actions/variables':
                    variables[fields['name']] = fields['value']
                    raise TimeoutError('response lost after acceptance')
                if path == '/actions/variables?per_page=100':
                    return {'total_count': len(variables), 'variables': [{'name': k} for k in variables]}
                name = path.rsplit('/', 1)[-1]
                if method == 'DELETE':
                    del variables[name]
                    return b''
                return {'value': variables[name]}
            with patch.object(experiment, 'api', side_effect=api):
                with self.assertRaises(TimeoutError):
                    experiment.launch('control')
                self.assertEqual(experiment.cleanup(), [])
            self.assertEqual(variables, {})

    def test_secret_set_then_failed_metadata_read_is_reconciled(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(client.shutil, 'which', return_value='/usr/bin/ssh'):
            experiment = client.Experiment(Path(temporary), 'a' * 40, 'openssl')
            variables, secret = {}, {}
            reads = []
            def command(*_args, **_kwargs):
                stamp = datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
                secret.update(name=client.SECRET, created_at=stamp, updated_at=stamp)
                return b''
            def api(path, *, method='GET', fields=None, **_kwargs):
                if path.startswith('/git/matching-refs/'):
                    return []
                if method == 'POST':
                    variables[fields['name']] = fields['value']
                    return b''
                if path == '/actions/variables?per_page=100':
                    return {'total_count': len(variables), 'variables': [{'name': k} for k in variables]}
                if path == '/actions/secrets?per_page=100':
                    return {'total_count': bool(secret), 'secrets': [{'name': client.SECRET}] if secret else []}
                name = path.rsplit('/', 1)[-1]
                if name == client.SECRET:
                    if method == 'DELETE':
                        secret.clear()
                        return b''
                    reads.append(1)
                    if len(reads) == 1:
                        raise TimeoutError('metadata response lost')
                    return secret.copy()
                if method == 'DELETE':
                    del variables[name]
                    return b''
                return {'value': variables[name]}
            with patch.object(experiment, 'api', side_effect=api), \
                    patch.object(experiment, 'command', side_effect=command):
                with self.assertRaises(TimeoutError):
                    experiment.launch('control')
                self.assertEqual(experiment.cleanup(), [])
            self.assertEqual(secret, {})

    def test_preexisting_same_head_tag_is_never_claimed(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(client.shutil, 'which', return_value='/usr/bin/ssh'):
            experiment = client.Experiment(Path(temporary), 'a' * 40, 'openssl')
            with patch.object(experiment, 'api', return_value=[{'object': {'sha': 'a' * 40}}]), \
                    self.assertRaises(ValueError):
                experiment.launch('control')
            self.assertEqual(experiment.tags, [])
            self.assertEqual(experiment.variables, {})

    def test_cleanup_discovers_run_after_ambiguous_dispatch(self):
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(client.shutil, 'which', return_value='/usr/bin/ssh'):
            experiment = client.Experiment(Path(temporary), 'a' * 40, 'openssl')
            tag = 'sp007-direct-probe-control-owned'
            experiment.tags = ['refs/tags/' + tag]
            run = dict(id=123, head_branch=tag, path=client.WORKFLOW, head_sha='a' * 40,
                       event='push', run_attempt=1, actor={'login': 'MrQuality'}, status='completed')
            def api(path, **_kwargs):
                if path.startswith('/actions/runs?'):
                    return {'total_count': 1, 'workflow_runs': [run]}
                if path.startswith('/git/matching-refs/'):
                    return []
                return run
            with patch.object(experiment, 'api', side_effect=api), \
                    patch.object(experiment, 'delete_artifacts') as delete:
                self.assertEqual(experiment.cleanup(), [])
            self.assertEqual(experiment.runs, [123])
            delete.assert_called_once_with(123)


if __name__ == '__main__':
    unittest.main()
