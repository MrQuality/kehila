"""Host-only write-ahead ownership for the verifier's finite Compose groups.

The lifecycle process is the sole writer. Recovery may open its fresh external
record only after that process and its descendants have joined. The caller binds
the record to the current run, source and pinned engine; a saved record cannot
establish its own freshness or permission to stop a provider.
"""
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import uuid

GROUPS = (('nats',), ('opensearch',), ('postgres',))
EMPTY = {'containers': {}, 'networks': {}, 'volumes': {}}


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def _closed(pairs):
    value = {}
    for key, item in pairs:
        _require(key not in value, 'Duplicate ownership field')
        value[key] = item
    return value


def _validate_resources(value):
    _require(isinstance(value, dict) and set(value) == set(EMPTY), 'Invalid resource record')
    for kind, maximum in (('containers', 4), ('networks', 1), ('volumes', 4)):
        records = value[kind]
        _require(isinstance(records, dict) and len(records) <= maximum
                 and all(isinstance(key, str) and re.fullmatch(r'[0-9a-f]{64}', key) for key in records),
                 'Invalid resource identities')
        keys = {'containers': {'service', 'name'}, 'networks': {'name'},
                'volumes': {'container', 'destination', 'created_at'}}[kind]
        _require(all(isinstance(binding, dict) and set(binding) == keys
                     and all(isinstance(item, str) and 0 < len(item) <= 1024 for item in binding.values())
                     for binding in records.values()), 'Invalid resource bindings')
        if kind == 'volumes':
            _require(all(re.fullmatch(r'[0-9a-f]{64}', binding['container'])
                         and binding['destination'].startswith('/') for binding in records.values()),
                     'Invalid anonymous volume binding')


class ServiceOwnership:
    def __init__(self, path, runtime, data):
        self.path, self.runtime, self._data = path, runtime, data

    @property
    def compose_digest(self):
        return self._data['compose_sha256']

    @classmethod
    def create(cls, path, runtime, source, owner, groups):
        path, source = Path(path), Path(source).resolve()
        _require(path.is_absolute() and not path.is_symlink()
                 and source != path.resolve() and source not in path.resolve().parents,
                 'Ownership record must be external')
        _require(isinstance(owner, str) and re.fullmatch(r'[0-9a-f]{32}', owner) is not None,
                 'Invalid run owner')
        _require(runtime.engine[0] == 'podman', 'Tracked services require pinned Podman routing')
        groups = [tuple(group) for group in groups]
        _require(len(set(groups)) == len(groups) and all(group in GROUPS for group in groups),
                 'Invalid verifier groups')
        data = {'schema': 1, 'owner': owner, 'engine': list(runtime.engine), 'source': str(source),
                'compose_sha256': hashlib.sha256((source / 'docker-compose.yml').read_bytes()).hexdigest(),
                'groups': [{'project': 'kehila-test-' + uuid.uuid4().hex, 'services': list(group),
                            'state': 'reserved', 'resources': copy.deepcopy(EMPTY)} for group in groups]}
        # A failed/partial first write precedes all resource creation. Never
        # overwrite an old run or try to recover from its incomplete authority.
        with path.open('x', encoding='utf-8') as output:
            json.dump(data, output, sort_keys=True)
            output.flush()
            os.fsync(output.fileno())
        return cls(path, runtime, data)

    @classmethod
    def open(cls, path, runtime, source, owner, compose_digest):
        path = Path(path)
        source = Path(source).resolve()
        _require(path.is_absolute() and not path.is_symlink() and path.resolve() != source
                 and source not in path.resolve().parents, 'Invalid ownership path')
        with path.open('rb') as stream:
            raw = stream.read(65537)
        _require(len(raw) <= 65536, 'Ownership record too large')
        data = json.loads(raw, object_pairs_hook=_closed)
        _require(isinstance(data, dict) and set(data) == {
            'schema', 'owner', 'engine', 'source', 'compose_sha256', 'groups'}, 'Invalid ownership schema')
        _require(type(data['schema']) is int and data['schema'] == 1
                 and data['owner'] == owner and re.fullmatch(r'[0-9a-f]{32}', owner or '') is not None
                 and data['engine'] == list(runtime.engine) and data['source'] == str(Path(source).resolve())
                 and data['compose_sha256'] == compose_digest
                 and re.fullmatch(r'[0-9a-f]{64}', compose_digest or '') is not None,
                 'Ownership run, route or source mismatch')
        groups = data['groups']
        _require(isinstance(groups, list) and len(groups) <= len(GROUPS), 'Invalid group count')
        projects, scopes, identities = set(), set(), set()
        for group in groups:
            _require(isinstance(group, dict) and set(group) == {
                'project', 'services', 'state', 'resources'}, 'Invalid group record')
            scope = tuple(group['services']) if isinstance(group['services'], list) else ()
            _require(scope in GROUPS and scope not in scopes
                     and isinstance(group['project'], str)
                     and re.fullmatch(r'kehila-test-[0-9a-f]{32}', group['project']) is not None
                     and group['project'] not in projects
                     and isinstance(group['state'], str)
                     and group['state'] in {'reserved', 'armed', 'observed', 'complete'},
                     'Invalid group authority')
            _validate_resources(group['resources'])
            bindings = group['resources']
            _require(all(binding['service'] in scope and
                         (binding['name'].startswith(group['project'] + '_')
                          or binding['name'].startswith(group['project'] + '-'))
                         for binding in bindings['containers'].values())
                     and len({binding['service'] for binding in bindings['containers'].values()})
                         == len(bindings['containers'])
                     and all(binding['name'] == group['project'] + '_default'
                             for binding in bindings['networks'].values())
                     and all(binding['container'] in bindings['containers']
                             for binding in bindings['volumes'].values()), 'Invalid group resource scope')
            if group['state'] in {'reserved', 'armed'}:
                _require(group['resources'] == EMPTY, 'Unobserved group has resource identities')
            current = set(group['resources']['containers']) | set(group['resources']['networks'])
            _require(not current & identities, 'Resource belongs to multiple groups')
            identities.update(current)
            projects.add(group['project'])
            scopes.add(scope)
        return cls(path, runtime, data)

    def _update(self, index, state, resources=None):
        data = copy.deepcopy(self._data)
        data['groups'][index]['state'] = state
        if resources is not None:
            _validate_resources(resources)
            data['groups'][index]['resources'] = copy.deepcopy(resources)
        temporary = self.path.with_name(self.path.name + '.' + uuid.uuid4().hex + '.tmp')
        try:
            with temporary.open('x', encoding='utf-8') as output:
                json.dump(data, output, sort_keys=True)
                output.flush()
                os.fsync(output.fileno())
            os.replace(temporary, self.path)
        finally:
            temporary.unlink(missing_ok=True)
        self._data = data

    def group(self, services, compose_file):
        _require(Path(compose_file).resolve() == Path(self._data['source']) / 'docker-compose.yml',
                 'Service source differs from ownership intent')
        _require(hashlib.sha256(Path(compose_file).read_bytes()).hexdigest() == self.compose_digest,
                 'Service source changed after ownership reservation')
        matches = [index for index, group in enumerate(self._data['groups'])
                   if group['services'] == list(services)]
        _require(len(matches) == 1, 'Service scope has no unique reservation')
        return OwnedGroup(self, matches[0])

    def recover(self, resources):
        # Validate every actionable group before removing anything. Discovery
        # after a kill during up is permitted only by a durably armed intent.
        plans = []
        for index, group in enumerate(self._data['groups']):
            if group['state'] == 'reserved':
                continue
            current = resources.inventory(group, self._data['source'])
            _validate_resources(current)
            if group['state'] == 'complete':
                _require(current == EMPTY, 'Resource replacement after completed cleanup')
            if group['state'] in {'observed', 'complete'}:
                for kind in EMPTY:
                    _require(all(identity in group['resources'][kind]
                                 and binding == group['resources'][kind][identity]
                                 for identity, binding in current[kind].items()),
                             'Resource replacement after recorded observation')
            if current != EMPTY:
                plans.append((index, current))
            else:
                resources.verify_empty(group, self._data['source'], group['resources'])
                if group['state'] != 'complete':
                    self._update(index, 'complete')
        # Persist exact discovered identities before the first destructive call.
        for index, current in plans:
            if self._data['groups'][index]['state'] == 'armed':
                self._update(index, 'observed', current)
        for index, current in plans:
            group = self._data['groups'][index]
            resources.remove(group, self._data['source'], group['resources'])
            self._update(index, 'complete')


class OwnedGroup:
    def __init__(self, ownership, index):
        self.ownership, self.index = ownership, index

    @property
    def record(self):
        return self.ownership._data['groups'][self.index]

    @property
    def project(self):
        return self.record['project']

    def arm(self, resources):
        _require(self.record['state'] == 'reserved', 'Group already used')
        _require(resources.inventory(self.record, self.ownership._data['source']) == EMPTY,
                 'Reserved service namespace occupied')
        self.ownership._update(self.index, 'armed')

    def observe(self, resources, *, expected_containers=None):
        _require(self.record['state'] == 'armed', 'Group not creation-authorized')
        current = resources.inventory(self.record, self.ownership._data['source'])
        if expected_containers is not None:
            _require(set(current['containers']) == set(expected_containers.values())
                     and all(current['containers'][cid]['service'] == service
                             for service, cid in expected_containers.items()),
                     'Service identities changed before observation')
        self.ownership._update(self.index, 'observed', current)

    def cleanup(self, resources):
        _require(self.record['state'] in {'armed', 'observed'}, 'Group not creation-authorized')
        if self.record['state'] == 'armed':
            # Failed up/observation still needs write-ahead partial identities
            # before deleting a container can hide its anonymous-volume binding.
            self.observe(resources)
        resources.remove(self.record, self.ownership._data['source'], self.record['resources'])
        self.ownership._update(self.index, 'complete')
