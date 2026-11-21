"""Own fresh, networkless clusters for nonoverlapping structural SQL profiles.

The foundation profile includes deliberate guard mutations. The Selection
profile composes WorkItem controls. Neither is run against the native fixture's
initialized cluster, and neither compiles a host-side codec helper.
"""
import re
import json
from pathlib import Path
import subprocess
import tempfile
import time

from check_postgresql import DatabaseCheck, IMAGE, validate_container
from check_selection_storage import check as selection_structure
from cleanup_errors import CleanupStack, retain_cleanup_failure
from container_executor import LABEL, one_object
from verification_budget import cleanup_time, command_timeout


class EngineDatabase(DatabaseCheck):
    def __init__(self, executor, identity):
        super().__init__([], identity, executor.source)
        self.executor = executor

    def command(self, *arguments, input=None, timeout=60):
        # The engine client controls a containerized psql process. Preserve its
        # actual exit/output so DatabaseCheck can discriminate SQLSTATE failures.
        from test_services import _account_directory
        return subprocess.run([*self.executor.runtime.engine, *arguments], input=input,
                   timeout=command_timeout(timeout), env=self.executor.runtime.environment,
                   cwd=_account_directory(), capture_output=True, text=True,
                   encoding='utf-8', check=False)


def owned_identity(executor, identity, name, *, timeout=None):
    info = (executor.inspect_container(identity) if timeout is None else one_object(
        executor.engine('inspect', '--type', 'container', identity,
                        capture=True, timeout=timeout).stdout))
    if (info['Id'] != identity or info['Name'].lstrip('/') != name
            or info['Config'].get('Labels', {}).get(LABEL) != executor.owner):
        raise ValueError('Structural cluster ownership mismatch')
    return info


def retain_failure(executor, identity, name, profile, stage, error):
    """Keep bounded server diagnostics outside source before releasing the cluster."""
    try:
        with cleanup_time():
            owned_identity(executor, identity, name, timeout=10)
            logs = executor.engine('logs', '--tail=50', identity, capture=True, timeout=10)
            parent = Path(tempfile.gettempdir()).resolve()
            source = executor.source.resolve()
            if parent == source or source in parent.parents:
                raise ValueError('Structural diagnostics must be outside source')
            destination = Path(tempfile.mkdtemp(prefix='kehila-structure-failure-', dir=parent))
            (destination / 'failure.json').write_text(json.dumps({
                'schema': 1, 'profile': profile, 'stage': stage,
                'failure_class': type(error).__name__,
                'stdout': (logs.stdout or '')[-65536:],
                'stderr': (logs.stderr or '')[-65536:]}), encoding='utf-8')
    except BaseException as diagnostic:
        retain_cleanup_failure(error, diagnostic, 'structural failure diagnostics')


def check(executor):
    for profile in ('foundation', 'selection'):
        name = 'kehila-structure-' + executor.owner + '-' + profile
        identity = executor.engine('create', '--pull=never', '--name', name,
            '--label', LABEL + '=' + executor.owner, '--label', 'purpose=kehila-postgresql-test',
            '--network=none', '--memory=256m', '--cpus=1', '--pids-limit=128',
            '--tmpfs', '/var/lib/postgresql/data:rw,size=256m',
            '--env', 'POSTGRES_HOST_AUTH_METHOD=trust', IMAGE,
            'postgres', '-c', 'listen_addresses=127.0.0.1', capture=True).stdout.strip()
        if re.fullmatch(r'[0-9a-f]{64}', identity) is None:
            raise ValueError('Structural cluster immutable identity unavailable')
        database = EngineDatabase(executor, identity)

        def remove():
            with cleanup_time():
                owned_identity(executor, identity, name)
                executor.engine('rm', '-f', identity)
                absent = database.command('container', 'exists', identity)
                if absent.returncode != 1:
                    raise ValueError('Structural cluster removal not confirmed')

        with CleanupStack() as cleanup:
            cleanup.callback(remove, resource='structural SQL cluster')
            stage = 'admission'
            try:
                owned_identity(executor, identity, name)
                stage = 'startup'
                executor.engine('start', identity)
                deadline = time.monotonic() + command_timeout(60)
                while True:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise ValueError('Structural cluster readiness deadline exceeded')
                    info = owned_identity(executor, identity, name, timeout=min(15, remaining))
                    validate_container(info)
                    # The temporary initialization server has no TCP listener.
                    # Only the final server can satisfy this loopback control.
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise ValueError('Structural cluster readiness deadline exceeded')
                    ready = database.command('exec', identity, 'pg_isready', '-h', '127.0.0.1',
                                             '-U', 'postgres', timeout=min(5, remaining))
                    if ready.returncode == 0:
                        break
                    time.sleep(min(0.1, max(0, deadline - time.monotonic())))
                stage = 'sql-assertions'
                if profile == 'foundation':
                    database.structural_integrity()
                else:
                    selection_structure(database, executor.source)
            except BaseException as error:
                retain_failure(executor, identity, name, profile, stage, error)
                raise
        print('PASS: structural SQL profile and owned cluster release:', profile, flush=True)
