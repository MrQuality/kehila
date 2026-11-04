"""Disposable A-only reachability research; no product secrets or TLS authority.

Raw state is confined to the caller's external storage. Only the explicitly
encrypted descriptor and aggregate results may leave that storage. This is a
temporary experiment, not a reusable production session bootstrap.
"""
import argparse
import base64
from contextlib import contextmanager
import hashlib
import io
import ipaddress
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import urllib.request
import zipfile

PORT = 49152
LEASE = 120
LIMIT = 32768
WORKFLOW = '.github/workflows/hosted-ssh-probe.yml'


def require(condition):
    if not condition:
        raise ValueError('probe contract rejected')


def write_result(storage, result):
    pending = storage / 'result.pending'
    pending.write_text(json.dumps(result))
    pending.replace(storage / 'result.json')


def read_json(raw):
    require(len(raw) <= LIMIT)

    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result)
            result[key] = value
        return result

    result = json.loads(raw, object_pairs_hook=unique)
    require(isinstance(result, dict))
    return result


def public_ipv4(value):
    address = ipaddress.IPv4Address(value)
    require(address.is_global and str(address) == value)
    return value


def public_key(value):
    require(isinstance(value, str) and re.fullmatch(r'ssh-ed25519 [A-Za-z0-9+/]{68}', value))
    decoded = base64.b64decode(value.split()[1], validate=True)
    require(len(decoded) == 51 and decoded[:19] == b'\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20')
    return value


def admit_descriptor(descriptor, expected, now):
    require(set(descriptor) == set(expected) | {'address', 'port', 'host_public_key', 'deadline'})
    require(all(type(descriptor[key]) is type(value) and descriptor[key] == value
                for key, value in expected.items()))
    public_ipv4(descriptor['address'])
    require(type(descriptor['port']) is int and descriptor['port'] == PORT)
    public_key(descriptor['host_public_key'])
    require(type(descriptor['deadline']) is int and now < descriptor['deadline'] <= now + LEASE)
    return descriptor


def envelope_from_archive(raw, digest, member_name='descriptor.der'):
    require(member_name in {'descriptor.der', 'control.der'})
    require(len(raw) <= LIMIT + 4096)
    require(digest == 'sha256:' + hashlib.sha256(raw).hexdigest())
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        members = archive.infolist()
        require(len(members) == 1 and members[0].filename == member_name)
        require(0 < members[0].file_size <= LIMIT and not members[0].is_dir())
        # Read a fixed member into memory; never extract paths or follow links.
        with archive.open(members[0]) as member:
            result = member.read(LIMIT + 1)
        require(0 < len(result) <= LIMIT)
        return result


def run(command, storage, *, timeout=30, data=None):
    """Hosted-only bounded commands; confidential output never inherits CI pipes."""
    with (storage / 'commands.log').open('ab') as errors, \
            (storage / 'command-output').open('w+b') as output:
        result = subprocess.run(command, input=data, stdout=output,
                                stderr=errors, timeout=timeout, check=False)
        require(result.returncode == 0 and output.tell() <= 1024 * 1024)
        output.seek(0)
        return output.read(1024 * 1024)


def encrypt(storage, plain, output):
    # OAEP + GCM provides confidentiality/integrity, not producer identity.
    run(['openssl', 'cms', '-encrypt', '-binary', '-aes-256-gcm',
         '-recip', str(storage / 'recipient.crt'), '-keyopt', 'rsa_padding_mode:oaep',
         '-keyopt', 'rsa_oaep_md:sha256', '-in', str(plain),
         '-outform', 'DER', '-out', str(output)], storage)


def source_binding(config):
    env = os.environ
    require(env['GITHUB_REPOSITORY'] == 'MrQuality/kehila'
            and env['GITHUB_ACTOR'] == 'MrQuality' and env['GITHUB_EVENT_NAME'] == 'push'
            and env['GITHUB_RUN_ATTEMPT'] == '1')
    require(re.fullmatch(r'[0-9a-f]{40}', config['sha'])
            and env['GITHUB_SHA'] == config['sha'] == env['GITHUB_WORKFLOW_SHA'])
    require(re.fullmatch(r'refs/tags/sp007-direct-probe-[a-z0-9-]{1,64}', config['ref'])
            and env['GITHUB_REF'] == config['ref'])
    return dict(version=1, repository='MrQuality/kehila', sha=config['sha'], ref=config['ref'],
                workflow=WORKFLOW, run_id=env['GITHUB_RUN_ID'], attempt=1,
                nonce=config['nonce'], recipient_sha256=config['recipient_sha256'])


def prepare(storage):
    require(not storage.exists())
    storage.mkdir(mode=0o700)
    config = read_json(os.environ.pop('PROBE_CONFIG').encode())
    require(set(config) == {'sha', 'ref', 'client_public_key', 'client_ipv4',
                            'nonce', 'recipient_pem', 'recipient_sha256'})
    binding = source_binding(config)
    require(re.fullmatch(r'[0-9a-f]{64}', config['nonce']))
    public_key(config['client_public_key'])
    public_ipv4(config['client_ipv4'])
    certificate = config['recipient_pem'].encode('ascii')
    require(len(certificate) <= 8192 and hashlib.sha256(certificate).hexdigest()
            == config['recipient_sha256'])
    (storage / 'recipient.crt').write_bytes(certificate)
    run(['openssl', 'x509', '-in', str(storage / 'recipient.crt'), '-noout', '-checkend', '600'], storage)
    (storage / 'binding.json').write_text(json.dumps(binding))
    # Restrict the original source as well as the credential. VPN rotation fails closed.
    (storage / 'authorized_keys').write_text('from="' + config['client_ipv4'] +
        '",restrict,port-forwarding ' + config['client_public_key'] + '\n')
    (storage / 'authorized_keys').chmod(0o444)
    run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-C', '',
         '-f', str(storage / 'host_key')], storage)
    # Preflight encryption is synthetic: local consumer must verify it before exposure.
    (storage / 'control.json').write_text(json.dumps(dict(binding, control='synthetic-only')))
    encrypt(storage, storage / 'control.json', storage / 'control.der')


def candidate_address(storage):
    # A public egress observation is only a candidate; it establishes no inbound mapping.
    request = urllib.request.Request('https://www.cloudflare.com/cdn-cgi/trace',
                                     headers={'User-Agent': 'Kehila-SSH-reachability-research'})
    with urllib.request.urlopen(request, timeout=5) as response:
        raw = response.read(4097)
    require(len(raw) <= 4096)
    (storage / 'egress-observation.txt').write_bytes(raw)
    fields = dict(line.split('=', 1) for line in raw.decode('ascii').splitlines() if '=' in line)
    return public_ipv4(fields['ip'])


@contextmanager
def creation_lock(storage):
    """Serialize engine creation with final removal; process death releases lock."""
    with (storage / 'creation.lock').open('a+b') as handle:
        if not handle.tell():
            handle.write(b'0')
            handle.flush()
        handle.seek(0)
        deadline = time.monotonic() + 35
        while True:
            try:
                if os.name == 'nt':
                    import msvcrt
                    msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
                else:
                    import fcntl
                    fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except (BlockingIOError, PermissionError):
                if time.monotonic() >= deadline:
                    raise TimeoutError('probe creation lock unavailable')
                time.sleep(0.05)
        try:
            yield
        finally:
            handle.seek(0)
            if os.name == 'nt':
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(handle, fcntl.LOCK_UN)


def write_owners(storage, owners):
    pending = storage / 'owners.pending'
    pending.write_text(json.dumps(owners))
    pending.replace(storage / 'owners.json')


def reserve_container(storage, suffix):
    """Persist ownership before invoking the engine, including ambiguous CLI failures."""
    binding = read_json((storage / 'binding.json').read_bytes())
    require(suffix in {'policy', 'control', 'live'} and re.fullmatch(r'[0-9]+', binding['run_id']))
    name = 'kehila-ssh-' + binding['run_id'] + '-' + suffix
    label = binding['run_id'] + ':' + binding['nonce']
    require(not run(['sudo', '-n', 'podman', 'ps', '-aq', '--filter', 'name=^' + name + '$'], storage).strip())
    manifest = storage / 'owners.json'
    owners = json.loads(manifest.read_bytes()) if manifest.exists() else []
    require(len(owners) < 3 and not any(o['name'] == name for o in owners))
    owners.append(dict(name=name, label=label, pending=True))
    write_owners(storage, owners)
    return ['--name', name, '--label', 'io.kehila.ssh-probe-owner=' + label]


def create_container(storage, suffix, options, image, command=()):
    with creation_lock(storage):
        require(not (storage / 'admission.closed').exists())
        identity = reserve_container(storage, suffix)
        result = run(['sudo', '-n', 'podman', 'run'] + options + identity + [image] + list(command), storage)
        owners = json.loads((storage / 'owners.json').read_bytes())
        owners[-1]['pending'] = False
        write_owners(storage, owners)
        return result


def remove_containers(storage):
    with creation_lock(storage):
        remove_containers_unlocked(storage)


def close_and_remove_containers(storage):
    # Close admission even if lock acquisition/engine cleanup subsequently fails.
    (storage / 'admission.closed').touch(exist_ok=True)
    remove_containers(storage)


def remove_containers_unlocked(storage):
    manifest = storage / 'owners.json'
    failures = []
    owners = json.loads(manifest.read_bytes()) if manifest.exists() else []
    for owner in owners:
        try:
            name, label = owner['name'], owner['label']
            require(re.fullmatch(r'kehila-ssh-[0-9]+-(policy|control|live)', name)
                    and re.fullmatch(r'[0-9]+:[0-9a-f]{64}', label))
            query = ['sudo', '-n', 'podman', 'ps', '-aq', '--no-trunc', '--filter', 'name=^' + name + '$',
                     '--filter', 'label=io.kehila.ssh-probe-owner=' + label]
            identities = run(query, storage).decode().split()
            require(len(identities) <= 1 and all(re.fullmatch(r'[0-9a-f]{12,64}', x) for x in identities))
            # An unacknowledged create with no observed resource may still be
            # pending in the engine. Never turn that uncertainty into final absence.
            require(identities or not owner.get('pending', False))
            owner['pending'] = False
            for identity in identities:
                run(['sudo', '-n', 'podman', 'rm', '--force', identity], storage, timeout=15)
            require(not run(query, storage).strip())
            owner['pending'] = False
        except Exception:
            failures.append('container_cleanup')
    if manifest.exists():
        write_owners(storage, owners)
    require(not failures)


def expiry_control(storage, image, mounts, binding):
    # A distinct disposable key exercises actual authentication without uploading
    # the admitted operator's private key or broadening the live source policy.
    run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-C', '',
         '-f', str(storage / 'control_key')], storage)
    key = public_key(' '.join((storage / 'control_key.pub').read_text().split()[:2]))
    (storage / 'control_authorized_keys').write_text(
        'from="127.0.0.1",restrict,port-forwarding ' + key + '\n')
    (storage / 'control_authorized_keys').chmod(0o444)
    host = public_key(' '.join((storage / 'host_key.pub').read_text().split()[:2]))
    (storage / 'control_known_hosts').write_text('[127.0.0.1]:2222 ' + host + '\n')
    (storage / 'control_known_hosts').chmod(0o444)
    run(['sudo', '-n', 'chown', '0:0', str(storage / 'control_key'),
         str(storage / 'control_authorized_keys')], storage)
    control_mounts = mounts[:2] + ['--volume', str(storage / 'control_authorized_keys') +
        ':/etc/ssh/probe_authorized_keys:ro', '--volume', str(storage / 'control_key') +
        ':/etc/ssh/probe_control_key:ro', '--volume', str(storage / 'control_known_hosts') +
        ':/etc/ssh/probe_control_known_hosts:ro']
    try:
        identity = create_container(storage, 'control', ['-d', '--network=none',
            '--memory=256m', '--memory-swap=256m', '--cpus=1', '--pids-limit=128',
            '--read-only', '--tmpfs', '/run:rw,nosuid,nodev,size=8m'] + control_mounts +
            ['--env', 'PROBE_NONCE=' + binding['nonce']], image,
            ['timeout', '--signal=KILL', '8', 'python3', '/opt/probe/service.py']).decode().strip()
        require(re.fullmatch(r'[0-9a-f]{64}', identity))
        command = ['sudo', '-n', 'podman', 'exec', '-i', identity, 'timeout', '--signal=KILL', '3',
            'ssh', '-F', 'none', '-T', '-i', '/etc/ssh/probe_control_key', '-p', '2222',
            '-o', 'BatchMode=yes', '-o', 'IdentitiesOnly=yes', '-o', 'IdentityAgent=none',
            '-o', 'StrictHostKeyChecking=yes', '-o', 'GlobalKnownHostsFile=none',
            '-o', 'UserKnownHostsFile=/etc/ssh/probe_control_known_hosts',
            '-o', 'HostKeyAlgorithms=ssh-ed25519', '-o', 'ConnectTimeout=2',
            '-W', '127.0.0.1:31337', 'probe@127.0.0.1']
        # Give the Python/sshd startup a bounded window before testing auth.
        run(['sudo', '-n', 'podman', 'exec', identity, 'python3', '-c',
            'import socket,time\n'
            'for attempt in range(20):\n'
            ' try: socket.create_connection(("127.0.0.1",2222),.1).close(); break\n'
            ' except OSError: time.sleep(.05)\n'
            'else: raise RuntimeError("SSH control not ready")'], storage, timeout=3)
        request = ('GET /' + binding['nonce'] + ' HTTP/1.0\r\nHost: synthetic\r\n\r\n').encode('ascii')
        response = run(command, storage, data=request, timeout=4)
        header, separator, body = response.partition(b'\r\n\r\n')
        require(separator and header.startswith(b'HTTP/1.0 200 ') and body == binding['nonce'].encode('ascii'))
        # Check the real process tree under the selected engine before public exposure.
        require(run(['sudo', '-n', 'podman', 'wait', identity], storage, timeout=12).strip() == b'137')
        state = json.loads(run(['sudo', '-n', 'podman', 'inspect', identity], storage))[0]['State']
        require(not state['Running'] and state['ExitCode'] == 137)
    finally:
        try:
            remove_containers(storage)
        finally:
            (storage / 'control_key').unlink(missing_ok=True)


def admit_mount_ownership(storage):
    # Rootful bind mounts preserve host UIDs. StrictModes requires root or the
    # probe account to own authorized_keys; the runner user's UID is neither.
    run(['sudo', '-n', 'chown', '0:0', str(storage / 'authorized_keys'),
         str(storage / 'host_key')], storage)
    for name, mode in (('authorized_keys', 0o444), ('host_key', 0o600)):
        state = (storage / name).stat()
        require(state.st_uid == 0 and state.st_gid == 0 and state.st_mode & 0o777 == mode)


def serve(storage):
    """Own the container until expiry; container PID 1 also has its own lease."""
    binding = read_json((storage / 'binding.json').read_bytes())
    require(binding == source_binding(binding))
    (storage / 'stage').write_text('candidate')
    address = candidate_address(storage)
    engine = ['sudo', '-n', 'podman']
    admit_mount_ownership(storage)
    (storage / 'stage').write_text('image')
    image_id = storage / 'image-id'
    require(not image_id.exists())
    output = run(engine + ['build', '--quiet', '--iidfile', str(image_id), '--memory=1g', '--cpu-period=100000',
        '--cpu-quota=100000', '-f', str(Path(__file__).with_name('Containerfile')),
        str(Path(__file__).parent)], storage, timeout=240)
    (storage / 'image-build.log').write_bytes(output)
    image = run(['sudo', '-n', 'cat', str(image_id)], storage).decode().strip()
    require(re.fullmatch(r'(sha256:)?[0-9a-f]{64}', image))
    # Effective policy is checked before the container publishes a host port.
    (storage / 'stage').write_text('policy')
    mounts = ['--volume', str(storage / 'host_key') + ':/etc/ssh/probe_host_key:ro',
              '--volume', str(storage / 'authorized_keys') + ':/etc/ssh/probe_authorized_keys:ro']
    policy = create_container(storage, 'policy', ['--rm', '--network=none', '--read-only',
                 '--memory=256m', '--memory-swap=256m', '--cpus=1', '--pids-limit=128'] + mounts,
                 image, ['/usr/sbin/sshd', '-T', '-f', '/etc/ssh/probe_config'])
    (storage / 'effective-sshd.txt').write_bytes(policy)
    required = (b'passwordauthentication no', b'kbdinteractiveauthentication no',
                b'allowtcpforwarding local', b'maxsessions 0',
                b'permitopen 127.0.0.1:31337', b'permitlisten none')
    require(all(line in policy.splitlines() for line in required))
    (storage / 'stage').write_text('expiry-control')
    expiry_control(storage, image, mounts, binding)
    container = None
    started = time.monotonic()
    deadline = int(time.time()) + LEASE
    result = dict(owner_succeeded=False, container_started=False, descriptor_ready=False,
                  container_removed=False, residual_check=False)
    try:
        (storage / 'stage').write_text('listener')
        container = create_container(storage, 'live', ['--detach', '--read-only',
            '--memory=256m', '--memory-swap=256m', '--cpus=1', '--pids-limit=128',
            '--tmpfs', '/run:rw,nosuid,nodev,size=8m',
            '--publish', '0.0.0.0:' + str(PORT) + ':2222'] + mounts +
            ['--env', 'PROBE_NONCE=' + binding['nonce']], image).decode().strip()
        require(re.fullmatch(r'[0-9a-f]{64}', container))
        result['container_started'] = True
        # Inspect only our owned container; no address or inspection output enters logs.
        inspection = run(engine + ['inspect', container], storage)
        (storage / 'container-inspect.json').write_bytes(inspection)
        effective = json.loads(inspection)[0]
        require(effective['State']['Running'])
        mapping = effective['NetworkSettings']['Ports']['2222/tcp']
        require(mapping == [{'HostIp': '0.0.0.0', 'HostPort': str(PORT)}])
        # Wait for the contained loopback target AND SSH listener without public traffic.
        run(engine + ['exec', container, 'python3', '-c',
            'import socket,time\n'
            'for port in (2222,31337):\n'
            ' for attempt in range(20):\n'
            '  try: socket.create_connection(("127.0.0.1",port),.2).close(); break\n'
            '  except OSError: time.sleep(.1)\n'
            ' else: raise RuntimeError("listener not ready")'], storage, timeout=8)
        host_key = public_key(' '.join((storage / 'host_key.pub').read_text().split()[:2]))
        (storage / 'stage').write_text('descriptor')
        descriptor = dict(binding, address=address, port=PORT, host_public_key=host_key,
                          deadline=deadline)
        (storage / 'descriptor.json').write_text(json.dumps(descriptor))
        encrypt(storage, storage / 'descriptor.json', storage / 'descriptor.pending')
        (storage / 'descriptor.pending').replace(storage / 'descriptor.der')
        result['descriptor_ready'] = True
        (storage / 'stage').write_text('lease')
        while time.monotonic() - started < LEASE:
            time.sleep(max(0, min(0.25, LEASE - (time.monotonic() - started))))
        result['owner_succeeded'] = True
    except Exception as error:
        result['failure_stage'] = (storage / 'stage').read_text()
        result['failure_class'] = type(error).__name__
        raise
    finally:
        (storage / 'stage').write_text('cleanup')
        try:
            if container:
                logs = run(engine + ['logs', container], storage, timeout=10)
                (storage / 'container.log').write_bytes(logs)
                result['synthetic_responses'] = logs.splitlines().count(b'synthetic_response_completed')
        finally:
            try:
                remove_containers(storage)
                result['container_removed'] = True
                result['residual_check'] = True
            finally:
                write_result(storage, result)
        require(result['container_removed'] and result['residual_check'])


def cleanup(storage):
    """Job-finally fallback; only a recorded container and this run's named artifacts."""
    failures = []
    try:
        close_and_remove_containers(storage)
    except Exception:
        failures.append('container_cleanup')
    try:
        delete_artifacts()
    except Exception:
        failures.append('artifact_cleanup')
    print(json.dumps(dict(container_cleanup='container_cleanup' not in failures,
                          artifact_cleanup='artifact_cleanup' not in failures)))
    require(not failures)


def delete_artifacts():
    token = os.environ['GH_TOKEN']
    repository = os.environ['GITHUB_REPOSITORY']
    run_id = os.environ['GITHUB_RUN_ID']
    require(repository == 'MrQuality/kehila' and re.fullmatch(r'[0-9]+', run_id))

    def api(path, method='GET'):
        request = urllib.request.Request('https://api.github.com/repos/' + repository + path,
            headers={'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
                     'X-GitHub-Api-Version': '2022-11-28'}, method=method)
        with urllib.request.urlopen(request, timeout=15) as response:
            return response.read(1024 * 1024)

    path = '/actions/runs/' + run_id + '/artifacts?per_page=100'
    artifacts = json.loads(api(path))
    require(artifacts['total_count'] <= 100)
    names = {'ssh-probe-control', 'ssh-probe-endpoint'}
    for artifact in artifacts['artifacts']:
        if artifact['name'] in names:
            api('/actions/artifacts/' + str(int(artifact['id'])), 'DELETE')
    require(not any(a['name'] in names for a in json.loads(api(path))['artifacts']))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('prepare', 'serve', 'wait', 'finish', 'cleanup'))
    parser.add_argument('storage', type=Path)
    args = parser.parse_args()
    storage = args.storage.resolve()
    root = Path(__file__).resolve().parents[2]
    require(storage != root and root not in storage.parents)
    if args.mode == 'prepare':
        prepare(storage)
    elif args.mode == 'serve':
        serve(storage)
    elif args.mode == 'cleanup':
        cleanup(storage)
    elif args.mode == 'finish':
        for _ in range(130):
            if (storage / 'result.json').exists():
                result = read_json((storage / 'result.json').read_bytes())
                print(json.dumps(result, sort_keys=True))
                require(result['owner_succeeded'] and result['descriptor_ready'] and result['container_removed']
                        and result['residual_check'])
                return
            time.sleep(3)
        raise TimeoutError('probe owner result unavailable')
    else:
        for _ in range(100):
            if (storage / 'descriptor.der').exists():
                return
            if (storage / 'result.json').exists():
                print((storage / 'result.json').read_text())
                raise ValueError('probe stopped before descriptor')
            time.sleep(3)
        raise TimeoutError('descriptor unavailable')


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        if len(sys.argv) == 3 and sys.argv[1] == 'serve':
            storage = Path(sys.argv[2])
            result_path = storage / 'result.json'
            result = read_json(result_path.read_bytes()) if result_path.exists() else dict(
                owner_succeeded=False, container_started=False, descriptor_ready=False,
                container_removed=False, residual_check=False)
            result.setdefault('failure_class', type(error).__name__)
            result.setdefault('failure_stage', (storage / 'stage').read_text()
                              if (storage / 'stage').exists() else 'admission')
            write_result(storage, result)
        # Never expose paths, addresses, command args or exception text to Actions.
        print('Synthetic SSH probe stopped; confidential diagnostics retained locally.', file=sys.stderr)
        sys.exit(1)
