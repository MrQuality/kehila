"""Storage reporting must keep physical, logical and lifecycle evidence apart."""
from copy import deepcopy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from storage_growth_report import growth_rows, complete_matrix, complete_selection, coverage, model_comparison
from storage_growth_plan import samples
from test_storage_growth_results import observation
from test_storage_history_model import histories


class StorageGrowthReportTests(unittest.TestCase):
    def test_explicit_cohorts_require_exact_disjoint_coverage_and_common_storage_contracts(self):
        def report(name, indices, memory, source):
            binding = {'source_inventory_sha256': source, 'storage_contract_sha256': 'a'*64,
                       'postgres': {'version': '16.15'}, 'column_storage': ['same'],
                       'execution_profile': {'runner_image': 'same', 'database_memory_bytes': memory,
                                             'database_tmpfs': {'/var/lib/postgresql/data':
                                                 'rw,size=256m' if memory == 256*1024**2 else 'rw,size=512m'}}}
            return {'cohort': name, 'selection': indices, 'binding': binding,
                    'matrix_complete': False,
                    'pairs': [{'sample': samples()[index].document(), 'rows': []} for index in indices]}
        reports = [report('baseline', list(range(20)), 256*1024**2, 'b'*64),
                   report('near_sha', list(range(20,24)), 512*1024**2, 'c'*64)]
        self.assertEqual(coverage(reports)['completed_pairs'], 24)
        for change in ('missing', 'contract', 'profile'):
            broken = deepcopy(reports)
            if change == 'missing':
                broken[1]['selection'].pop()
                broken[1]['pairs'].pop()
            elif change == 'contract':
                broken[1]['binding']['storage_contract_sha256'] = 'd'*64
            else:
                broken[1]['binding']['execution_profile']['runner_image'] = 'different'
            with self.subTest(change=change), self.assertRaises(ValueError):
                coverage(broken)

    def test_explicit_subset_does_not_silently_make_a_partial_default_report_complete(self):
        values = [{'sample': sample.document(), 'binding': {'source': 'a'}, 'rows': []}
                  for sample in samples()[:20]]
        self.assertEqual(len(complete_selection(values, list(range(20)))), 20)
        with self.assertRaises(ValueError):
            complete_selection(values[:-1], list(range(20)))
        with self.assertRaises(ValueError):
            complete_matrix(values)

    def test_growth_reports_allocation_and_logical_deltas_without_inventing_reclamation(self):
        before = observation()
        after = deepcopy(before)
        after['logical'].update(revisions=22, logical_history_text_bytes=105000)
        after['wal_insert_lsn'] = '0/1002000'
        rows = growth_rows(samples()[0], 'native', [('edit-20', before), ('extra-control', after)])
        self.assertEqual(rows[1]['history_text_growth_bytes'], 5000)
        self.assertEqual(rows[1]['relation_growth_bytes'], 0)
        self.assertEqual(rows[1]['generated_wal_bytes'], 8192)
        self.assertEqual(rows[1]['wal_directory_growth_bytes'], 0)

    def test_retirement_is_reported_as_a_separate_stage_with_no_per_edit_rate(self):
        before = observation()
        retired = deepcopy(before)
        retired['logical'].update(replay_payloads=1, logical_request_bytes=1, logical_result_text_bytes=2)
        rows = growth_rows(samples()[0], 'historical', [('edit-20', before), ('retired', retired)])
        self.assertEqual(rows[1]['revisions_added'], 0)
        self.assertIsNone(rows[1]['relation_bytes_per_added_revision'])
        self.assertLess(rows[1]['request_growth_bytes'], 0)
        self.assertEqual(rows[1]['relation_growth_bytes'], 0)

    def test_complete_report_rejects_missing_duplicate_or_mixed_source_samples(self):
        values = [{'sample': sample.document(), 'binding': {'source': 'a'}, 'rows': []}
                  for sample in samples()]
        self.assertEqual(len(complete_matrix(values)), 24)
        for broken in (values[:-1], values + [values[0]],
                       [dict(value, binding={'source': 'b'}) if index == 5 else value
                        for index, value in enumerate(values)]):
            with self.subTest(population=len(broken)), self.assertRaises(ValueError):
                complete_matrix(broken)

    def test_model_compares_declared_logical_encoder_and_proves_every_reconstruction(self):
        records = histories()
        comparison = model_comparison(records)
        self.assertEqual(comparison['histories'], 21)
        self.assertEqual(comparison['maximum_records_read'], 10)
        self.assertLess(comparison['modeled_encoded_bytes'], comparison['full_encoded_bytes'])
        self.assertEqual(comparison['physical_savings'], 'unmeasured')


if __name__ == '__main__':
    unittest.main()
