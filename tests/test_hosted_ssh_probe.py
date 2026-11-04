"""Admission and confidentiality oracles for the disposable reachability probe."""
import hashlib
import base64
import importlib.util
import io
import json
from pathlib import Path
import unittest
import zipfile
import tempfile
import subprocess
import threading
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('hosted_ssh_probe',
    Path(__file__).resolve().parents[1] / 'experiments/hosted_ssh_probe/probe.py')
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)



class ProbeContractTests(unittest.TestCase):
    def expected(self):
        return dict(version=1, repository='MrQuality/kehila', sha='a' * 40,
                    ref='refs/tags/sp007-direct-probe-control',
                    workflow='.github/workflows/hosted-ssh-probe.yml',
                    run_id='123', attempt=1, nonce='b' * 64,
                    recipient_sha256='c' * 64)

    def descriptor(self):
        return dict(self.expected(), address='8.8.8.8', port=49152,
                    host_public_key='ssh-ed25519 ' + base64.b64encode(
                        b'\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20' + b'k' * 32).decode(),
                    deadline=1100)

    def test_current_exact_descriptor_is_admitted(self):
        self.assertEqual(probe.admit_descriptor(self.descriptor(), self.expected(), 1000),
                         self.descriptor())

    def test_each_producer_and_session_binding_is_required(self):
        for field in self.expected():
            with self.subTest(field=field):
                changed = self.descriptor()
                changed[field] = 'wrong'
                with self.assertRaises(ValueError):
                    probe.admit_descriptor(changed, self.expected(), 1000)

    def test_expiry_and_excessive_lease_fail_closed(self):
        for deadline in (1000, 999, 1121, True):
            changed = dict(self.descriptor(), deadline=deadline)
            with self.subTest(deadline=deadline), self.assertRaises(ValueError):
                probe.admit_descriptor(changed, self.expected(), 1000)

    def test_non_public_addresses_and_non_ipv4_are_rejected(self):
        for address in ('127.0.0.1', '10.0.0.1', '169.254.169.254', '::1',
                        'example.com', '8.8.8.8\noption'):
            with self.subTest(address=address), self.assertRaises(ValueError):
                probe.admit_descriptor(dict(self.descriptor(), address=address),
                                       self.expected(), 1000)

    def test_only_selected_high_port_is_accepted(self):
        for port in (22, 443, 65536, True, '49152'):
            with self.subTest(port=port), self.assertRaises(ValueError):
                probe.admit_descriptor(dict(self.descriptor(), port=port),
                                       self.expected(), 1000)

    def test_key_and_unknown_fields_cannot_inject_configuration(self):
        for changed in (dict(self.descriptor(), host_public_key='ssh-ed25519 x\nHost *'),
                        dict(self.descriptor(), extra='unexpected')):
            with self.assertRaises(ValueError):
                probe.admit_descriptor(changed, self.expected(), 1000)

    def archive(self, entries):
        output = io.BytesIO()
        with zipfile.ZipFile(output, 'w') as archive:
            for name, payload in entries:
                archive.writestr(name, payload)
        return output.getvalue()

    def test_archive_requires_exact_digest_and_single_fixed_member(self):
        raw = self.archive([('descriptor.der', b'opaque encrypted bytes')])
        digest = 'sha256:' + hashlib.sha256(raw).hexdigest()
        self.assertEqual(probe.envelope_from_archive(raw, digest), b'opaque encrypted bytes')
        with self.assertRaises(ValueError):
            probe.envelope_from_archive(raw, 'sha256:' + '0' * 64)
        for entries in ([('../descriptor.der', b'x')], [('other', b'x')],
                        [('descriptor.der', b'x'), ('other', b'y')],
                        [('descriptor.der', b'x' * 32769)]):
            raw = self.archive(entries)
            with self.subTest(entries=[x[0] for x in entries]), self.assertRaises(ValueError):
                probe.envelope_from_archive(raw, 'sha256:' + hashlib.sha256(raw).hexdigest())

    def test_duplicate_json_fields_are_not_silently_overwritten(self):
        with self.assertRaises(ValueError):
            probe.read_json(b'{"nonce":"one","nonce":"two"}')
        with self.assertRaises(ValueError):
            probe.read_json(b'x' * 32769)

    def test_control_member_requires_explicit_selection(self):
        raw = self.archive([('control.der', b'opaque')])
        digest = 'sha256:' + hashlib.sha256(raw).hexdigest()
        with self.assertRaises(ValueError):
            probe.envelope_from_archive(raw, digest)
        self.assertEqual(probe.envelope_from_archive(raw, digest, 'control.der'), b'opaque')

    def test_artifact_deletion_survives_container_cleanup_failure(self):
        with patch.object(probe, 'close_and_remove_containers', side_effect=RuntimeError), \
                patch.object(probe, 'delete_artifacts') as delete, \
                patch('builtins.print'), self.assertRaises(ValueError):
            probe.cleanup(Path('synthetic'))
        delete.assert_called_once_with()

    def test_both_cleanup_failures_remain_failure(self):
        with patch.object(probe, 'close_and_remove_containers') as remove, \
                patch.object(probe, 'delete_artifacts', side_effect=RuntimeError), \
                patch('builtins.print'), self.assertRaises(ValueError):
            probe.cleanup(Path('synthetic'))
        remove.assert_called_once()

    def test_creation_identity_is_persisted_before_ambiguous_engine_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            storage = Path(temporary)
            (storage / 'binding.json').write_text(json.dumps(self.expected()))
            with patch.object(probe, 'run', return_value=b''):
                options = probe.reserve_container(storage, 'live')
            self.assertEqual(options[:2], ['--name', 'kehila-ssh-123-live'])
            calls = []
            def engine(command, *_args, **_kwargs):
                calls.append(command)
                if command[3] == 'ps' and sum(c[3] == 'ps' for c in calls) == 1:
                    return b'a' * 64
                return b''
            # No container ID was ever returned or saved. Ownership still suffices.
            with patch.object(probe, 'run', side_effect=engine):
                probe.remove_containers(storage)
            self.assertTrue(any(command[3] == 'rm' for command in calls))

    def test_owner_cleans_after_lost_creation_response_and_preserves_failure_stage(self):
        with tempfile.TemporaryDirectory() as temporary:
            storage = Path(temporary)
            binding = self.expected()
            (storage / 'binding.json').write_text(json.dumps(binding))
            def engine(command, *_args, **_kwargs):
                if '--version' in command:
                    return b'podman version 4.9.3\n'
                if 'build' in command:
                    return b'build progress is not image identity'
                if 'cat' in command:
                    return b'a' * 64
                raise RuntimeError('unexpected engine command')
            def create(_storage, suffix, *_args):
                self.assertEqual(_args[1], 'a' * 64)
                if suffix == 'policy':
                    return (b'passwordauthentication no\nkbdinteractiveauthentication no\n'
                            b'allowtcpforwarding local\nmaxsessions 0\n'
                            b'permitopen 127.0.0.1:31337\npermitlisten none\n')
                options = _args[0]
                self.assertIn('--network=bridge', options)
                self.assertEqual(options[options.index('--publish') + 1], '10.20.30.40:49152:2222')
                raise subprocess.TimeoutExpired(['synthetic'], 30)
            with patch.object(probe, 'source_binding', return_value=binding), \
                    patch.object(probe, 'candidate_address', return_value='8.8.8.8'), \
                    patch.object(probe, 'create_container', side_effect=create), \
                    patch.object(probe, 'admit_mount_ownership'), \
                    patch.object(probe, 'selected_guest_ipv4', return_value='10.20.30.40'), \
                    patch.object(probe, 'expiry_control', return_value={'ssh_nonce': True, 'expiry': True}), \
                    patch.object(probe, 'run', side_effect=engine), \
                    patch.object(probe, 'remove_containers') as remove, \
                    self.assertRaises(subprocess.TimeoutExpired):
                probe.serve(storage)
            remove.assert_called_once_with(storage)
            result = json.loads((storage / 'result.json').read_bytes())
            self.assertEqual(result['failure_stage'], 'listener')
            self.assertEqual(result['failure_class'], 'TimeoutExpired')
            self.assertFalse(result['owner_succeeded'])

    def test_cleanup_during_image_preparation_closes_future_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            storage = Path(temporary)
            binding = self.expected()
            (storage / 'binding.json').write_text(json.dumps(binding))
            entered, resume = threading.Event(), threading.Event()
            late_creates, errors = [], []
            def engine(command, *_args, **_kwargs):
                if '--version' in command:
                    return b'podman version 4.9.3\n'
                if 'build' in command:
                    entered.set()
                    if not resume.wait(2):
                        raise TimeoutError('test synchronization failed')
                    return b'a' * 64
                if 'cat' in command:
                    return b'a' * 64
                if '/usr/sbin/sshd' in command:
                    return (b'passwordauthentication no\nkbdinteractiveauthentication no\n'
                            b'allowtcpforwarding local\nmaxsessions 0\n'
                            b'permitopen 127.0.0.1:31337\npermitlisten none\n')
                if '--detach' in command:
                    late_creates.append(command)
                    raise RuntimeError('late listener creation')
                return b''
            def owner():
                try:
                    probe.serve(storage)
                except Exception as error:
                    errors.append(type(error).__name__)
            with patch.object(probe, 'source_binding', return_value=binding), \
                    patch.object(probe, 'candidate_address', return_value='8.8.8.8'), \
                    patch.object(probe, 'admit_mount_ownership'), \
                    patch.object(probe, 'selected_guest_ipv4', return_value='10.20.30.40'), \
                    patch.object(probe, 'expiry_control'), \
                    patch.object(probe, 'run', side_effect=engine), \
                    patch.object(probe, 'remove_containers'), \
                    patch.object(probe, 'delete_artifacts'), patch('builtins.print'):
                worker = threading.Thread(target=owner)
                worker.start()
                try:
                    self.assertTrue(entered.wait(2))
                    probe.cleanup(storage)
                finally:
                    resume.set()
                    worker.join(3)
                self.assertFalse(worker.is_alive())
            self.assertEqual(late_creates, [])
            self.assertEqual(errors, ['ValueError'])

    def test_unacknowledged_unobserved_creation_cannot_be_claimed_absent(self):
        with tempfile.TemporaryDirectory() as temporary:
            storage = Path(temporary)
            (storage / 'binding.json').write_text(json.dumps(self.expected()))
            def engine(command, *_args, **_kwargs):
                if command[3] == 'ps':
                    return b''
                raise subprocess.TimeoutExpired(command, 30)
            with patch.object(probe, 'run', side_effect=engine), self.assertRaises(subprocess.TimeoutExpired):
                probe.create_container(storage, 'live', [], 'a' * 64)
            with patch.object(probe, 'run', return_value=b''), self.assertRaises(ValueError):
                probe.close_and_remove_containers(storage)
            self.assertTrue((storage / 'admission.closed').exists())

    def test_listener_rejections_identify_state_mapping_and_readiness_without_leaking(self):
        for failure in ('state', 'mapping', 'readiness'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                storage = Path(temporary)
                inspect = dict(State={'Running': failure != 'state', 'ExitCode': 0,
                                      'Error': 'synthetic-sensitive-diagnostic'},
                    NetworkSettings={'Ports': {'2222/tcp': [{'HostIp': '198.51.100.27'
                        if failure == 'mapping' else '127.0.0.1', 'HostPort': '49152'}]}})
                responses = [json.dumps([inspect]).encode()]
                if failure == 'readiness':
                    responses.append(ValueError('synthetic-sensitive-command-error'))
                with patch.object(probe, 'run', side_effect=responses), self.assertRaises(ValueError):
                    probe.admit_listener(storage, 'a' * 64, '127.0.0.1', 'control')
                self.assertEqual((storage / 'stage').read_text(), 'control-' + failure)
                safe = (storage / 'admission.json').read_text()
                self.assertNotIn('198.51.100.27', safe)
                self.assertNotIn('synthetic-sensitive', safe)
                self.assertIn('mapping_matches', safe)

    def test_inspect_command_failure_has_its_own_stage(self):
        with tempfile.TemporaryDirectory() as temporary:
            storage = Path(temporary)
            with patch.object(probe, 'run', side_effect=ValueError), self.assertRaises(ValueError):
                probe.admit_listener(storage, 'a' * 64, '127.0.0.1', 'control')
            self.assertEqual((storage / 'stage').read_text(), 'control-inspect')

    def test_admission_only_cannot_create_public_listener_or_descriptor(self):
        with tempfile.TemporaryDirectory() as temporary:
            storage = Path(temporary)
            binding = self.expected()
            (storage / 'binding.json').write_text(json.dumps(binding))
            policy = (b'passwordauthentication no\nkbdinteractiveauthentication no\n'
                      b'allowtcpforwarding local\nmaxsessions 0\n'
                      b'permitopen 127.0.0.1:31337\npermitlisten none\n')
            def engine(command, *_args, **_kwargs):
                if '--version' in command:return b'podman version 4.9.3\n'
                if 'cat' in command:return b'a' * 64
                return b'build progress'
            with patch.object(probe, 'source_binding', return_value=binding), \
                    patch.object(probe, 'candidate_address') as candidate, \
                    patch.object(probe, 'admit_mount_ownership'), \
                    patch.object(probe, 'selected_guest_ipv4', return_value='10.20.30.40'), \
                    patch.object(probe, 'run', side_effect=engine), \
                    patch.object(probe, 'create_container', return_value=policy) as create, \
                    patch.object(probe, 'expiry_control', return_value={'ssh_nonce': True, 'expiry': True}):
                probe.serve(storage, admission_only=True)
            candidate.assert_not_called()
            self.assertEqual([c.args[1] for c in create.call_args_list], ['policy'])
            self.assertFalse((storage / 'descriptor.der').exists())
            result = json.loads((storage / 'result.json').read_bytes())
            self.assertTrue(result['admission_only'])
            self.assertFalse(result['descriptor_ready'])

    def test_dispatch_phase_must_match_the_admitted_tag(self):
        config = dict(self.expected(), ref='refs/tags/sp007-direct-probe-admission-owned')
        environment = dict(GITHUB_REPOSITORY='MrQuality/kehila', GITHUB_ACTOR='MrQuality',
            GITHUB_EVENT_NAME='push', GITHUB_RUN_ATTEMPT='1', GITHUB_SHA='a' * 40,
            GITHUB_WORKFLOW_SHA='a' * 40, GITHUB_REF=config['ref'], GITHUB_RUN_ID='123',
            PROBE_PHASE='admission')
        with patch.dict(probe.os.environ, environment, clear=True):
            self.assertEqual(probe.source_binding(config)['ref'], config['ref'])
            probe.os.environ['PROBE_PHASE'] = 'live'
            with self.assertRaises(ValueError):
                probe.source_binding(config)

    def test_guest_bind_uses_private_preferred_source_assigned_on_same_interface(self):
        route = [{'prefsrc': '10.20.30.40', 'dev': 'eth0'}]
        interface = [{'ifname': 'eth0', 'addr_info': [
            {'family': 'inet', 'scope': 'global', 'local': '10.20.30.40'}]}]
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(probe, 'run', side_effect=[json.dumps(route).encode(),
                    json.dumps(interface).encode()]):
                self.assertEqual(probe.selected_guest_ipv4(Path(temporary)), '10.20.30.40')

    def test_guest_bind_rejects_ambiguous_route_or_unassigned_source(self):
        good = {'prefsrc': '10.20.30.40', 'dev': 'eth0'}
        assigned = {'family': 'inet', 'scope': 'global', 'local': '10.20.30.40'}
        for routes, interfaces in (
                ([good, good], []), ([{'dev': 'eth0'}], []),
                ([dict(good, prefsrc='::1')], []), ([dict(good, prefsrc='0.0.0.0')], []),
                ([dict(good, prefsrc='127.0.0.1')], []),
                ([dict(good, prefsrc='169.254.169.254')], []),
                ([dict(good, dev='--help')], []),
                ([good], [{'ifname': 'eth1', 'addr_info': [assigned]}]),
                ([good], [{'ifname': 'eth0', 'addr_info': []}]),
                ([good], [{'ifname': 'eth0', 'addr_info': [dict(assigned, scope='host')]}]),
                ([good], [{'ifname': 'eth0', 'addr_info': [assigned, assigned]}])):
            with self.subTest(routes=routes), tempfile.TemporaryDirectory() as temporary:
                with patch.object(probe, 'run', side_effect=[json.dumps(routes).encode(),
                        json.dumps(interfaces).encode()]), self.assertRaises(ValueError):
                    probe.selected_guest_ipv4(Path(temporary))

    def test_empty_mapping_is_not_normalized_for_the_selected_guest_bind(self):
        with tempfile.TemporaryDirectory() as temporary:
            inspected = [{'State': {'Running': True, 'ExitCode': 0},
                'NetworkSettings': {'Ports': {'2222/tcp': [{'HostIp': '', 'HostPort': '49152'}]}}}]
            with patch.object(probe, 'run', return_value=json.dumps(inspected).encode()), \
                    self.assertRaises(ValueError):
                probe.admit_listener(Path(temporary), 'a' * 64, '10.20.30.40', 'listener')
            self.assertEqual((Path(temporary) / 'stage').read_text(), 'listener-mapping')

    def test_exact_selected_guest_mapping_is_admitted_without_public_address_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            inspected = [{'State': {'Running': True, 'ExitCode': 0},
                'NetworkSettings': {'Ports': {'2222/tcp': [{'HostIp': '10.20.30.40', 'HostPort': '49152'}]}}}]
            with patch.object(probe, 'run', side_effect=[json.dumps(inspected).encode(), b'']):
                result = probe.admit_listener(Path(temporary), 'a' * 64, '10.20.30.40', 'listener')
            self.assertTrue(result['ready'])
            self.assertEqual(result['host_kind'], 'selected-guest-ipv4')
            self.assertNotIn('10.20.30.40', json.dumps(result))

    def test_guest_mapping_rejects_broader_different_or_extra_publications(self):
        expected = {'HostIp': '10.20.30.40', 'HostPort': '49152'}
        for mappings in ([dict(expected, HostIp='0.0.0.0')],
                [dict(expected, HostIp='::')], [dict(expected, HostIp='10.20.30.41')],
                [dict(expected, HostPort='49153')], [dict(expected, HostPort=49152)],
                [dict(expected, HostIp=None)], [expected, expected],
                [dict(expected, extra='unexpected')]):
            with self.subTest(mapping=mappings), tempfile.TemporaryDirectory() as temporary:
                inspected = [{'State': {'Running': True, 'ExitCode': 0},
                    'NetworkSettings': {'Ports': {'2222/tcp': mappings}}}]
                with patch.object(probe, 'run', return_value=json.dumps(inspected).encode()), \
                        self.assertRaises(ValueError):
                    probe.admit_listener(Path(temporary), 'a' * 64, '10.20.30.40', 'listener')
                self.assertEqual((Path(temporary) / 'stage').read_text(), 'listener-mapping')

    def test_unpublished_metadata_is_distinct_from_an_additional_host_mapping(self):
        for extra, accepted in ((None, True), ([], False),
                ([{'HostIp': '10.20.30.40', 'HostPort': '49153'}], False)):
            with self.subTest(extra=extra), tempfile.TemporaryDirectory() as temporary:
                inspected = [{'State': {'Running': True, 'ExitCode': 0},
                    'NetworkSettings': {'Ports': {'2222/tcp': [
                        {'HostIp': '10.20.30.40', 'HostPort': '49152'}], '9999/tcp': extra}}}]
                with patch.object(probe, 'run', side_effect=[json.dumps(inspected).encode(), b'']):
                    if accepted:
                        self.assertTrue(probe.admit_listener(Path(temporary), 'a' * 64,
                                                            '10.20.30.40', 'listener')['ready'])
                    else:
                        with self.assertRaises(ValueError):
                            probe.admit_listener(Path(temporary), 'a' * 64, '10.20.30.40', 'listener')



if __name__ == '__main__':
    unittest.main()
