"""Validate settled storage censuses before deriving physical or WAL growth.

These checks establish accounting consistency, not fixture correctness, historical
preservation, source identity, or acceptance of a projected storage envelope.
"""
import hashlib
import json
import re

_COMPONENTS = ('heap_main_bytes', 'heap_auxiliary_bytes', 'index_bytes',
               'toast_main_bytes', 'toast_auxiliary_bytes', 'toast_index_bytes')
_REQUIRED_TABLES = {'kehila.item_revisions', 'kehila.items', 'kehila.operations',
                    'kehila.operation_payloads', 'kehila.item_field_uses',
                    'kehila_auth.pending_items'}
_LOGICAL_FIELDS = {'revisions', 'logical_history_text_bytes', 'minimum_history_text_bytes',
                   'maximum_history_text_bytes', 'replay_payloads', 'logical_request_bytes',
                   'logical_result_text_bytes', 'pending_creation_identities',
                   'pending_creation_payloads', 'logical_pending_request_bytes'}
_SETTINGS = {'version', 'block_size', 'default_toast_compression', 'wal_compression',
             'full_page_writes', 'autovacuum', 'checkpoint_timeout', 'max_wal_size'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def _count(value):
    return type(value) is int and value >= 0


def _lsn(value):
    require(isinstance(value, str) and re.fullmatch(r'[0-9A-F]{1,8}/[0-9A-F]{1,8}', value),
            'Invalid WAL insert position')
    high, low = value.split('/')
    return (int(high, 16) << 32) + int(low, 16)


def validate_census(value):
    require(isinstance(value, dict) and type(value.get('schema')) is int
            and value['schema'] == 1, 'Unknown storage census schema')
    require(isinstance(value.get('cluster_system_identifier'), str)
            and re.fullmatch(r'[0-9]+', value['cluster_system_identifier'])
            and _count(value.get('database_oid')) and value['database_oid'] > 0,
            'Storage census cluster/database identity missing')
    settings = value.get('postgres')
    require(isinstance(settings, dict) and set(settings) == _SETTINGS
            and all(isinstance(item, str) and item for item in settings.values()),
            'Storage census settings missing')
    _lsn(value.get('wal_insert_lsn'))
    for name in ('wal_directory_allocated_bytes', 'database_allocated_bytes',
                 'items', 'operations', 'retired_operations', 'field_uses'):
        require(_count(value.get(name)), 'Invalid storage census count: ' + name)
    require(value['retired_operations'] <= value['operations'], 'Retired count exceeds operations')
    relations = value.get('relations')
    require(isinstance(relations, list) and relations, 'Storage relation census missing')
    names = set()
    for relation in relations:
        require(isinstance(relation, dict), 'Invalid relation census')
        name = relation.get('name')
        require(isinstance(name, str) and re.fullmatch(r'kehila(?:_auth)?\.[a-z_]+', name)
                and name not in names, 'Invalid or repeated storage relation')
        names.add(name)
        require(all(_count(relation.get(field)) for field in (*_COMPONENTS, 'total_bytes')),
                'Invalid physical relation byte count')
        require(sum(relation[field] for field in _COMPONENTS) == relation['total_bytes'],
                'Physical relation components do not reconcile')
        options = relation.get('reloptions')
        require(options is None or (isinstance(options, list)
                and all(isinstance(option, str) for option in options)),
                'Invalid relation storage options')
    require(_REQUIRED_TABLES <= names, 'Required storage relations missing')
    require(sum(relation['total_bytes'] for relation in relations)
            <= value['database_allocated_bytes'], 'Relation allocation exceeds database allocation')
    columns = value.get('column_storage')
    require(isinstance(columns, list) and columns, 'Column storage census missing')
    identities = set()
    for column in columns:
        require(isinstance(column, dict) and column.get('relation') in names
                and isinstance(column.get('column'), str)
                and column.get('storage') in ('p', 'e', 'm', 'x')
                and column.get('compression') in ('', 'p', 'l'), 'Invalid column storage census')
        identity = (column['relation'], column['column'])
        require(identity not in identities, 'Repeated column storage census')
        identities.add(identity)
    logical = value.get('logical')
    require(isinstance(logical, dict) and set(logical) == _LOGICAL_FIELDS
            and all(_count(item) for item in logical.values()), 'Invalid logical storage census')
    require(logical['pending_creation_payloads'] <= logical['pending_creation_identities']
            and logical['minimum_history_text_bytes'] <= logical['maximum_history_text_bytes']
            <= 1_048_576, 'Logical storage counts or history bounds disagree')


def wal_delta(before, after):
    """Cluster-generated bytes; never WAL-directory size or a causal per-edit cost."""
    validate_census(before)
    validate_census(after)
    require(all(before[field] == after[field] for field in
                ('cluster_system_identifier', 'database_oid', 'postgres', 'column_storage')),
            'Storage environment changed between WAL observations')
    delta = _lsn(after['wal_insert_lsn']) - _lsn(before['wal_insert_lsn'])
    require(delta >= 0, 'WAL insert position moved backwards')
    return delta


def fixture_text(sample, edit, field):
    """Independent Python implementation of the declared finite text fixture."""
    size = sample.text_bytes if sample.text_bytes is not None else 7600
    if sample.entropy == 'repeated':
        return chr(ord('a') + edit % 26) * size
    return ''.join(hashlib.sha256(f'storage:{sample.index}:{edit}:{field}:{block}'.encode()).hexdigest()
                   for block in range((size + 63) // 64))[:size]


def validate_expected(records, sample):
    require(isinstance(records, list) and len(records) == 21, 'Storage expected population mismatch')
    previous = None
    for version, record in enumerate(records, 1):
        require(type(record.get('version')) is int and record['version'] == version,
                'Storage expected version mismatch')
        raw = bytes.fromhex(record['request_hex'])
        require(0 < len(raw) <= 1_048_576, 'Storage request bound exceeded')
        request = json.loads(raw)
        require(request['codec_version'] == 1 and request['kind'] ==
                ('item_create_request' if version == 1 else 'item_edit_request'),
                'Storage request codec/family mismatch')
        command = request['value']
        result = record['result']
        require(result['codec_version'] == 1 and result['kind'] ==
                ('item_create_result' if version == 1 else 'item_edit_result'),
                'Storage result codec/family mismatch')
        require(command['operation_id'] == record['token']
                and command['project_id'] == result['value']['project']['id']
                and command['item_id'] == result['value']['item']['id']
                and command['expected_configuration_revision'] == '1',
                'Storage request/result identity mismatch')
        entries = result['value']['data']['values']
        values = {entry['id']: entry['value'] for entry in entries}
        field_ids = [f'capacity-text-{index}' for index in range(sample.fields)]
        require(len(entries) == len(values) == sample.fields and set(values) == set(field_ids),
                'Storage result field population mismatch')
        require(result['value']['item']['version'] == str(version)
                and result['value']['data']['estimate'] is None
                and result['value']['project']['next_sequence'] == '2'
                and result['value']['project']['ever_estimated'] is False,
                'Storage result version/project/estimate mismatch')
        if version == 1:
            expected_values = {key: {'kind': 'text', 'value': fixture_text(sample, 0, index)}
                               for index, key in enumerate(field_ids)}
            require({entry['id']: entry['value'] for entry in command['values']} == expected_values
                    and command['estimate'] is None, 'Storage create input differs from fixture')
        else:
            require(command['expected_item_version'] == str(version-1)
                    and command['estimate'] == {'action': 'keep'}, 'Storage edit predecessor mismatch')
            updates = command['fields']
            changed = range(sample.fields) if sample.edit == 'all_fields' else (0,)
            expected_updates = {field_ids[index]: {'action': 'set', 'value': {
                'kind': 'text', 'value': fixture_text(sample, version-1, index)}} for index in changed}
            require(len(updates) == len(expected_updates)
                    and {field['id']: field['edit'] for field in updates} == expected_updates,
                    'Storage edit input differs from fixture/locality')
            expected_values = previous.copy()
            expected_values.update({key: update['value'] for key, update in expected_updates.items()})
            for key in expected_updates:
                require(previous[key] != expected_values[key], 'Storage edit did not change its field')
            old = records[version-2]['result']['value']
            require(result['value']['project'] == old['project']
                    and result['value']['phase'] == old['phase']
                    and {k: v for k, v in result['value']['item'].items() if k != 'version'}
                    == {k: v for k, v in old['item'].items() if k != 'version'},
                    'Storage edit changed immutable identity/lifecycle')
        require(values == expected_values, 'Storage result differs from independently applied fields')
        previous = values


def validate_state(records, observed, sample, role):
    require(role in {'native', 'historical'}, 'Unknown storage role')
    meta = [row for row in observed if row.get('type') == 'meta']
    identity = [row for row in observed if row.get('type') == 'identity']
    revisions = [row for row in observed if row.get('type') == 'revision']
    uses = [row for row in observed if row.get('type') == 'field_use']
    require(len(meta) == len(identity) == 1 and len(revisions) == 21
            and len(uses) == sample.fields and len(observed) == 23 + sample.fields,
            'Storage independent observer population mismatch')
    meta, identity = meta[0], identity[0]
    for key, expected_count in {'projects': 1, 'items': 1, 'revisions': 21, 'operations': 22,
                               'payloads': 22 if role == 'native' else 1,
                               'field_uses': sample.fields, 'runtime_backends': 0,
                               'prepared_transactions': 0, 'authority_locks': 0}.items():
        require(type(meta.get(key)) is int and meta[key] == expected_count,
                'Storage independent count/ownership mismatch: ' + key)
    predecessor = identity['project_creation_operation_row_id']
    operation_ids = {predecessor}
    for version, (record, row) in enumerate(zip(records, revisions), 1):
        saved = record['result']
        digest = hashlib.sha256(bytes.fromhex(record['request_hex'])).hexdigest()
        expected_history = {'codec_version': 1, 'kind': 'item_revision', 'value': {
            'actor_id': 'owner', 'configuration_revision': '1',
            'project_state_version': str(version), 'previous_project_operation': str(predecessor),
            'result': saved}}
        require(row['history'] == expected_history and row['version'] == str(version)
                and row['project_state_version'] == str(version)
                and row['previous_item_version'] == (None if version == 1 else str(version-1))
                and row['previous_project_operation'] == predecessor,
                'Storage independent history/chain mismatch')
        require(row['operation_row_id'] not in operation_ids, 'Storage operation identity reused')
        operation_ids.add(row['operation_row_id'])
        predecessor = row['operation_row_id']
        require(row['request_sha256'] == digest
                and row['payload_sha256'] == (digest if role == 'native' else None)
                and row['payload_result'] == (saved if role == 'native' else None)
                and row['payload_retired'] is (role == 'historical')
                and row['token'] == row['history_token'] == record['token']
                and row['actor_id'] == 'owner'
                and row['family'] == ('item_create' if version == 1 else 'item_edit')
                and row['project_id'] == saved['value']['project']['id']
                and row['item_id'] == saved['value']['item']['id']
                and row['request_codec_version'] == row['history_codec_version'] == 1,
                'Storage independent attribution/replay mismatch')
        origin = int(row['replay_origin_ms'])
        require(int(row['replay_deadline_ms']) == origin + 7_776_000_000
                and (origin == 0 if role == 'historical' else origin > 0),
                'Storage replay origin/window mismatch')
    first, last = records[0], records[-1]['result']['value']
    digest = hashlib.sha256(bytes.fromhex(first['request_hex'])).hexdigest()
    require(identity['current_version'] == identity['state_version'] == '21'
            and identity['head_operation_row_id'] == predecessor
            and identity['creation_operation_row_id'] == identity['pending_operation_row_id']
            == revisions[0]['operation_row_id']
            and identity['item_id'] == identity['pending_target_item_id'] == last['item']['id']
            and identity['project_id'] == last['project']['id']
            and identity['issued_prefix'] == last['project']['prefix']
            and identity['issued_sequence'] == '1' and identity['next_sequence'] == '2'
            and identity['ever_estimated'] is False and identity['pending_state'] == 'completed'
            and identity['pending_request_sha256'] == digest
            and identity['pending_payload_sha256'] == (digest if role == 'native' else None),
            'Storage successful identity/head/pending preservation mismatch')
    require({row['field_id'] for row in uses} == {f'capacity-text-{i}' for i in range(sample.fields)}
            and all(row['first_item_id'] == last['item']['id'] and row['first_item_version'] == '1'
                    and row['first_operation_row_id'] == revisions[0]['operation_row_id']
                    and row['owner_type_id'] == last['item']['type_id'] for row in uses),
            'Storage field-use witness mismatch')


def validate_checkpoint(census, sample, stage, count, baseline, previous=None):
    validate_census(census)
    retired = stage in {'retired', 'pruned'}
    pruned = stage == 'pruned'
    logical = census['logical']
    expected = {'items': int(count > 0), 'operations': count+1,
                'retired_operations': count if retired else 0,
                'field_uses': sample.fields if count else 0}
    require(all(census[key] == expected_count for key, expected_count in expected.items()),
            'Storage checkpoint physical population mismatch')
    expected = {'revisions': count, 'replay_payloads': 1 if retired else count+1,
                'pending_creation_identities': int(count > 0),
                'pending_creation_payloads': int(count > 0 and not pruned)}
    require(all(logical[key] == expected_count for key, expected_count in expected.items()),
            'Storage checkpoint logical population mismatch')
    require((logical['logical_pending_request_bytes'] > 0) == (count > 0 and not pruned),
            'Storage checkpoint pending request content mismatch')
    if count:
        require(logical['minimum_history_text_bytes'] > 0
                and logical['minimum_history_text_bytes'] * count <= logical['logical_history_text_bytes']
                <= logical['maximum_history_text_bytes'] * count, 'Storage history byte population mismatch')
        if sample.profile == 'near_boundary':
            require(logical['minimum_history_text_bytes'] >= 943_719,
                    'Storage near-boundary profile did not reach its declared bound')
    else:
        require(all(logical[key] == 0 for key in ('logical_history_text_bytes',
                'minimum_history_text_bytes', 'maximum_history_text_bytes')), 'Storage baseline history is not empty')
    if retired:
        require(logical['logical_request_bytes'] == baseline['logical']['logical_request_bytes']
                and logical['logical_result_text_bytes'] == baseline['logical']['logical_result_text_bytes'],
                'Storage retired item payload content remains')
        require(previous is not None and all(logical[key] == previous['logical'][key] for key in
                ('logical_history_text_bytes', 'minimum_history_text_bytes', 'maximum_history_text_bytes')),
                'Storage history bytes changed during retirement/pruning')
    if previous is not None:
        wal_delta(previous, census)


def validate_sample_observations(folder, native_path, historical_path, sample):
    records = json.loads((folder / 'expected.json').read_text(encoding='utf-8'))
    validate_expected(records, sample)
    for role, path in (('native', native_path), ('historical', historical_path)):
        observed = [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()
                    if line.strip().startswith('{')]
        validate_state(records, observed, sample, role)
        stages = [('baseline', 0), ('create', 1), ('edit-1', 2), ('edit-5', 6), ('edit-10', 11), ('edit-20', 21)]
        if role == 'historical':
            stages += [('retired', 21), ('pruned', 21)]
        previous = baseline = None
        for stage, count in stages:
            census = json.loads((folder / f'{role}-{stage}.json').read_text(encoding='utf-8'))
            if baseline is None:
                baseline = census
            validate_checkpoint(census, sample, stage, count, baseline, previous)
            previous = census
    summary = json.loads((folder / 'storage-summary.json').read_text(encoding='utf-8'))
    require(summary == {'schema': 1, 'sample': sample.index, 'native_revisions': 21,
                        'historical_revisions': 21, 'runtime_joined': True, 'native_http_completed': 25,
                        'historical_retirement': 'owner-only constructed past origin'},
            'Storage runtime/selection completion mismatch')
