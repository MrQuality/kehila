"""Serial real native product fixture inside an already-owned toolchain runner.

The caller owns provider supervision. This child owns its PostgreSQL dependency
and removes it before its namespace-owning runner can be removed.
"""
import json
from dataclasses import dataclass
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
from native_authority_fixture import AUTH_DATABASES


@dataclass(frozen=True)
class NumericToken:
    text: str


def snapshot_document(text):
    def invalid_constant(_value):
        raise ValueError('Nonstandard numeric token in native snapshot')
    # Preserve exact numeric tokens and distinguish numbers from booleans.
    # Ordinary Python equality would equate 1, 1.0 and True.
    return json.loads(text, parse_int=NumericToken, parse_float=NumericToken,
                      parse_constant=invalid_constant)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify_write_trace(expected, log):
    require(isinstance(expected, list) and expected
            and all(isinstance(item, str) and re.fullmatch(
                r'kehila(?:_auth)?\.[a-z_]+ (?:INSERT|UPDATE)', item) for item in expected),
            'Native write trace expectation is invalid')
    observed = [(int(ordinal), table + ' ' + operation)
                for ordinal, table, operation in re.findall(
                    r'NATIVE_WRITE (\d+) (kehila(?:_auth)?\.[a-z_]+) (INSERT|UPDATE)\b', log)]
    require(observed == list(enumerate(expected, 1)),
            'Native write trace differs from the complete ordered write plan')


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


def admit_database(info, identity, runner, owner, name):
    admit_owned_database(info, identity, owner, name)
    host, config = info['HostConfig'], info['Config']
    require(config['Image'] == IMAGE, 'Native database image mismatch')
    cpu = (host['NanoCpus'] == 10**9 if host.get('NanoCpus') else
           host.get('CpuPeriod', 0) > 0 and host.get('CpuQuota') == host['CpuPeriod'])
    require(not host['Privileged'] and not host.get('Binds') and not info.get('Mounts')
            and not host.get('Devices') and not host.get('PortBindings')
            and host['NetworkMode'] == 'container:' + runner,
            'Native database isolation mismatch')
    require(host['Memory'] == 256 * 1024**2 and host['PidsLimit'] == 128 and cpu,
            'Native database limits mismatch')
    tmpfs = host.get('Tmpfs', {})
    require(set(tmpfs) == {'/var/lib/postgresql/data'}
            and 'size=256m' in tmpfs['/var/lib/postgresql/data'].split(','),
            'Native database must use bounded ephemeral storage')


def check(executor):
    require(executor.build_network_detached, 'Disconnect build networking before native verification')
    admit_runner(executor.inspect_container(executor.container), executor.container, executor.owner)
    root = Path(tempfile.gettempdir()).resolve()
    require(root != executor.source and executor.source not in root.parents,
            'Native diagnostic storage must be outside the source snapshot')
    require(not any((parent / '.git').exists() for parent in (root, *root.parents)),
            'Native diagnostic storage must be outside Git checkouts')
    folder = Path(tempfile.mkdtemp(prefix='kehila-native-diagnostics-', dir=root)).resolve()
    name = executor.name + '-native-db'
    stage, attempted, failure = 'prepare', False, None

    def control(*args, both_streams=False):
        try:
            result = executor.engine(*args, capture=True)
            return result.stdout + ((getattr(result, 'stderr', '') or '') if both_streams else '')
        except subprocess.CalledProcessError as error:
            (folder / (stage + '-failure.txt')).write_text(
                (error.stdout or '') + (error.stderr or ''), encoding='utf-8')
            raise RuntimeError('Native fixture command failed at ' + stage) from None

    def work(command):
        # This child must release its dependent DB before the parent destroys
        # the runner. Use the same cooperative container deadline wrapper;
        # do not invoke execute()'s immediate parent-runner teardown here.
        seconds = command_timeout(executor.settings['command_seconds'])
        executor.engine('exec', executor.container, 'python', 'scripts/container_command.py',
            str(seconds), *command, timeout=seconds + executor.settings['runner_stop_seconds'])

    def psql(file, database='postgres', *, tuples=False):
        return control('exec', name, 'psql', '-X', '-U', 'postgres', '-d', database,
                       *(['-q', '-A', '-t'] if tuples else []), '-v', 'ON_ERROR_STOP=1', '-f', file)

    try:
        work(['python', '-c', "from pathlib import Path; assert {line.split(':')[0].strip() "
              "for line in Path('/proc/net/dev').read_text().splitlines() if ':' in line} == {'lo'}"])
        work(['python', 'scripts/native_authority_fixture.py', 'prepare', '--seconds',
              str(executor.settings['command_seconds'])])
        control('cp', executor.container + ':/work/native-authority/postgres', str(folder / 'postgres'))
        stage, attempted = 'database-create', True
        identity = control('create', '--pull=never', '--name', name, '--label', LABEL + '=' + executor.owner,
            '--network', 'container:' + executor.container, '--memory=256m', '--cpus=1', '--pids-limit=128',
            '--tmpfs', '/var/lib/postgresql/data:rw,size=256m',
            '--env', 'POSTGRES_PASSWORD_FILE=/tmp/kehila-native/admin-password',
            '--env', 'POSTGRES_HOST_AUTH_METHOD=scram-sha-256', '--entrypoint', '/bin/sh', IMAGE,
            '-c', 'set -eu; chmod 700 /tmp/kehila-native; chmod 600 /tmp/kehila-native/*; '
            'chown -R postgres:postgres /tmp/kehila-native; '
            'exec /usr/local/bin/docker-entrypoint.sh postgres -c max_connections=8 '
            '-c shared_buffers=16MB -c ssl=on -c ssl_min_protocol_version=TLSv1.3 '
            '-c ssl_cert_file=/tmp/kehila-native/server.pem '
            '-c ssl_key_file=/tmp/kehila-native/server-key.pem -c listen_addresses=127.0.0.1').strip()
        info = one_object(control('inspect', '--type', 'container', identity))
        admit_database(info, identity, executor.container, executor.owner, name)
        control('cp', str(folder / 'postgres'), identity + ':/tmp/kehila-native')
        control('cp', str(executor.source / 'storage/postgresql'), identity + ':/tmp/kehila-schema')
        control('cp', str(executor.source / 'tests/integration/native_authority_state.sql'),
                identity + ':/tmp/native-authority-state.sql')
        control('cp', str(executor.source / 'tests/integration/native_clock_state.sql'),
                identity + ':/tmp/native-clock-state.sql')
        for file in ('native_copy_snapshot.sql', 'native_restored_state.sql', 'native_auth_fixture_state.sql'):
            control('cp', str(executor.source / 'tests/integration' / file), identity + ':/tmp/' + file)
        stage = 'database-start'
        control('start', identity)
        ready_by = time.monotonic() + command_timeout(executor.settings['command_seconds'])
        while True:
            info = one_object(control('inspect', '--type', 'container', identity))
            require(info['State']['Running'], 'Native database exited at startup')
            if control('exec', identity, 'sh', '-c',
                'if pg_isready -h 127.0.0.1 -p 5432 -q -U postgres; then echo READY; else echo WAIT; fi').strip() == 'READY':
                break
            require(time.monotonic() < ready_by, 'Native database readiness allowance expired')
            time.sleep(0.1)
        stage = 'database-install'
        control('exec', identity, 'sh', '-c',
            'set -eu; test "$(cat /sys/fs/cgroup/memory.max)" = 268435456; '
            'test "$(cat /sys/fs/cgroup/pids.max)" = 128; '
            'set -- $(cat /sys/fs/cgroup/cpu.max); test "$1" != max; test "$1" = "$2"')
        psql('/tmp/kehila-schema/install_product.sql')
        psql('/tmp/kehila-native/setup.sql')
        stage = 'pristine-authentication-fixtures'
        baseline = "DO $$ BEGIN IF EXISTS (SELECT FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid()) OR EXISTS (SELECT FROM kehila.projects) OR (SELECT runtime_generation FROM kehila_auth.installation)<>0 OR (SELECT auth_generation<>1 OR verifier_revision<>1 FROM kehila_auth.owner_credentials) THEN RAISE EXCEPTION 'pristine quiescent fixture required'; END IF; END $$;"
        control('exec', identity, 'psql', '-X', '-U', 'postgres', '-v', 'ON_ERROR_STOP=1', '-c', baseline)
        for database in AUTH_DATABASES.values():
            control('exec', identity, 'createdb', '-U', 'postgres', '--owner=kehila_owner',
                    '--template=postgres', database)
            admission = "DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_database WHERE datname=current_database() AND datdba='kehila_owner'::regrole AND datallowconn AND NOT datistemplate AND pg_encoding_to_char(encoding)='UTF8') OR NOT has_database_privilege('kehila_auth_runtime_login',current_database(),'CONNECT') OR EXISTS (SELECT FROM pg_db_role_setting WHERE setdatabase=(SELECT oid FROM pg_database WHERE datname=current_database())) THEN RAISE EXCEPTION 'authentication database profile mismatch'; END IF; END $$;"
            control('exec', identity, 'psql', '-X', '-U', 'postgres', '-d', database,
                    '-v', 'ON_ERROR_STOP=1', '-c', admission + baseline)
        stage = 'journey'
        work(['python', 'scripts/native_authority_fixture.py', 'journey', '--seconds',
              str(executor.settings['command_seconds'])])
        stage = 'write-trace-observer'
        control('cp', executor.container + ':/work/native-authority/controls/write-trace.json',
                str(folder / 'write-trace.json'))
        write_log = control('logs', '--timestamps', identity, both_streams=True)
        (folder / 'write-trace.log').write_text(write_log, encoding='utf-8')
        verify_write_trace(json.loads((folder / 'write-trace.json').read_text(encoding='utf-8')),
                           write_log)
        stage = 'independent-observer'
        observation = psql('/tmp/native-authority-state.sql')
        (folder / 'observation.txt').write_text(observation, encoding='utf-8')
        stage = 'clock-authority-journey'
        work(['python', 'scripts/native_authority_fixture.py', 'clock', '--seconds',
              str(executor.settings['command_seconds'])])
        stage = 'clock-independent-observer'
        (folder / 'clock-observation.txt').write_text(psql('/tmp/native-clock-state.sql'), encoding='utf-8')
        stage = 'quiescent-copy'
        source_state = psql('/tmp/native_copy_snapshot.sql', tuples=True)
        require(len(source_state.encode('utf-8')) <= 1024**2, 'Synthetic copy observation exceeds bound')
        decoded = snapshot_document(source_state)
        require(isinstance(decoded, dict) and decoded.get('tables') and decoded.get('sequences')
                and all(key in decoded for key in ('schemas','relations','types','indexes','columns','functions','constraints','triggers','raw_checks')),
                'Synthetic copy observation is incomplete')
        (folder / 'copy-source-state.txt').write_text(source_state, encoding='utf-8')
        repeated_state = psql('/tmp/native_copy_snapshot.sql', tuples=True)
        (folder / 'copy-source-repeat-state.txt').write_text(repeated_state, encoding='utf-8')
        require(source_state == repeated_state, 'CHECK normalization changed production snapshot or was unstable')
        control('exec', identity, 'sh', '-c',
                'set -eu; umask 077; mkdir -m 700 /tmp/native-copy; '
                'pg_dump -U postgres -d postgres --format=custom --schema=kehila '
                '--schema=kehila_auth --schema=kehila_native_fixture --file=/tmp/native-copy/state.dump; '
                'test "$(stat -c %a /tmp/native-copy/state.dump)" = 600')
        control('exec', identity, 'createdb', '-U', 'postgres', '--owner=kehila_owner', 'kehila_native_restored')
        control('exec', identity, 'pg_restore', '-U', 'postgres', '-d', 'kehila_native_restored',
                '--single-transaction', '--exit-on-error', '/tmp/native-copy/state.dump')
        restored_state = psql('/tmp/native_copy_snapshot.sql', 'kehila_native_restored', tuples=True)
        require(len(restored_state.encode('utf-8')) <= 1024**2, 'Restored copy observation exceeds bound')
        (folder / 'copy-restored-state.txt').write_text(restored_state, encoding='utf-8')
        restored_decoded = snapshot_document(restored_state)
        require(isinstance(restored_decoded, dict) and 'raw_checks' in restored_decoded,
                'Restored copy observation is incomplete')
        # Both raw deparse forms stay in the private snapshots. Only CHECK
        # definitions use PostgreSQL's scratch-object reparse normalization;
        # every other captured field remains an exact comparison.
        decoded.pop('raw_checks')
        restored_decoded.pop('raw_checks')
        require(decoded == restored_decoded, 'Synthetic copy state/schema/authority differs before startup')
        stage = 'restored-native-journey'
        work(['python', 'scripts/native_authority_fixture.py', 'restored', '--seconds',
              str(executor.settings['command_seconds'])])
        stage = 'restored-independent-observer'
        (folder / 'restored-observation.txt').write_text(
            psql('/tmp/native_restored_state.sql', 'kehila_native_restored'), encoding='utf-8')
        # The final test changes the source database's password. Keep it after
        # every journey that depends on its original fixture credential.
        for mode in ('auth-expiry', 'auth-delivery', 'auth-unknown'):
            stage = mode
            work(['python', 'scripts/native_authority_fixture.py', mode, '--seconds',
                  str(executor.settings['command_seconds'])])
        stage = 'authentication-independent-observer'
        (folder / 'authentication-observation.txt').write_text(
            psql('/tmp/native-clock-state.sql'), encoding='utf-8')
        for mode, database in AUTH_DATABASES.items():
            stage = mode
            work(['python', 'scripts/native_authority_fixture.py', mode, '--seconds',
                  str(executor.settings['command_seconds'])])
            stage = mode + '-independent-observer'
            (folder / (mode + '-observation.txt')).write_text(
                psql('/tmp/native_auth_fixture_state.sql', database), encoding='utf-8')
    except BaseException as error:
        failure = error
        (folder / 'failure-stage.txt').write_text(stage, encoding='ascii')
        # Copy failure evidence without printing its contents or installing any
        # trust. The caller may retain this external directory privately.
        try:
            with cleanup_time():
                control('cp', executor.container + ':/work/native-authority', str(folder / 'runner-evidence'))
        except BaseException as evidence_error:
            retain_cleanup_failure(error, evidence_error, 'native diagnostic collection')
        raise
    finally:
        try:
            if attempted:
                with cleanup_time():
                    ids = control('ps', '-a', '--filter', 'label=' + LABEL + '=' + executor.owner,
                                  '--filter', 'name=^' + name + '$', '--no-trunc', '--format', '{{.ID}}').split()
                    for identity in ids:
                        info = one_object(control('inspect', '--type', 'container', identity))
                        # A rejected execution profile does not revoke ownership
                        # of a container this invocation must still remove.
                        admit_owned_database(info, identity, executor.owner, name)
                        try:
                            (folder / 'postgres.log').write_text(
                                control('logs', '--timestamps', identity, both_streams=True), encoding='utf-8')
                        finally:
                            executor.engine('rm', '--force', '--volumes', identity,
                                            timeout=executor.settings['runner_stop_seconds'])
        except BaseException as error:
            if failure is not None:
                retain_cleanup_failure(failure, error, 'native PostgreSQL')
            else:
                failure = error
                raise
        finally:
            if failure is None:
                require(folder.parent == root and folder.name.startswith('kehila-native-diagnostics-'),
                        'Refusing cleanup outside allocated diagnostics directory')
                shutil.rmtree(folder)
    print('PASS: native durable cardinalities, identity coherence and owned cleanup')
