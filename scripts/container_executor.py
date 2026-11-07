"""Own an isolated toolchain runner; engine control stays outside the container."""
from contextlib import contextmanager
import ipaddress
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import uuid

from cleanup_errors import CleanupStack
from test_services import run, _account_directory, resource_lease
from container_cache import DependencyCache
from verification_budget import command_timeout, cleanup_time
from temp_work import owned_directory
from verification_settings import immutable_image as image_reference

LABEL = 'org.kehila.verification-owner'


def one_object(output):
    rows = json.loads(output)
    if not isinstance(rows, list) or len(rows) != 1 or not isinstance(rows[0], dict):
        raise ValueError('Expected one inspected engine resource')
    return rows[0]


def inspected_image_id(value):
    match = re.fullmatch(r'(?:sha256:)?([0-9a-f]{64})', value or '')
    if match is None:
        raise ValueError('Expected immutable toolchain image identity')
    return 'sha256:' + match.group(1)


class ContainerExecutor:
    def __init__(self, runtime, settings, image, source, *, owner=None,
                 dependencies=False, preparing=False):
        self.runtime = runtime
        self.settings = settings
        self.image = image_reference(image)
        self.source = Path(source).resolve()
        self.owner = uuid.uuid4().hex if owner is None else owner
        if not re.fullmatch(r'[0-9a-f]{32}', self.owner):
            raise ValueError('Invalid verification resource owner')
        self.name = 'kehila-verify-' + self.owner
        self.build_network = self.name + '-build'
        self.container = None
        self.removed = False
        self.dependencies = dependencies
        self.preparing = preparing
        self.cache = None
        self.build_network_detached = False

    def engine(self, *arguments, capture=False, timeout=120):
        return run([*self.runtime.engine, *arguments], timeout=timeout,
                   env=self.runtime.environment, cwd=_account_directory(),
                   capture_output=capture, text=capture)

    def inspect_container(self, identity):
        value = one_object(self.engine('inspect', '--type', 'container', identity, capture=True).stdout)
        if value.get('Id') != identity:
            raise ValueError('Inspected container identity changed')
        return value

    def cpu_limit_matches(self, limits):
        nano = limits.get('NanoCpus', 0)
        if nano:
            return nano == int(self.settings['runner_cpus'] * 10**9)
        period, quota = limits.get('CpuPeriod', 0), limits.get('CpuQuota', 0)
        return period > 0 and quota == self.settings['runner_cpus'] * period

    def copy_source(self):
        manifest = {}
        for path in self.source.rglob('*'):
            if path.is_symlink():
                raise ValueError('Symlinks are unsupported in the source snapshot')
            if path.is_file():
                manifest[path.relative_to(self.source).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
        with owned_directory('kehila-source-manifest-') as directory:
            file = directory / 'source.json'
            file.write_text(json.dumps(manifest, sort_keys=True), encoding='utf-8')
            self.engine('cp', str(self.source) + '/.', self.container + ':/workspace')
            self.engine('cp', str(file), self.container + ':/work/source.json')
        self.verify_source()

    def verify_source(self):
        self.execute(['python', 'scripts/container_source.py', '/work/source.json',
                      str(self.settings['runner_memory_mib'] * 1024**2),
                      str(self.settings['runner_pids']), str(self.settings['runner_cpus'])])

    def remove(self):
        if self.removed:
            return
        with cleanup_time():
            # Inventory also covers partial creation that did not return an ID.
            identities = self.engine('ps', '-a', '--filter', 'label=' + LABEL + '=' + self.owner,
                                     '--format', '{{.ID}}', capture=True).stdout.split()
            for identity in identities:
                inspected = one_object(self.engine('inspect', '--type', 'container', identity,
                                                  capture=True).stdout)
                if inspected.get('Config', {}).get('Labels', {}).get(LABEL) != self.owner:
                    raise ValueError('Refusing removal of an unowned runner')
                self.engine('rm', '--force', '--volumes', inspected['Id'],
                            timeout=self.settings['runner_stop_seconds'])
            if self.engine('ps', '-a', '--filter', 'label=' + LABEL + '=' + self.owner,
                           '--format', '{{.ID}}', capture=True).stdout.strip():
                raise RuntimeError('Owned verification runner remains after cleanup')
            self.removed = True

    def remove_build_network(self):
        with cleanup_time():
            names = self.engine('network', 'ls', '--filter', 'label=' + LABEL + '=' + self.owner,
                                '--format', '{{.Name}}', capture=True).stdout.split()
            for name in names:
                if name != self.build_network:
                    raise ValueError('Refusing removal of an unexpected network')
                self.engine('network', 'rm', name, timeout=self.settings['runner_stop_seconds'])
            if self.engine('network', 'ls', '--filter', 'label=' + LABEL + '=' + self.owner,
                           '--format', '{{.Name}}', capture=True).stdout.strip():
                raise RuntimeError('Owned build network remains after cleanup')

    @contextmanager
    def owned(self):
        if (self.engine('ps', '-a', '--filter', 'label=' + LABEL + '=' + self.owner,
                        '--format', '{{.ID}}', capture=True).stdout.strip()
                or self.engine('network', 'ls', '--filter', 'label=' + LABEL + '=' + self.owner,
                               '--format', '{{.Name}}', capture=True).stdout.strip()):
            raise ValueError('Verification resource owner is already in use')
        with CleanupStack() as cleanup:
            inspected = one_object(self.engine('image', 'inspect', self.image, capture=True).stdout)
            identity = inspected_image_id(inspected.get('Id'))
            if inspected.get('Os') != 'linux' or inspected.get('Architecture') != 'amd64':
                raise ValueError('The verification toolchain must be a Linux amd64 image')
            print('VERIFICATION IMAGE:', identity, flush=True)
            mounts = []
            cache_mountpoint = None
            if self.dependencies:
                self.cache = DependencyCache.for_source(self.source, identity)
                cleanup.enter_context(resource_lease('dependency-cache:' + self.cache.key,
                                                     description='dependency cache'),
                                      resource='dependency-cache lease')
                cache_mountpoint = self.cache.admit(self.engine, prepare=self.preparing)
                mounts = ['--mount', 'type=volume,source=' + self.cache.name + ',target=/work/cargo',
                          '--env', 'GOMODCACHE=/work/cargo/go-mod']
                if not self.preparing:
                    mounts += ['--env', 'CARGO_NET_OFFLINE=true', '--env', 'GOPROXY=off']
            # Remove workloads before network cleanup and releasing the cache lease.
            cleanup.callback(self.remove_build_network, resource='verification build network')
            cleanup.callback(self.remove, resource='verification runner')
            self.engine('network', 'create', '--label', LABEL + '=' + self.owner, self.build_network)
            output = self.engine('create', '--pull=never', '--platform=linux/amd64', '--name', self.name,
                                 '--label', LABEL + '=' + self.owner, '--network', self.build_network,
                                 '--memory', str(self.settings['runner_memory_mib']) + 'm',
                                 '--cpus', str(self.settings['runner_cpus']),
                                 '--pids-limit', str(self.settings['runner_pids']),
                                 '--cap-drop=ALL', '--security-opt=no-new-privileges',
                                 '--user', '10001:10001', '--workdir', '/workspace',
                                 *mounts, self.image, capture=True).stdout.strip()
            if not re.fullmatch(r'[0-9a-f]{64}', output):
                raise ValueError('Expected the full owned runner ID')
            self.container = output
            state = self.inspect_container(output)
            config = state.get('Config', {})
            limits = state.get('HostConfig', {})
            if (config.get('Labels', {}).get(LABEL) != self.owner
                    or config.get('User') != '10001:10001'
                    or limits.get('Privileged') is not False
                    or limits.get('Memory') != self.settings['runner_memory_mib'] * 1024**2
                    or limits.get('PidsLimit') != self.settings['runner_pids']
                    or not self.cpu_limit_matches(limits)
                    or (not self.dependencies and limits.get('Binds') not in (None, []))
                    or limits.get('Devices') not in (None, [])):
                raise ValueError('Effective verification runner identity/resource limits mismatch')
            effective_mounts = state.get('Mounts', [])
            if self.dependencies:
                if (len(effective_mounts) != 1
                        or effective_mounts[0].get('Type') != 'volume'
                        or effective_mounts[0].get('Name') != self.cache.name
                        or effective_mounts[0].get('Destination') != '/work/cargo'
                        or effective_mounts[0].get('Source') != cache_mountpoint
                        or effective_mounts[0].get('RW') is not True):
                    raise ValueError('Effective dependency-cache mount differs from owned volume')
            elif effective_mounts:
                raise ValueError('Unexpected verification runner mount')
            # Copy the exported index; never mount the Windows checkout or engine socket.
            self.engine('start', self.container)
            self.copy_source()
            yield self

    def execute(self, command, environment=None, *, limit=None):
        seconds = command_timeout(self.settings['command_seconds'] if limit is None else limit)
        command = ['python' if value == sys.executable else str(value) for value in command]
        arguments = ['exec']
        allowed = {'KEHILA_NATS_HOST', 'KEHILA_NATS_PORT', 'KEHILA_NATS_ADDRESS'}
        for key, value in (environment or {}).items():
            if key in allowed:
                arguments += ['--env', key + '=' + value]
        arguments += [self.container, 'python', 'scripts/container_command.py', str(seconds), *command]
        try:
            self.engine(*arguments, timeout=seconds + self.settings['runner_stop_seconds'])
        except BaseException as primary:
            # Destroy all descendant workloads before the surrounding group unwinds.
            try:
                self.remove()
            except BaseException as cleanup:
                from cleanup_errors import retain_cleanup_failure
                retain_cleanup_failure(primary, cleanup, 'failed verification runner')
            raise

    def finish_builds(self):
        if not self.build_network_detached:
            self.engine('network', 'disconnect', self.build_network, self.container)
            self.build_network_detached = True

    @contextmanager
    def service_network(self, containers):
        network_name = None
        project = None
        addresses = {}
        for service, identity in containers.items():
            state = self.inspect_container(identity)
            candidate = state.get('Config', {}).get('Labels', {}).get('com.docker.compose.project')
            networks = state.get('NetworkSettings', {}).get('Networks', {})
            if not isinstance(candidate, str) or not candidate.startswith('kehila-test-') or len(networks) != 1:
                raise ValueError('Expected one owned Compose service network')
            name, attachment = next(iter(networks.items()))
            if project is not None and (candidate != project or name != network_name):
                raise ValueError('Service group network identity mismatch')
            project, network_name = candidate, name
            address = ipaddress.ip_address(attachment.get('IPAddress', ''))
            if address.version != 4 or address.is_loopback or address.is_unspecified:
                raise ValueError('Expected an attached service IPv4 address')
            addresses[service] = str(address)
        network = one_object(self.engine('network', 'inspect', network_name, capture=True).stdout)
        labels = network.get('Labels', network.get('labels', {})) or {}
        if labels.get('com.docker.compose.project') != project:
            raise ValueError('Network does not belong to this verified service group')
        network_id = network.get('Id', network.get('id'))
        if not isinstance(network_id, str) or not re.fullmatch(r'[0-9a-f]{64}', network_id):
            raise ValueError('Expected immutable service network ID')
        def disconnect():
            if not self.removed:
                with cleanup_time():
                    self.engine('network', 'disconnect', network_id, self.container)
        with CleanupStack() as cleanup:
            cleanup.callback(disconnect, resource='verification service network attachment')
            self.engine('network', 'connect', network_id, self.container)
            yield addresses
