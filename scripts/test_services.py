"""Own short-lived Compose groups; never tear down the development project."""
from contextlib import contextmanager
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import urlsplit
import uuid

from setup_env import compose_command

ROOT = Path(__file__).resolve().parents[1]
SERVICES = {'postgres', 'ferretdb', 'nats', 'opensearch'}


@dataclass(frozen=True)
class ComposeRuntime:
    compose: list[str]
    environment: dict[str, str]

    @property
    def engine(self):
        command = [self.compose[0]]
        if command[0] == 'podman' and self.environment.get('CONTAINER_CONNECTION'):
            command += ['--connection', self.environment['CONTAINER_CONNECTION']]
        return command


def run(command, timeout=120, **kwargs):
    print('TEST RESOURCE:', ' '.join(command), flush=True)
    return subprocess.run(command, cwd=ROOT, check=True, timeout=timeout, **kwargs)


def _containers(runtime, project, service=None, include_stopped=False):
    command = [*runtime.engine, 'ps', '--filter', 'label=com.docker.compose.project=' + project]
    if service:
        command += ['--filter', 'label=com.docker.compose.service=' + service]
    if include_stopped:
        command.append('--all')
    return run([*command, '--format', '{{.ID}}'], capture_output=True,
               text=True, timeout=15, env=runtime.environment).stdout.split()


def _machine_info(name, environment=None):
    info = json.loads(run(['podman', 'machine', 'inspect', name],
                          capture_output=True, text=True, timeout=15, env=environment).stdout)
    if (not isinstance(info, list) or len(info) != 1 or not isinstance(info[0], dict)
            or info[0].get('Name') != name
            or info[0].get('State') not in {'stopped', 'running'}):
        raise ValueError('Expected one named Podman machine in a stable state')
    return info[0]


def _machine_connections(name, info, environment=None):
    connections = json.loads(run(['podman', 'system', 'connection', 'list', '--format', 'json'],
                                 capture_output=True, text=True, timeout=15, env=environment).stdout)
    if not isinstance(connections, list) or not all(isinstance(c, dict) for c in connections):
        raise ValueError('Expected the Podman connection inventory')
    ssh = info.get('SSHConfig')
    if (not isinstance(ssh, dict) or type(ssh.get('Port')) is not int
            or not 1 <= ssh['Port'] <= 65535
            or not isinstance(ssh.get('RemoteUsername'), str) or not ssh['RemoteUsername']):
        raise ValueError('Expected the requested machine SSH endpoint')
    selected = []
    for connection_name, user in ((name, ssh['RemoteUsername']), (name + '-root', 'root')):
        matching = [c for c in connections if c.get('Name') == connection_name]
        if len(matching) != 1 or not isinstance(matching[0].get('URI'), str):
            raise ValueError('Both machine-created Podman connections are required for safe VM cleanup')
        endpoint = urlsplit(matching[0]['URI'])
        socket_matches = (endpoint.path == '/run/podman/podman.sock' if user == 'root' else
                          re.fullmatch(r'/run/user/[1-9][0-9]*/podman/podman\.sock', endpoint.path))
        if (endpoint.scheme != 'ssh' or endpoint.hostname not in {'127.0.0.1', 'localhost', '::1'}
                or endpoint.port != ssh['Port'] or endpoint.username != user or not socket_matches
                or endpoint.query or endpoint.fragment or endpoint.password is not None
                or ssh['RemoteUsername'] == 'root'):
            raise ValueError('Podman connection does not identify the requested machine/store')
        selected.append(connection_name)
    return selected


def _stop_owned_machine(name, environment=None):
    info = _machine_info(name, environment=environment)
    if info['State'] == 'stopped':
        return
    # Both stores share this VM; a concurrent user may have started other work.
    # Query the registered endpoints: `machine ssh` can create a literal NUL
    # known-hosts file in its Windows working directory.
    for connection in _machine_connections(name, info, environment=environment):
        active = run(['podman', '--connection', connection, 'ps', '-q'],
                     capture_output=True, text=True, timeout=30, env=environment).stdout.strip()
        if active:
            print('TEST RESOURCE: keeping VM running because other containers are active', flush=True)
            return
    run(['podman', 'machine', 'stop', name], timeout=120, env=environment)


@contextmanager
def managed_runtime(preferred='auto', machine=None, environment=None):
    if preferred not in {'auto', 'podman', 'docker'}:
        raise ValueError('Unknown container engine')
    if machine and (preferred == 'docker' or
                    re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,127}', machine) is None):
        raise ValueError('An explicit Podman machine requires a simple name and the Podman engine')
    owned_machine = False
    failure = None
    environment = dict(os.environ if environment is None else environment)
    try:
        if machine:
            preferred = 'podman'
            info = _machine_info(machine, environment=environment)
            connections = _machine_connections(machine, info, environment=environment)
            selected = environment.get('CONTAINER_CONNECTION') or connections[bool(info.get('Rootful'))]
            if selected not in connections or environment.get('CONTAINER_HOST'):
                raise ValueError('Select a verified connection belonging to the requested Podman machine')
            environment['CONTAINER_CONNECTION'] = selected
            owned_machine = info['State'] == 'stopped'
            if owned_machine:
                run(['podman', 'machine', 'start', machine], timeout=120, env=environment)
        yield ComposeRuntime(compose_command(preferred, environment=environment), environment)
    except BaseException as error:
        failure = error
        raise
    finally:
        if owned_machine:
            try:
                _stop_owned_machine(machine, environment=environment)
            except (OSError, ValueError, subprocess.SubprocessError) as cleanup_error:
                if failure is not None:
                    raise failure from cleanup_error
                raise


@contextmanager
def service_group(runtime, selected):
    if not selected or len(set(selected)) != len(selected) or not set(selected) <= SERVICES:
        raise ValueError('Expected a nonempty set of known test services')
    # -p overrides the Compose file's development name. Every group owns a fresh
    # project, including partial startup; cleanup can only address that project.
    project = 'kehila-test-' + uuid.uuid4().hex
    command = [*runtime.compose, '-p', project, '-f', str(ROOT / 'docker-compose.yml')]
    failure = None
    try:
        run([*command, 'up', '-d', *selected], timeout=300, env=runtime.environment)
        for service in selected:
            if len(_containers(runtime, project, service)) != 1:
                raise RuntimeError('Expected one running owned container for ' + service)
        yield
    except BaseException as error:
        failure = error
        try:
            run([*command, 'logs', '--no-color'], timeout=30, env=runtime.environment)
        except (OSError, subprocess.SubprocessError) as diagnostic_error:
            print('TEST RESOURCE: diagnostics failed: ' + str(diagnostic_error), file=sys.stderr)
        raise
    finally:
        try:
            run([*command, 'down', '--volumes'], timeout=120, env=runtime.environment)
            if _containers(runtime, project, include_stopped=True):
                raise RuntimeError('Owned test containers remain after Compose cleanup')
        except (OSError, RuntimeError, subprocess.SubprocessError) as cleanup_error:
            if failure is not None:
                raise failure from cleanup_error
            raise
