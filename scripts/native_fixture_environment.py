"""Own one isolated native database lifetime; qualification belongs to callers."""
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time

from check_postgresql import IMAGE
from cleanup_errors import retain_cleanup_failure
from container_executor import LABEL, one_object
from verification_budget import cleanup_time, command_timeout
from native_authority_fixture import (AUTH_DATABASES, ITEM_DATABASES, ITEM_OBSERVERS,
    CAPACITY_DATABASE, CAPACITY_OBSERVER, STORAGE_DATABASE, STORAGE_HISTORICAL_DATABASE,
    STORAGE_OBSERVER)
from native_fixture_assets import write_owner, write_manifest, verify_export, MANIFEST_FILE
from native_database_limits import DatabaseLimits, DEFAULT_LIMITS


def require(condition, message):
    if not condition:
        raise ValueError(message)


def admit_runner(info, identity, owner):
    host, config = info['HostConfig'], info['Config']
    require(info['Id'] == identity and config.get('Labels', {}).get(LABEL) == owner,
            'Native runner ownership mismatch')
    require(info['State']['Running'] and config['User'] == '10001:10001',
            'Native runner must be running as the selected user')
    require(host['NetworkMode'] in {'none', 'bridge'} and not host.get('PortBindings')
            and not info.get('NetworkSettings', {}).get('Networks'),
            'Native runner must have no attached network or published ports')


def admit_owned_database(info, identity, owner, name):
    require(re.fullmatch(r'[0-9a-f]{64}', identity) is not None
            and info['Id'] == identity and info['Name'].lstrip('/') == name
            and info['Config'].get('Labels', {}).get(LABEL) == owner,
            'Native database ownership mismatch')


def admit_database(info, identity, runner, owner, name, *, limits=DEFAULT_LIMITS):
    admit_owned_database(info, identity, owner, name)
    host, config = info['HostConfig'], info['Config']
    require(config['Image'] == IMAGE, 'Native database image mismatch')
    cpu = (host['NanoCpus'] == 10**9 if host.get('NanoCpus') else
           host.get('CpuPeriod', 0) > 0 and host.get('CpuQuota') == host['CpuPeriod'])
    require(not host['Privileged'] and not host.get('Binds') and not info.get('Mounts')
            and not host.get('Devices') and not host.get('PortBindings')
            and host['NetworkMode'] == 'container:' + runner,
            'Native database isolation mismatch')
    require(host['Memory'] == limits.memory_bytes and host['PidsLimit'] == 128 and cpu,
            'Native database limits mismatch')
    tmpfs = host.get('Tmpfs', {})
    require(set(tmpfs) == {'/var/lib/postgresql/data'}
            and f'size={limits.mib}m' in tmpfs['/var/lib/postgresql/data'].split(','),
            'Native database must use bounded ephemeral storage')


class NativeEnvironment:
    """Resource/evidence ownership without selecting or qualifying a workload.

    A long-lived caller must finish its child, final state observations and joins
    before leaving the body. A release_dependents callback can enforce this
    boundary during cleanup. Its failure withholds database removal and export;
    the outer runner owner must also refuse removal while the database remains.
    The context removes its database before the caller can remove the runner.
    """
    def __init__(self, executor, *, databases, observers, selection,
                 evidence_directory=None, reset_assets=False, limits=DEFAULT_LIMITS,
                 failure_observer=None, release_dependents=None):
        require(type(databases) is tuple and databases and len(set(databases)) == len(databases),
                'Select distinct closed native databases')
        known_databases = {*AUTH_DATABASES.values(), *ITEM_DATABASES.values(),
                          CAPACITY_DATABASE, STORAGE_DATABASE, STORAGE_HISTORICAL_DATABASE}
        require(all(type(value) is str and value in known_databases for value in databases),
                'Unknown native database')
        known_observers = {*ITEM_OBSERVERS.values(), CAPACITY_OBSERVER, STORAGE_OBSERVER}
        require(type(observers) is tuple and all(type(value) is str and value in known_observers
                for value in observers), 'Unknown native observer')
        require(isinstance(limits, DatabaseLimits), 'Closed database resource profile required')
        require(type(selection) is dict and isinstance(selection.get('scope'), str)
                and type(selection.get('selected_items')) is list
                and type(selection.get('observed_items')) is list,
                'Explicit fixture selection record required')
        require(failure_observer is None or callable(failure_observer), 'Invalid failure observer')
        require(release_dependents is None or callable(release_dependents), 'Invalid dependent-release gate')
        require(type(reset_assets) is bool, 'Fixture reset selection must be boolean')
        require(not reset_assets or evidence_directory is not None,
                'Generated fixture reset requires complete external evidence retention')
        self.executor, self.databases, self.observers = executor, databases, observers
        self.selection, self.limits = selection, limits
        self.reset_assets, self.failure_observer = reset_assets, failure_observer
        self.release_dependents = release_dependents
        self.cleanup_withheld = False
        self.destination = None
        if evidence_directory is not None:
            destination = Path(evidence_directory)
            require(destination.is_absolute() and not destination.exists()
                    and destination.resolve() == destination
                    and destination != executor.source and executor.source not in destination.parents
                    and destination not in executor.source.parents and destination.parent.is_dir(),
                    'Native evidence destination must be fresh and outside the source snapshot')
            require(not any((parent / '.git').exists() for parent in (destination, *destination.parents)),
                    'Native evidence destination must be outside Git checkouts')
            self.destination = destination
        self.name = executor.name + '-native-db'
        self.stage, self.attempted, self.failure = 'prepare', False, None
        self.folder = self.root = self.identity = self.receipt = self.runner_info = None
        self._used = False

    def _start(self):
        require(self.executor.build_network_detached, 'Disconnect build networking before native verification')
        self.runner_info = self.executor.inspect_container(self.executor.container)
        admit_runner(self.runner_info, self.executor.container, self.executor.owner)
        self.root = Path(tempfile.gettempdir()).resolve()
        require(self.root != self.executor.source and self.executor.source not in self.root.parents,
                'Native diagnostic storage must be outside the source snapshot')
        require(not any((parent / '.git').exists() for parent in (self.root, *self.root.parents)),
                'Native diagnostic storage must be outside Git checkouts')
        self.folder = Path(tempfile.mkdtemp(prefix='kehila-native-diagnostics-', dir=self.root)).resolve()
        write_owner(self.folder, self.executor.owner)
        (self.folder / 'selection.json').write_text(json.dumps(self.selection), encoding='utf-8')
        print('NATIVE SCOPE:', self.selection['scope'], ', '.join(self.selection['selected_items']), flush=True)

    def control(self, *args, both_streams=False):
        try:
            result = self.executor.engine(*args, capture=True)
            return result.stdout + ((getattr(result, 'stderr', '') or '') if both_streams else '')
        except subprocess.CalledProcessError as error:
            (self.folder / (self.stage + '-failure.txt')).write_text(
                (error.stdout or '') + (error.stderr or ''), encoding='utf-8')
            raise RuntimeError('Native fixture command failed at ' + self.stage) from None

    def work(self, command):
        # This child must release its dependent DB before the parent destroys
        # the runner. Use the same cooperative container deadline wrapper;
        # do not invoke execute()'s immediate parent-runner teardown here.
        seconds = command_timeout(self.executor.settings['command_seconds'])
        self.executor.engine('exec', self.executor.container, 'python', 'scripts/container_command.py',
            str(seconds), *command, timeout=seconds + self.executor.settings['runner_stop_seconds'])

    def psql(self, file, database='postgres', *, tuples=False):
        return self.control('exec', self.name, 'psql', '-X', '-U', 'postgres', '-d', database,
                       *(['-q', '-A', '-t'] if tuples else []), '-v', 'ON_ERROR_STOP=1', '-f', file)

    def retain_failure(self, error, location, evidence_name):
        if self.folder is None:
            return
        try:
            (self.folder / 'failure-stage.txt').write_text(location, encoding='ascii')
        except BaseException as metadata_error:
            retain_cleanup_failure(error, metadata_error, 'native failure-stage retention')
        try:
            with cleanup_time():
                self.control('cp', self.executor.container + ':/work/native-authority', str(self.folder / evidence_name))
        except BaseException as evidence_error:
            retain_cleanup_failure(error, evidence_error, 'native diagnostic collection')

    def _release_dependents(self):
        if self.release_dependents is None:
            return
        release_error = None
        try:
            # Required release must remain possible after the work deadline.
            # The callback owns finite control deadlines and raises on doubt.
            with cleanup_time():
                self.release_dependents(self, self.failure)
        except BaseException as error:
            release_error = error
        if release_error is None:
            return
        self.cleanup_withheld = True
        if self.failure is None:
            self.failure = release_error
            self.retain_failure(release_error, 'dependent-release', 'runner-evidence')
        else:
            retain_cleanup_failure(self.failure, release_error, 'native dependent release')
            # Preserve the original failure stage and evidence. This second
            # observation may still work after a forced stop of the runner.
            if self.folder is not None and self.attempted:
                try:
                    with cleanup_time():
                        self.control('cp', self.executor.container + ':/work/native-authority',
                                     str(self.folder / 'runner-evidence-after-release-failure'))
                except BaseException as diagnostic:
                    retain_cleanup_failure(self.failure, diagnostic, 'native release diagnostic collection')
        raise self.failure

    def _prepare(self):
        self.work(['python', '-c', "from pathlib import Path; assert {line.split(':')[0].strip() "
              "for line in Path('/proc/net/dev').read_text().splitlines() if ':' in line} == {'lo'}"])
        self.work(['python', 'scripts/native_authority_fixture.py', 'prepare', '--seconds',
              str(self.executor.settings['command_seconds']), '--asset-owner', self.executor.owner])
        self.control('cp', self.executor.container + ':/work/native-authority/postgres', str(self.folder / 'postgres'))
        self.stage, self.attempted = 'database-create', True
        identity = self.control('create', '--pull=never', '--name', self.name, '--label', LABEL + '=' + self.executor.owner,
            '--network', 'container:' + self.executor.container, f'--memory={self.limits.mib}m', '--cpus=1', '--pids-limit=128',
            '--tmpfs', f'/var/lib/postgresql/data:rw,size={self.limits.mib}m',
            '--env', 'POSTGRES_PASSWORD_FILE=/tmp/kehila-native/admin-password',
            '--env', 'POSTGRES_HOST_AUTH_METHOD=scram-sha-256', '--entrypoint', '/bin/sh', IMAGE,
            '-c', 'set -eu; chmod 700 /tmp/kehila-native; chmod 600 /tmp/kehila-native/*; '
            'chown -R postgres:postgres /tmp/kehila-native; '
            'exec /usr/local/bin/docker-entrypoint.sh postgres -c max_connections=8 '
            '-c shared_buffers=16MB -c ssl=on -c ssl_min_protocol_version=TLSv1.3 '
            '-c ssl_cert_file=/tmp/kehila-native/server.pem '
            '-c ssl_key_file=/tmp/kehila-native/server-key.pem -c listen_addresses=127.0.0.1').strip()
        self.identity = identity
        info = one_object(self.control('inspect', '--type', 'container', identity))
        admit_database(info, identity, self.executor.container, self.executor.owner, self.name, limits=self.limits)
        host = info['HostConfig']
        (self.folder / 'execution-profile.json').write_text(json.dumps({
            'schema': 1, 'owner': self.executor.owner, 'runner_identity': self.executor.container,
            'runner_image': self.runner_info.get('Image', self.runner_info['Config'].get('Image')),
            'runner_memory_bytes': self.runner_info['HostConfig'].get('Memory'),
            'runner_pids': self.runner_info['HostConfig'].get('PidsLimit'),
            'runner_nano_cpus': self.runner_info['HostConfig'].get('NanoCpus', 0),
            'runner_cpu_period': self.runner_info['HostConfig'].get('CpuPeriod', 0),
            'runner_cpu_quota': self.runner_info['HostConfig'].get('CpuQuota', 0),
            'database_identity': identity, 'database_image': info['Config']['Image'],
            'database_memory_bytes': host['Memory'], 'database_pids': host['PidsLimit'],
            'database_nano_cpus': host.get('NanoCpus', 0),
            'database_cpu_period': host.get('CpuPeriod', 0),
            'database_cpu_quota': host.get('CpuQuota', 0), 'database_tmpfs': host['Tmpfs'],
        }, sort_keys=True), encoding='utf-8')
        self.control('cp', str(self.folder / 'postgres'), identity + ':/tmp/kehila-native')
        self.control('cp', str(self.executor.source / 'storage/postgresql'), identity + ':/tmp/kehila-schema')
        self.control('cp', str(self.executor.source / 'tests/integration/native_authority_state.sql'),
                identity + ':/tmp/native-authority-state.sql')
        self.control('cp', str(self.executor.source / 'tests/integration/native_clock_state.sql'),
                identity + ':/tmp/native-clock-state.sql')
        for file in ('native_copy_snapshot.sql', 'native_restored_state.sql', 'native_auth_fixture_state.sql', *self.observers):
            self.control('cp', str(self.executor.source / 'tests/integration' / file), identity + ':/tmp/' + file)
        self.stage = 'database-start'
        self.control('start', identity)
        ready_by = time.monotonic() + command_timeout(self.executor.settings['command_seconds'])
        while True:
            info = one_object(self.control('inspect', '--type', 'container', identity))
            require(info['State']['Running'], 'Native database exited at startup')
            if self.control('exec', identity, 'sh', '-c',
                'if pg_isready -h 127.0.0.1 -p 5432 -q -U postgres; then echo READY; else echo WAIT; fi').strip() == 'READY':
                break
            require(time.monotonic() < ready_by, 'Native database readiness allowance expired')
            time.sleep(0.1)
        self.stage = 'database-install'
        self.control('exec', identity, 'sh', '-c',
            f'set -eu; test "$(cat /sys/fs/cgroup/memory.max)" = {self.limits.memory_bytes}; '
            'test "$(cat /sys/fs/cgroup/pids.max)" = 128; '
            'set -- $(cat /sys/fs/cgroup/cpu.max); test "$1" != max; test "$1" = "$2"')
        self.psql('/tmp/kehila-schema/install_product.sql')
        self.psql('/tmp/kehila-native/setup.sql')
        self.stage = 'pristine-isolated-fixtures'
        baseline = "DO $$ BEGIN IF EXISTS (SELECT FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid()) OR EXISTS (SELECT FROM kehila.projects) OR (SELECT runtime_generation FROM kehila_auth.installation)<>0 OR (SELECT auth_generation<>1 OR verifier_revision<>1 FROM kehila_auth.owner_credentials) THEN RAISE EXCEPTION 'pristine quiescent fixture required'; END IF; END $$;"
        self.control('exec', identity, 'psql', '-X', '-U', 'postgres', '-v', 'ON_ERROR_STOP=1', '-c', baseline)
        for database in self.databases:
            self.control('exec', identity, 'createdb', '-U', 'postgres', '--owner=kehila_owner',
                    '--template=postgres', database)
            admission = "DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_database WHERE datname=current_database() AND datdba='kehila_owner'::regrole AND datallowconn AND NOT datistemplate AND pg_encoding_to_char(encoding)='UTF8') OR NOT has_database_privilege('kehila_auth_runtime_login',current_database(),'CONNECT') OR EXISTS (SELECT FROM pg_db_role_setting WHERE setdatabase=(SELECT oid FROM pg_database WHERE datname=current_database())) THEN RAISE EXCEPTION 'authentication database profile mismatch'; END IF; END $$;"
            self.control('exec', identity, 'psql', '-X', '-U', 'postgres', '-d', database,
                    '-v', 'ON_ERROR_STOP=1', '-c', admission + baseline)

    @contextmanager
    def owned(self):
        require(not self._used, 'A native environment can be acquired only once')
        self._used = True
        try:
            self._start()
            self._prepare()
            yield self
        except BaseException as error:
            self.failure = error
            if self.attempted and self.failure_observer is not None:
                try:
                    self.failure_observer(error, self)
                except BaseException as diagnostic:
                    retain_cleanup_failure(error, diagnostic, 'native failure observation')
            self.retain_failure(error, self.stage, 'runner-evidence')
            raise
        finally:
            # A failed release gate deliberately bypasses every destructive
            # cleanup/export/reset below. It never converts a failure to PASS.
            self._release_dependents()
            try:
                if self.attempted:
                    with cleanup_time():
                        ids = self.control('ps', '-a', '--filter', 'label=' + LABEL + '=' + self.executor.owner,
                                      '--filter', 'name=^' + self.name + '$', '--no-trunc', '--format', '{{.ID}}').split()
                        for identity in ids:
                            info = one_object(self.control('inspect', '--type', 'container', identity))
                            # A rejected execution profile does not revoke ownership
                            # of a container this invocation must still remove.
                            admit_owned_database(info, identity, self.executor.owner, self.name)
                            try:
                                (self.folder / 'postgres.log').write_text(
                                    self.control('logs', '--timestamps', identity, both_streams=True), encoding='utf-8')
                            finally:
                                self.executor.engine('rm', '--force', '--volumes', identity,
                                                timeout=self.executor.settings['runner_stop_seconds'])
            except BaseException as error:
                if self.failure is not None:
                    retain_cleanup_failure(self.failure, error, 'native PostgreSQL')
                else:
                    self.failure = error
                    self.retain_failure(error, 'owned-database-cleanup', 'runner-evidence')
                    raise
            finally:
                if self.failure is None:
                    try:
                        if self.destination is not None:
                            self.stage = 'evidence-manifest'
                            self.work(['python', 'scripts/native_authority_fixture.py', 'manifest', '--seconds',
                                  str(self.executor.settings['command_seconds']), '--asset-owner', self.executor.owner])
                            self.control('cp', self.executor.container + ':/work/native-authority', str(self.folder / 'runner-evidence'))
                            self.control('cp', self.executor.container + ':/work/source.json', str(self.folder / 'source-sha256.json'))
                            asset_digest = hashlib.sha256(
                                (self.folder / 'runner-evidence' / MANIFEST_FILE).read_bytes()).hexdigest()
                            verify_export(self.folder / 'runner-evidence', self.executor.owner, asset_digest)
                            evidence_digest = write_manifest(self.folder, self.executor.owner)
                            self.stage = 'evidence-export'
                            shutil.copytree(self.folder, self.destination)
                            verify_export(self.destination, self.executor.owner, evidence_digest)
                            self.receipt = {'schema': 1, 'evidence_sha256': evidence_digest,
                                       'asset_sha256': asset_digest, 'selected_items': list(self.selection['selected_items']),
                                       'reset_completed': False}
                        if self.reset_assets:
                            self.stage = 'generated-assets-reset'
                            self.work(['python', 'scripts/native_authority_fixture.py', 'reset', '--seconds',
                                  str(self.executor.settings['command_seconds']), '--asset-owner', self.executor.owner,
                                  '--export-digest', self.receipt['asset_sha256']])
                            self.receipt['reset_completed'] = True
                        require(self.folder.parent == self.root and self.folder.name.startswith('kehila-native-diagnostics-'),
                                'Refusing cleanup outside allocated diagnostics directory')
                        shutil.rmtree(self.folder)
                    except BaseException as error:
                        self.retain_failure(error, self.stage, 'runner-evidence-after-export-failure')
                        raise
