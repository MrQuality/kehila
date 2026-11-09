"""Validate finite client lineage and emit a streaming independent SQL witness."""
import argparse
import base64
import copy
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path

from capacity_plan import cell as planned_cell

LATENCY_BOUNDARY = 'prepared HTTP request through verified TLS/driver cleanup and result validation'
THROUGHPUT_BOUNDARY = 'phase clock through joined clients, including request preparation and evidence encoding'


def require(value, message):
    if not value:
        raise ValueError(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode('utf-8')


def same(left, right):
    return canonical(left) == canonical(right)


def number(value, *, positive=False):
    require(type(value) is int and (value > 0 if positive else value >= 0), 'Invalid capacity count or duration')
    return value


def _pairs(pairs):
    value = {}
    for key, item in pairs:
        require(key not in value, 'Duplicate JSON property in capacity evidence')
        value[key] = item
    return value


def _constant(value):
    raise ValueError('Nonstandard JSON number in capacity evidence')


def parse(raw):
    return json.loads(raw, object_pairs_hook=_pairs, parse_constant=_constant)


def response(attempt):
    status = attempt['status']
    if status is None:
        require(isinstance(attempt['transport_error'], str) and attempt['transport_error']
                and attempt['response_base64'] is None and attempt['response_body_bytes'] == 0,
                'Missing transport-failure evidence')
        return None
    number(status, positive=True)
    require(attempt['transport_error'] is None, 'HTTP outcome also claims a transport failure')
    raw = base64.b64decode(attempt['response_base64'], validate=True)
    require(len(raw) == attempt['response_body_bytes'], 'Response byte count differs')
    value = parse(raw)
    require(isinstance(value, dict), 'Response is not an object')
    if status != 200:
        require(isinstance(value.get('error'), str), 'Non-success response lacks an error code')
    return value


def validate_outcome(attempt, payload):
    status, edit = attempt['status'], attempt['edit']
    if status is None:
        expected, uncertain = 'transport_failure', edit
    elif status == 200:
        expected, uncertain = 'success', False
    elif status == 429:
        expected, uncertain = 'busy', False
    elif status == 408:
        expected, uncertain = 'request_deadline', False
    elif status == 503:
        if not edit:
            expected, uncertain = 'unavailable', False
        elif payload['error'] == 'attempt_exhausted':
            expected, uncertain = 'attempt_exhausted', False
        else:
            expected, uncertain = 'uncertain', True
    else:
        raise ValueError('Unexpected original HTTP status')
    require(attempt['outcome'] == expected and attempt['uncertain'] is uncertain,
            'Original outcome or reconciliation requirement differs from HTTP evidence')


def field_map(values):
    require(isinstance(values, list), 'Field values are not a list')
    result = {}
    for value in values:
        require(isinstance(value, dict) and set(value) == {'id', 'value'}
                and isinstance(value['id'], str) and value['id'] not in result, 'Invalid or duplicate field identity')
        result[value['id']] = value['value']
    return result


def full_values(values, fields, text_bytes):
    require([value['id'] for value in values] == fields, 'Request is not a full configured-field update')
    for value in values:
        require(isinstance(value['value'], str) and len(value['value'].encode('utf-8')) == text_bytes,
                'Synthetic text size differs from the selected profile')


def create_request(seed):
    body, registration = seed['request'], seed['registration']
    return {'codec_version': 1, 'kind': 'item_create_request', 'value': {
        **body, 'item_id': registration['target_item_id'], 'operation_id': registration['token'],
        'values': [{'id': field['id'], 'value': {'kind': 'text', 'value': field['value']}} for field in body['values']]}}


def edit_request(body):
    return {'codec_version': 1, 'kind': 'item_edit_request', 'value': {
        **{key: value for key, value in body.items() if key != 'fields'},
        'fields': [{'id': field['id'], 'edit': {'action': 'set', 'value': {'kind': 'text', 'value': field['value']}}}
                   for field in body['fields']]}}


def accept(previous, body, result, projects):
    expected = copy.deepcopy(previous)
    expected['kind'] = 'item_edit_result'
    expected['value'].pop('readable_id', None)
    expected['value']['item']['version'] = str(int(previous['value']['item']['version']) + 1)
    expected['value']['project']['next_sequence'] = str(16 // projects + 1)
    values = result['value']['data']['values']
    intended = {field['id']: {'kind': 'text', 'value': field['value']} for field in body['fields']}
    require(same(field_map(values), intended), 'Accepted edit differs from the original full-field intent')
    expected['value']['data']['values'] = values  # Order is preserved for later exact reads.
    require(same(expected, result), 'Accepted edit changed identity, version, metadata, lifecycle or estimate')
    return result


def percentiles(values):
    values = sorted(values)
    return {'count': len(values), **{name: values[math.ceil(len(values) * quantile) - 1] if values else None
            for name, quantile in [('p50', .5), ('p95', .95), ('p99', .99)]}}


def observation_summary(phase):
    rows = phase['rows']
    stages = None
    if rows is not None:
        grouped = {}
        for index, row in enumerate(rows):
            require(type(row['id']) is int and row['id'] == index, 'Stage row identity differs')
            number(row['phase_offset_ns'])
            for event in row['events']:
                require(isinstance(event['stage'], str) and type(event['completed']) is bool,
                        'Malformed stage observation')
                number(event['offset_ns'])
                group = grouped.setdefault(event['stage'], {'count': 0, 'incomplete': 0, 'wall': [], 'cpu': []})
                group['count'] += 1
                group['incomplete'] += not event['completed']
                if event['duration_ns'] is not None: group['wall'].append(number(event['duration_ns']))
                if event['cpu_time_ns'] is not None:
                    require(event['stage'] == 'CpuWork', 'CPU observation attached to a non-worker stage')
                    group['cpu'].append(number(event['cpu_time_ns']))
        stages = {name: {'events': value['count'], 'incomplete_events': value['incomplete'],
                        'wall_ns': percentiles(value['wall']), 'thread_cpu_ns': percentiles(value['cpu'])}
                  for name, value in sorted(grouped.items())}
    sampling = phase['sampling']
    waits = None
    if sampling is not None:
        require(number(sampling['interval_ns'], positive=True) == 5_000_000
                and len(sampling['samples']) <= number(sampling['sample_limit'], positive=True),
                'Sampler cadence or evidence bound differs')
        queries, observations = [], Counter()
        for sample in sampling['samples']:
            start, end = number(sample['started_ns']), number(sample['finished_ns'])
            require(end >= start, 'Sampler clock reversed')
            queries.append(end - start)
            for backend in sample['backends']:
                require(type(backend['pid']) is int and isinstance(backend['blocking_pids'], list),
                        'Malformed sampled backend identity')
                observations[(backend['state'], backend['wait_type'], backend['wait_event'],
                              bool(backend['blocking_pids']))] += 1
        waits = {'samples': len(queries), 'query_elapsed_ns': percentiles(queries),
                 'backend_observations': [{'state': state, 'wait_type': kind, 'wait_event': event,
                                           'has_blocker': blocked, 'count': count}
                     for (state, kind, event, blocked), count in observations.items()],
                 'interpretation': 'sample counts; not wait durations or actor/project attribution'}
    return {'stages': stages, 'sampling': waits,
            'stage_interpretation': 'aggregate stage populations; overlapping stages must not be summed'}


def phase_summary(phase):
    attempts = phase['attempts']
    window = number(phase['request_window_ns'], positive=True)
    successful = [attempt for attempt in attempts if attempt['outcome'] == 'success']
    return {'attempted': len(attempts), 'successful': len(successful), 'recoveries': len(phase['recoveries']),
            'outcomes': dict(Counter(attempt['outcome'] for attempt in attempts)),
            'latency_ns': percentiles(attempt['latency_ns'] for attempt in attempts),
            'successful_latency_ns': percentiles(attempt['latency_ns'] for attempt in successful),
            'request_window_ns': window, 'checkpoint_ns': number(phase['checkpoint_ns']),
            'attempts_per_second': len(attempts) * 1e9 / window,
            'successful_per_second': len(successful) * 1e9 / window,
            **observation_summary(phase)}


@dataclass
class Validated:
    records: list
    summary: dict


def validate_calibration(phase, project):
    controls = phase['controls']
    require(len(controls) == 2 and len(phase['attempts']) == 2 and not phase['recoveries'],
            'Calibration control population differs')
    samples = phase['sampling']['samples']
    offset = 0
    identities, transactions = set(), set()
    for control, target, fingerprint, key in zip(controls, ('actor','project'),
            (' FROM kehila_auth.fence(', ' FROM kehila_auth.save_item_edit('), ('owner',project)):
        require(control['target'] == target and control['target_key'] == key
                and control['query_fingerprint'] == fingerprint and isinstance(control['target_ctid'], str)
                and control['known_blocker_observed'] is True and control['edge_gone'] is True
                and control['blocker_idle'] is True, 'Known blocker or release evidence differs')
        pid = number(control['blocker_pid'], positive=True)
        xid, start = control['blocker_transaction_id'], control['blocker_backend_start']
        require(isinstance(xid, str) and xid.isdecimal() and isinstance(start, str) and start
                and isinstance(control['blocker_transaction_start'], str), 'Missing blocker transaction identity')
        identities.add((pid,start))
        require(xid not in transactions, 'Calibration reused a blocker transaction')
        transactions.add(xid)
        dispatch, release, response_ns = (number(control[name]) for name in ('dispatch_ns','release_ns','response_ns'))
        require(dispatch <= release <= response_ns, 'Calibration dispatch/release/response order differs')
        count = number(control['sample_count'], positive=True)
        recorded = samples[offset:offset+count]
        require(len(recorded) == count and all(dispatch <= number(sample['started_ns'])
                <= number(sample['finished_ns']) <= release for sample in recorded), 'Calibration sample window differs')
        offset += count
        matching = []
        for backend in recorded[-1]['backends']:
            locks = parse(backend['ungranted_locks_json'])
            if (backend['state'] == 'active' and backend['wait_type'] == 'Lock'
                    and pid in backend['blocking_pids'] and backend['backend_start'] and backend['transaction_start']
                    and fingerprint in (backend['query'] or '')
                    and any(lock['locktype'] == 'transactionid' and lock['transaction_id'] == xid for lock in locks)):
                matching.append(backend)
        require(len(matching) == 1, 'Calibration sampler did not observe the exact known transaction blocker')
        require(all(pid not in backend['blocking_pids'] for backend in control['after_release']['backends']),
                'Known blocking edge survived release')
    require(offset == len(samples) and len(identities) == 1, 'Calibration sampler population or retained blocker differs')
    require(all(attempt['outcome'] == 'success' and attempt['edit'] is True for attempt in phase['attempts']),
            'Controlled original edit did not succeed after release')


def validate(work, owned, cell, observer, calibration=False):
    require(observer in {'off', 'stages', 'sampled'}, 'Unknown capacity observer selection')
    require(work['latency_boundary'] == LATENCY_BOUNDARY and work['throughput_boundary'] == THROUGHPUT_BOUNDARY,
            'Measured latency or throughput boundary differs')
    require(type(calibration) is bool and work.get('calibration', False) is calibration
            and (not calibration or (cell.index == 15 and observer == 'sampled')), 'Calibration selection differs')
    require(type(work.get('schema')) is int and work['schema'] == 1 and work.get('valid') is True
            and same(work['plan'], cell.document()) and work['observer'] == observer, 'Capacity plan or validity differs')
    require(type(owned.get('schema')) is int and owned['schema'] == 1 and owned.get('runtime_joined') is True
            and owned.get('connection_accounting_valid') is True, 'Native ownership was not qualified')
    number(owned['total_ns'], positive=True)
    setup = work['setup']
    require(number(setup['elapsed_ns']) >= number(setup['authentication_ns']) >= number(setup['authentication_pacing_ns']),
            'Setup timing components are inconsistent')
    require(work['postgres']['max_connections'] == '8' and work['postgres']['ssl'] == 'on', 'PostgreSQL profile differs')
    slots = work['slots']
    require(len(slots) == 16 and [slot['slot'] for slot in slots] == list(range(16)), 'Seeded item population differs')
    records = [{'type': 'meta', 'schema': 1, 'plan': cell.document(), 'observer': observer, 'calibration': calibration}]
    known, readable = {}, {}
    projects = {}
    for slot in slots:
        index, seed = slot['slot'], slot['seed']
        require(type(index) is int and slot['unresolved'] is False, 'Unresolved or invalid item slot')
        project = slot['project_id']
        projects.setdefault(index % cell.document()['projects'], project)
        require(projects[index % cell.document()['projects']] == project, 'Independent-project distribution differs')
        require(seed['registration']['target_item_id'] == slot['item_id']
                and seed['registration']['project_id'] == project, 'Seed registration identity differs')
        fields = slot['field_ids']
        require(len(fields) == cell.document()['fields_per_item'] and len(set(fields)) == len(fields), 'Configured field population differs')
        full_values(seed['request']['values'], fields, cell.document()['text_bytes_per_field'])
        saved = seed['result']
        require(type(saved['codec_version']) is int and saved['codec_version'] == 1 and saved['kind'] == 'item_create_result'
                and saved['value']['item']['id'] == slot['item_id']
                and saved['value']['item']['project_id'] == project
                and saved['value']['item']['version'] == '1'
                and saved['value']['readable_id'] == seed['readable_id'], 'Initial saved identity differs')
        intended = {field['id']: {'kind': 'text', 'value': field['value']} for field in seed['request']['values']}
        require(same(field_map(saved['value']['data']['values']), intended), 'Initial field data differs')
        request = canonical(create_request(seed))
        records.append({'type': 'seed', 'slot': index, 'project_id': project, 'item_id': slot['item_id'],
                        'registration': seed['registration'], 'request_sha256': hashlib.sha256(request).hexdigest(),
                        'request_base64': base64.b64encode(request).decode('ascii'),
                        'result': saved, 'field_ids': fields})
        known[index], readable[index] = copy.deepcopy(saved), seed['readable_id']
    require(len(set(projects.values())) == cell.document()['projects'], 'Project population differs')
    require({entry['project_id'] for entry in setup['projects']} == set(projects.values()), 'Configuration witnesses differ')
    for entry in setup['projects']: number(entry['configuration_json_bytes'], positive=True)
    phases = work['phases']
    specification = ([('calibration', 2)] if calibration else []) + [('warmup',10),('measured',100),('control',1)]
    require([phase['phase'] for phase in phases] == [kind for kind, _ in specification], 'Phase sequence differs')
    tokens, summaries, recoveries_count = set(), {}, 0
    for phase, (kind, count) in zip(phases, specification):
        require(phase['errors'] == [] and phase['planned_requests'] == count
                and len(phase['attempts']) == count
                and phase['configured_concurrency'] == (1 if kind == 'control' else cell.concurrency), 'Phase is incomplete')
        number(phase['planned_requests'], positive=True)
        number(phase['configured_concurrency'], positive=True)
        require((phase['rows'] is None) == (observer == 'off')
                and (phase['sampling'] is None) == (observer != 'sampled'), 'Observation mode differs')
        if observer == 'sampled':
            require(phase['sampling']['error'] is None and phase['sampling']['samples'], 'Sampler evidence is incomplete')
        if kind == 'calibration': validate_calibration(phase, slots[0]['project_id'])
        pending = {}
        for ordinal, attempt in enumerate(phase['attempts']):
            require(type(attempt['ordinal']) is int and attempt['ordinal'] == ordinal
                    and attempt['phase'] == kind and type(attempt['session']) is int
                    and attempt['session'] == ordinal % 4, 'Request or session schedule differs')
            index = number(attempt['slot'])
            require(index < phase['configured_concurrency'] and index not in pending, 'Item slot reused before reconciliation')
            slot, previous = slots[index], known[index]
            version = int(previous['value']['item']['version'])
            body = attempt['request']
            require(attempt['integrity_error'] is None and type(attempt['uncertain']) is bool
                    and attempt['project'] == slot['project_id'] and attempt['item'] == slot['item_id']
                    and body['project_id'] == slot['project_id'] and body['item_id'] == slot['item_id']
                    and type(attempt['expected_item_version']) is int and attempt['expected_item_version'] == version,
                    'Request identity, known version or integrity differs')
            edit = kind == 'control' or cell.mix == 'edit' or (cell.mix == 'mixed' and ordinal % 2 == 1)
            require(type(attempt['edit']) is bool and attempt['edit'] == edit, 'Read/edit mix differs')
            require(attempt['request_json_bytes'] == len(canonical(body)), 'Request byte count differs')
            number(attempt['latency_ns'])
            payload = response(attempt)
            validate_outcome(attempt, payload)
            token, digest = None, None
            if edit:
                token = body['operation_id']
                require(set(body) == {'project_id','item_id','operation_id','expected_item_version',
                                     'expected_configuration_revision','estimate','fields'}
                        and token not in tokens and body['expected_item_version'] == str(version)
                        and body['expected_configuration_revision'] == '1' and body['estimate'] == {'action': 'keep'}, 'Edit identity reused or changed')
                tokens.add(token)
                updates = body['fields']
                require(all(field['action'] == 'set' for field in updates), 'Sparse update in a full-field cell')
                full_values(updates, slot['field_ids'], cell.document()['text_bytes_per_field'])
                raw = base64.b64decode(attempt['canonical_base64'], validate=True)
                require(same(parse(raw), edit_request(body)), 'Canonical identity differs from the original HTTP request')
                digest = hashlib.sha256(raw).hexdigest()
            else:
                require(set(body) == {'project_id','item_id'} and attempt['canonical_base64'] is None
                        and not attempt['uncertain'], 'Read claims a mutation identity')
            success = attempt['outcome'] == 'success'
            require(success == (attempt['status'] == 200), 'Success population differs from HTTP outcomes')
            result = payload['result'] if success else None
            accepted = False
            if success:
                require(not attempt['uncertain'], 'Successful request remains uncertain')
                if edit:
                    known[index] = accept(previous, body, result, cell.document()['projects'])
                    accepted = True
                else:
                    require(same(result, previous) and payload['readable_id'] == readable[index], 'Read differs from the known exact snapshot')
            elif attempt['uncertain']:
                require(edit and attempt['status'] in (None, 503), 'Unproven uncertainty classification')
            else:
                require(attempt['status'] in (None, 408, 429, 503), 'Unexpected non-success HTTP status')
                if edit and attempt['status'] == 503:
                    require(payload['error'] == 'attempt_exhausted', 'Unavailable edit was not reconciled')
            record = {'type': 'attempt', 'phase': kind, 'ordinal': ordinal, 'slot': index, 'session': attempt['session'],
                      'edit': edit, 'project_id': slot['project_id'], 'item_id': slot['item_id'],
                      'expected_version': version, 'operation_id': token, 'request_sha256': digest,
                      'request_base64': attempt['canonical_base64'],
                      'accepted': accepted, 'success': success, 'result': result, 'status': attempt['status'],
                      'outcome': attempt['outcome']}
            records.append(record)
            if attempt['uncertain']: pending[index] = (attempt, record)
        for recovery in phase['recoveries']:
            index = number(recovery['slot'])
            require(index in pending, 'Reconciliation has no original uncertain request')
            original, record = pending.pop(index)
            number(recovery['ordinal'])
            number(recovery['session'])
            require(recovery['phase'] == 'recovery' and recovery['ordinal'] == original['ordinal']
                    and recovery['session'] == original['session'] and recovery['edit'] is True
                    and recovery['integrity_error'] is None and recovery['uncertain'] is False
                    and recovery['expected_item_version'] == original['expected_item_version']
                    and same(recovery['request'], original['request'])
                    and recovery['canonical_base64'] == original['canonical_base64'], 'Reconciliation changed the original identity')
            payload = response(recovery)
            if recovery['status'] == 200 and recovery['outcome'] == 'success':
                known[index] = accept(known[index], original['request'], payload['result'], cell.document()['projects'])
                record['accepted'], record['result'] = True, payload['result']
            else:
                require(recovery['status'] == 404 and recovery['outcome'] == 'reconciled_absent'
                        and payload['error'] == 'operation_not_found', 'Original outcome remains unresolved')
            recoveries_count += 1
        require(not pending, 'Uncertain original has no terminal reconciliation')
        if kind == 'control': require(phase['attempts'][0]['outcome'] == 'success', 'Serial post-load progress control failed')
        summaries['calibration_phase' if kind == 'calibration' else kind] = phase_summary(phase)
    for slot in slots:
        index = slot['slot']
        require(type(slot['expected_final_version']) is int
                and slot['expected_final_version'] == int(known[index]['value']['item']['version'])
                and same(slot['final_result'], known[index]), 'Final client version chain differs')
        records.append({'type': 'slot', 'slot': index, 'project_id': slot['project_id'], 'item_id': slot['item_id'],
                        'expected_version': slot['expected_final_version'], 'result': slot['final_result']})
    expected_connections = 40 + cell.document()['projects'] + 111 + (2 if calibration else 0) + recoveries_count
    connection = owned['connections']
    for value in connection.values(): number(value)
    require(owned['expected_connections'] == expected_connections
            and connection['accepted'] + connection['capacity_rejected'] == expected_connections
            and connection['completed'] + connection['protocol_rejected'] + connection['timed_out'] == connection['accepted']
            and sum(connection[key] for key in ('tls_rejected', 'http_rejected', 'tls_shutdown_rejected', 'peer_close_rejected')) == connection['protocol_rejected'],
            'Owned transport population differs from original and recovery requests')
    target = None
    if cell.concurrency == 4:
        target = (summaries['measured']['successful'] == 100 and summaries['measured']['latency_ns']['p95'] <= 1_000_000_000
                  and not any(connection[key] for key in ('capacity_rejected', 'protocol_rejected', 'timed_out')))
    return Validated(records, {'schema': 1, 'plan': cell.document(), 'observer': observer,
        'calibration': calibration, **summaries,
        'latency_boundary': work['latency_boundary'], 'throughput_boundary': work['throughput_boundary'],
        'normal_four_request_target_met': target, 'connections': connection,
        'target_population': '100 original measured requests; warmup, control and recovery reported separately; transport counters cover the entire cell',
        'setup_ns': setup['elapsed_ns'], 'authentication_pacing_ns': setup['authentication_pacing_ns'],
        'configuration_json_bytes': [entry['configuration_json_bytes'] for entry in setup['projects']],
        'postgres': work['postgres'], 'total_ns': owned['total_ns']})


def write_evidence(work_path, owned_path, cell, observer, output, negative_control=False, calibration=False):
    source = Path(__file__).resolve().parents[1]
    require(output.is_absolute() and output.is_dir() and output.resolve() == output
            and source != output and source not in output.parents
            and not any((parent / '.git').exists() for parent in (output, *output.parents)), 'Evidence output must be outside source and Git')
    raw_work, raw_owned = work_path.read_bytes(), owned_path.read_bytes()
    validated = validate(parse(raw_work), parse(raw_owned), cell, observer, calibration)
    path = output / 'matrix-expectations.jsonl'
    with path.open('x', encoding='utf-8', newline='\n') as stream:
        for record in validated.records: stream.write(canonical(record).decode('utf-8') + '\n')
    validated.summary.update(work_sha256=hashlib.sha256(raw_work).hexdigest(),
                             ownership_sha256=hashlib.sha256(raw_owned).hexdigest(),
                             expectations_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    with (output / 'matrix-summary.json').open('x', encoding='utf-8') as stream:
        json.dump(validated.summary, stream, sort_keys=True, separators=(',', ':'))
    if negative_control:
        changed = copy.deepcopy(validated.records)
        slot = next(record for record in changed if record['type'] == 'slot' and record['slot'] == 0)
        field = slot['result']['value']['data']['values'][0]['value']
        field['value'] = ('1' if field['value'][0] == '0' else '0') + field['value'][1:]
        with (output / 'matrix-negative-expectations.jsonl').open('x', encoding='utf-8', newline='\n') as stream:
            for record in changed: stream.write(canonical(record).decode('utf-8') + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--owned', type=Path, required=True)
    parser.add_argument('--cell', type=int, required=True)
    parser.add_argument('--observer', choices=['off', 'stages', 'sampled'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--negative-control', action='store_true')
    parser.add_argument('--calibration', action='store_true')
    args = parser.parse_args()
    write_evidence(args.work, args.owned, planned_cell(args.cell), args.observer, args.output, args.negative_control, args.calibration)
    print('CAPACITY EVIDENCE: original identities, schedule and ownership validated')


if __name__ == '__main__': main()
