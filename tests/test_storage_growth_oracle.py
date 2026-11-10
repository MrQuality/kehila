"""Independent storage witnesses reject changed fields, history and tombstones."""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from storage_growth_plan import select
from storage_growth_results import validate_expected, validate_state


def expected_records():
    project = {'id': 'p', 'prefix': 'ST', 'next_sequence': '2', 'ever_estimated': False}
    values = [{'id': f'capacity-text-{i}', 'value': {'kind': 'text', 'value': 'a' * 64}}
              for i in range(4)]
    records = []
    for version in range(1, 22):
        token = f'{version:032x}'
        request = {'codec_version': 1, 'kind': 'item_create_request' if version == 1 else 'item_edit_request',
                   'value': {'project_id': 'p', 'item_id': 'i', 'operation_id': token,
                             'expected_configuration_revision': '1'}}
        if version == 1:
            request['value'].update(values=deepcopy(values), estimate=None)
        else:
            text = chr(ord('a') + version - 1) * 64
            values[0]['value']['value'] = text
            request['value'].update(expected_item_version=str(version-1), estimate={'action': 'keep'},
                                    fields=[{'id': 'capacity-text-0', 'edit': {'action': 'set',
                                              'value': {'kind': 'text', 'value': text}}}])
        result = {'codec_version': 1, 'kind': 'item_create_result' if version == 1 else 'item_edit_result',
                  'value': {'project': deepcopy(project),
                            'item': {'id': 'i', 'project_id': 'p', 'type_id': 'task',
                                     'workflow_id': 'wf', 'status_id': 'new', 'version': str(version)},
                            'phase': 'new', 'data': {'estimate': None, 'values': deepcopy(values)}}}
        records.append({'version': version, 'token': token, 'result': result,
                        'request_hex': json.dumps(request, sort_keys=True, separators=(',', ':')).encode().hex()})
    return records


def observed_state(records, historical=False):
    rows = [{'type': 'meta', 'projects': 1, 'items': 1, 'revisions': 21, 'operations': 22,
             'payloads': 1 if historical else 22, 'field_uses': 4, 'runtime_backends': 0,
             'prepared_transactions': 0, 'authority_locks': 0},
            {'type': 'identity', 'project_creation_operation_row_id': 100,
             'current_version': '21', 'state_version': '21', 'head_operation_row_id': 121,
             'creation_operation_row_id': 101, 'pending_operation_row_id': 101, 'item_id': 'i',
             'pending_target_item_id': 'i', 'project_id': 'p', 'issued_prefix': 'ST',
             'issued_sequence': '1', 'next_sequence': '2', 'ever_estimated': False,
             'pending_state': 'completed',
             'pending_request_sha256': hashlib.sha256(bytes.fromhex(records[0]['request_hex'])).hexdigest(),
             'pending_payload_sha256': None if historical else hashlib.sha256(
                 bytes.fromhex(records[0]['request_hex'])).hexdigest()}]
    for version, record in enumerate(records, 1):
        digest = hashlib.sha256(bytes.fromhex(record['request_hex'])).hexdigest()
        origin = 0 if historical else 100000000000
        rows.append({'type': 'revision', 'version': str(version), 'project_state_version': str(version),
                     'previous_item_version': None if version == 1 else str(version-1),
                     'previous_project_operation': 99 + version, 'operation_row_id': 100 + version,
                     'history': {'codec_version': 1, 'kind': 'item_revision', 'value': {
                         'actor_id': 'owner', 'configuration_revision': '1',
                         'project_state_version': str(version), 'previous_project_operation': str(99+version),
                         'result': deepcopy(record['result'])}},
                     'actor_id': 'owner', 'family': 'item_create' if version == 1 else 'item_edit',
                     'project_id': 'p', 'item_id': 'i', 'token': record['token'], 'history_token': record['token'],
                     'request_sha256': digest, 'payload_sha256': None if historical else digest,
                     'payload_result': None if historical else deepcopy(record['result']),
                     'payload_retired': historical, 'request_codec_version': 1, 'history_codec_version': 1,
                     'replay_origin_ms': str(origin), 'replay_deadline_ms': str(origin+7776000000)})
    rows.extend({'type': 'field_use', 'field_id': f'capacity-text-{i}', 'owner_type_id': 'task',
                 'first_item_id': 'i', 'first_item_version': '1', 'first_operation_row_id': 101} for i in range(4))
    return rows


class StorageGrowthOracleTests(unittest.TestCase):
    def test_native_and_historical_observers_preserve_full_expected_semantics(self):
        records = expected_records()
        sample = select([0])[0]
        validate_expected(records, sample)
        for historical in (False, True):
            validate_state(records, observed_state(records, historical), sample,
                           'historical' if historical else 'native')

    def test_sparse_edit_cannot_silently_change_an_untouched_field(self):
        records = expected_records()
        records[10]['result']['value']['data']['values'][1]['value']['value'] = 'unexpected'
        with self.assertRaisesRegex(ValueError, 'independently applied fields'):
            validate_expected(records, select([0])[0])

    def test_retired_history_attribution_identity_and_field_use_are_independent_oracles(self):
        records = expected_records()
        valid = observed_state(records, True)
        mutations = [lambda rows: rows[3]['history']['value'].update(actor_id='foreign'),
                     lambda rows: rows[1].update(pending_operation_row_id=999),
                     lambda rows: rows[3].update(payload_sha256='0'*64),
                     lambda rows: rows[-1].update(first_item_version='2'),
                     lambda rows: rows[0].update(authority_locks=1),
                     lambda rows: rows.pop(10)]
        for mutate in mutations:
            rows = deepcopy(valid)
            mutate(rows)
            with self.subTest(mutation=mutate), self.assertRaises(ValueError):
                validate_state(records, rows, select([0])[0], 'historical')


if __name__ == '__main__':
    unittest.main()
