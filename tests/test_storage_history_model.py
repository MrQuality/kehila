"""Comparison model must preserve exact history, including order and absence."""
from copy import deepcopy
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from storage_history_model import encode_chain, reconstruct_at


def histories():
    # History-shaped JSON exercises the comparison algorithm; this helper does
    # not establish SQL/domain admission of the deliberately varied envelopes.
    seed = json.loads((ROOT / 'src/pure/work_item_codec/tests/fixtures/revision.json').read_text())
    records = []
    for version in range(1, 22):
        history = deepcopy(seed)
        history['value']['project_state_version'] = str(version)
        history['value']['previous_project_operation'] = str(1000+version)
        result = history['value']['result']['value']
        result['item']['version'] = str(version)
        result['data']['estimate'] = None if version % 2 else '0'
        result['data']['values'] = [
            {'id': 'title', 'value': {'kind': 'text', 'value': 'a' * 4096}},
            {'id': 'details', 'value': {'kind': 'text', 'value': f'edit {version}: é\n'}}]
        if version == 5:
            result['data']['values'].reverse()
        if version == 6:
            result['data']['values'].pop()
        if version == 7:
            result['data']['values'][0]['value']['value'] = ''
        records.append(history)
    return records


class StorageHistoryModelTests(unittest.TestCase):
    def test_reconstructs_every_full_envelope_without_losing_absence_order_or_metadata(self):
        records = histories()
        encoded = encode_chain(records)
        self.assertEqual([row['version'] for row in encoded if row['kind'] == 'full'], [1, 11, 21])
        for version, history in enumerate(records, 1):
            actual, reads = reconstruct_at(encoded, version)
            self.assertEqual(actual, history)
            self.assertLessEqual(reads, 10)
        self.assertLess(len(json.dumps(encoded)), len(json.dumps(records)))

    def test_wrong_base_missing_record_and_changed_delta_fail_closed(self):
        encoded = encode_chain(histories())
        for mutate in (lambda rows: rows[3].update(base_version=1),
                       lambda rows: rows.pop(2),
                       lambda rows: rows[3]['updates'][0]['value'].update(value='corrupt')):
            rows = deepcopy(encoded)
            mutate(rows)
            with self.subTest(mutation=mutate), self.assertRaises(ValueError):
                reconstruct_at(rows, 5)

    def test_later_full_checkpoint_contains_earlier_chain_corruption(self):
        records = histories()
        encoded = encode_chain(records)
        encoded[3]['updates'][0]['value']['value'] = 'corrupt'
        with self.assertRaises(ValueError):
            reconstruct_at(encoded, 10)
        self.assertEqual(reconstruct_at(encoded, 12)[0], records[11])


if __name__ == '__main__':
    unittest.main()
