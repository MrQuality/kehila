"""Reject censuses that could silently misstate physical growth or WAL cost."""
from copy import deepcopy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from storage_growth_results import validate_census, validate_checkpoint, wal_delta
from storage_growth_plan import select


def observation():
    return {'schema': 1, 'cluster_system_identifier': '123456789', 'database_oid': 16384,
            'postgres': {'version': '16.15', 'block_size': '8192',
                         'default_toast_compression': 'pglz', 'wal_compression': 'off',
                         'full_page_writes': 'on', 'autovacuum': 'on',
                         'checkpoint_timeout': '5min', 'max_wal_size': '1GB'},
            'wal_insert_lsn': '0/1000000', 'wal_directory_allocated_bytes': 16777216,
            'database_allocated_bytes': 10_000_000,
            'relations': [{'name': 'kehila.' + name, 'reloptions': None,
                           'heap_main_bytes': 8192, 'heap_auxiliary_bytes': 0,
                           'index_bytes': 8192, 'toast_main_bytes': 8192,
                           'toast_auxiliary_bytes': 0, 'toast_index_bytes': 8192,
                           'total_bytes': 32768}
                          for name in ('item_revisions', 'items', 'operations',
                                       'operation_payloads', 'item_field_uses')]
                         + [{'name': 'kehila_auth.pending_items', 'reloptions': None,
                             'heap_main_bytes': 8192, 'heap_auxiliary_bytes': 0,
                             'index_bytes': 8192, 'toast_main_bytes': 0,
                             'toast_auxiliary_bytes': 0, 'toast_index_bytes': 8192,
                             'total_bytes': 24576}],
            'column_storage': [{'relation': 'kehila.item_revisions', 'column': 'history',
                                'storage': 'x', 'compression': ''}],
            'logical': {'revisions': 21, 'logical_history_text_bytes': 100000,
                        'minimum_history_text_bytes': 4000, 'maximum_history_text_bytes': 5000,
                        'replay_payloads': 22, 'logical_request_bytes': 10000,
                        'logical_result_text_bytes': 90000,
                        'pending_creation_identities': 1, 'pending_creation_payloads': 1,
                        'logical_pending_request_bytes': 500},
            'items': 1, 'operations': 22, 'retired_operations': 0, 'field_uses': 4}


class StorageGrowthResultTests(unittest.TestCase):
    def test_retirement_keeps_pending_content_until_the_separate_prune(self):
        baseline = observation()
        baseline['logical']['logical_request_bytes'] = 1
        baseline['logical']['logical_result_text_bytes'] = 2
        previous = observation()
        retired = observation()
        retired['retired_operations'] = 21
        retired['logical'].update(replay_payloads=1, logical_request_bytes=1, logical_result_text_bytes=2)
        validate_checkpoint(retired, select([0])[0], 'retired', 21, baseline, previous)
        wrong = deepcopy(retired)
        wrong['logical'].update(pending_creation_payloads=0, logical_pending_request_bytes=0)
        with self.assertRaises(ValueError):
            validate_checkpoint(wrong, select([0])[0], 'retired', 21, baseline, previous)
        pruned = deepcopy(wrong)
        validate_checkpoint(pruned, select([0])[0], 'pruned', 21, baseline, retired)
        with self.assertRaises(ValueError):
            validate_checkpoint(retired, select([0])[0], 'pruned', 21, baseline, retired)
        wrong = deepcopy(retired)
        wrong['logical']['logical_history_text_bytes'] -= 100
        with self.assertRaises(ValueError):
            validate_checkpoint(wrong, select([0])[0], 'retired', 21, baseline, previous)

    def test_reconciled_census_keeps_toast_indexes_separate_from_main_indexes(self):
        value = observation()
        validate_census(value)
        self.assertEqual(value['relations'][0]['total_bytes'], 32768)

    def test_accounting_corruption_cannot_be_hidden_in_an_otherwise_valid_census(self):
        for field, replacement in [('total_bytes', 40960), ('index_bytes', -1),
                                   ('heap_main_bytes', True)]:
            value = observation()
            value['relations'][0][field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_census(value)
        value = observation()
        value['relations'].append(deepcopy(value['relations'][0]))
        with self.assertRaises(ValueError):
            validate_census(value)
        value = observation()
        value['relations'].pop(0)
        with self.assertRaises(ValueError):
            validate_census(value)

    def test_wal_generated_is_lsn_delta_not_directory_allocation(self):
        before = observation()
        after = deepcopy(before)
        after['wal_insert_lsn'] = '0/1002000'
        self.assertEqual(wal_delta(before, after), 8192)
        self.assertEqual(after['wal_directory_allocated_bytes'],
                         before['wal_directory_allocated_bytes'])

    def test_wal_rejects_reset_replaced_cluster_or_changed_settings(self):
        before = observation()
        for field, replacement in [('cluster_system_identifier', '987654321'),
                                   ('database_oid', 16385), ('wal_insert_lsn', '0/FFFFFF'),
                                   ('wal_insert_lsn', 'not-an-lsn')]:
            after = deepcopy(before)
            after[field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                wal_delta(before, after)
        after = deepcopy(before)
        after['postgres']['wal_compression'] = 'on'
        with self.assertRaises(ValueError):
            wal_delta(before, after)


if __name__ == '__main__':
    unittest.main()
