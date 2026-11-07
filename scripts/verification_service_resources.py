"""Exact Podman resources for the verifier's existing disposable service matrix.

This matrix declares no named volumes or bind mounts. Anonymous volumes may be
removed only with their verified owning container. Unexpected mounts or retained
anonymous volumes fail cleanup rather than widening deletion authority.
"""
import json
import math
from pathlib import Path
import re
import subprocess
import time

from test_services import _account_directory
from verification_budget import command_timeout
from verification_ownership import EMPTY


def require(condition, message):
    if not condition:
        raise ValueError(message)


def identity(value):
    require(isinstance(value, str) and re.fullmatch(r'[0-9a-f]{64}', value) is not None,
            'Expected full resource identity')
    return value


def one(text):
    values = json.loads(text)
    require(isinstance(values, list) and len(values) == 1 and isinstance(values[0], dict),
            'Expected one inspected resource')
    return values[0]


class ComposeResources:
    def __init__(self, runtime, *, seconds=30):
        require(runtime.engine[0] == 'podman', 'Tracked services require pinned Podman routing')
        require(math.isfinite(seconds) and 0 < seconds <= 120, 'Invalid service cleanup allowance')
        self.runtime = runtime
        self.deadline = time.monotonic() + seconds

    def _invoke(self, arguments, *, check=True):
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError('Service ownership deadline exceeded')
        command = [*self.runtime.engine, *arguments]
        print('TEST RESOURCE:', ' '.join(command), flush=True)
        return subprocess.run(command, check=check, capture_output=True, text=True,
                              env=self.runtime.environment, cwd=_account_directory(),
                              timeout=command_timeout(min(15, remaining)))

    def _list(self, *arguments, maximum=4):
        values = self._invoke(arguments).stdout.split()
        require(len(values) <= maximum and len(values) == len(set(values)),
                'Unexpected resource cardinality')
        return values

    def inventory(self, group, source):
        project, services = group['project'], set(group['services'])
        require(re.fullmatch(r'kehila-test-[0-9a-f]{32}', project) is not None,
                'Invalid service project')
        container_ids = set(self._list('ps', '-a', '--filter', 'label=com.docker.compose.project=' + project,
                                      '--no-trunc', '--format', '{{.ID}}'))
        # Also detect occupied expected names whose labels do not confer ownership.
        container_ids.update(self._list('ps', '-a', '--filter', 'name=^' + project + '[_-]',
                                        '--no-trunc', '--format', '{{.ID}}'))
        require(len(container_ids) <= len(services), 'Duplicate or unexpected service container')
        containers, networks, volumes = {}, {}, {}
        mounts, seen_services, attached = [], set(), set()
        for cid in sorted(container_ids):
            identity(cid)
            info = one(self._invoke(('inspect', '--type', 'container', cid)).stdout)
            labels = info.get('Config', {}).get('Labels', {}) or {}
            service = labels.get('com.docker.compose.service')
            require(info.get('Id') == cid and service in services and service not in seen_services
                    and labels.get('com.docker.compose.project') == project
                    and labels.get('com.docker.compose.project.working_dir') == str(Path(source))
                    and labels.get('com.docker.compose.project.config_files') ==
                        str(Path(source) / 'docker-compose.yml'), 'Service attribution mismatch')
            seen_services.add(service)
            name = info.get('Name', '').lstrip('/')
            require(name.startswith(project + '_') or name.startswith(project + '-'),
                    'Service name differs from reserved project')
            containers[cid] = {'service': service, 'name': name}
            current_networks = info.get('NetworkSettings', {}).get('Networks', {}) or {}
            require(isinstance(current_networks, dict)
                    and set(current_networks) <= {project + '_default'}, 'Foreign service network')
            attached.update(current_networks)
            current_mounts = info.get('Mounts', [])
            require(isinstance(current_mounts, list) and len(current_mounts) <= 4,
                    'Unexpected service mounts')
            for mount in current_mounts:
                require(isinstance(mount, dict) and mount.get('Type') == 'volume',
                        'Service recovery refuses host mounts')
                mounts.append((cid, mount))
        require(len(mounts) <= 4, 'Unexpected service volume count')
        for cid, mount in mounts:
            name = identity(mount.get('Name'))
            require(name not in volumes and isinstance(mount.get('Destination'), str)
                    and mount['Destination'].startswith('/'), 'Ambiguous service volume')
            info = one(self._invoke(('volume', 'inspect', name)).stdout)
            require(info.get('Name') == name and info.get('Anonymous') is True
                    and isinstance(info.get('CreatedAt'), str), 'Service recovery refuses named volumes')
            users = self._list('ps', '-a', '--filter', 'volume=' + name, '--no-trunc', '--format', '{{.ID}}')
            require(set(users) == {cid}, 'Anonymous service volume has foreign users')
            volumes[name] = {'container': cid, 'destination': mount['Destination'],
                             'created_at': info['CreatedAt']}
        labelled_volumes = self._list('volume', 'ls', '--filter',
                                      'label=com.docker.compose.project=' + project, '--format', '{{.Name}}')
        require(set(labelled_volumes) <= set(volumes), 'Unbound project volume')
        network_names = set(self._list('network', 'ls', '--filter',
                                       'label=com.docker.compose.project=' + project, '--format', '{{.Name}}',
                                       maximum=1))
        network_names.update(self._list('network', 'ls', '--filter', 'name=^' + project + '_default$',
                                        '--format', '{{.Name}}', maximum=1))
        require(network_names <= {project + '_default'} and attached <= network_names,
                'Unexpected service network identity')
        for name in network_names:
            info = one(self._invoke(('network', 'inspect', name)).stdout)
            nid = identity(info.get('id'))
            endpoints = info.get('containers', {})
            require(info.get('name') == name
                    and (info.get('labels') or {}).get('com.docker.compose.project') == project
                    and isinstance(endpoints, dict) and set(endpoints) <= set(containers),
                    'Network attribution or endpoint mismatch')
            networks[nid] = {'name': name}
        return {'containers': containers, 'networks': networks, 'volumes': volumes}

    def remove(self, group, source, observed):
        def remaining():
            current = self.inventory(group, source)
            for kind in EMPTY:
                require(all(key in observed[kind] and value == observed[kind][key]
                            for key, value in current[kind].items()),
                        'Service resource replacement during cleanup')
            return current

        # Podman refuses to remove a container while a dependent exists. Plan
        # from inspected full IDs, never receipt serialization or ID sort order.
        current = remaining()['containers']
        dependencies = {}
        for cid in current:
            info = one(self._invoke(('inspect', '--type', 'container', cid)).stdout)
            values = info.get('Dependencies')
            require(info.get('Id') == cid and isinstance(values, list)
                    and all(isinstance(value, str) for value in values)
                    and len(values) == len(set(values)) and cid not in values
                    and set(values) <= set(observed['containers']),
                    'Invalid or foreign service dependencies')
            # Already absent owned dependencies confer no new removal authority.
            dependencies[cid] = set(values) & set(current)
        order = []
        while dependencies:
            depended_on = set().union(*dependencies.values())
            removable = sorted(set(dependencies) - depended_on)
            require(removable, 'Service dependency cycle prevents cleanup')
            for cid in removable:
                order.append(cid)
                del dependencies[cid]
        # Every call re-inspects the remaining scope; names cannot adopt a new ID.
        for cid in order:
            if cid in remaining()['containers']:
                self._invoke(('rm', '--force', '--volumes', cid))
        for nid in observed['networks']:
            if nid in remaining()['networks']:
                self._invoke(('network', 'rm', nid))
        self.verify_empty(group, source, observed)

    def verify_empty(self, group, source, observed):
        require(self.inventory(group, source) == EMPTY, 'Owned service resources remain after removal')
        for name in observed['volumes']:
            result = self._invoke(('volume', 'exists', name), check=False)
            require(result.returncode == 1,
                    'Anonymous volume retention or inspection failure requires explicit recovery')
