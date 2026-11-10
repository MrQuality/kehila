"""Finite storage batch admission, receipts and stop-on-failure behavior."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import container_storage_growth as storage
from storage_growth_plan import select


class Executor:
    def __init__(self): self.events = []
    def execute(self, command): self.events.append(('execute', command))
    def finish_builds(self): self.events.append(('finish',))
    def verify_source(self): self.events.append(('verify',))


class StorageBatchTests(unittest.TestCase):
    def test_extended_profile_is_recorded_and_cannot_expand_to_other_samples(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory).resolve()
            with patch.object(storage, 'preflight', return_value={}), \
                 patch.object(storage, 'check', return_value={}) as check, \
                 patch.object(storage, 'read_verified_sample', return_value={}):
                storage.batch(Executor(), select([20]), output, storage_profile='extended')
            self.assertEqual(check.call_args.kwargs['storage_profile'], 'extended')
            document = json.loads((output / 'batch-plan.json').read_text())
            self.assertEqual(document['database_memory_bytes'], 512*1024**2)
            self.assertEqual(document['database_tmpfs_bytes'], 512*1024**2)
            self.assertEqual(document['database_stop_margin_bytes'], 64*1024**2)
        with tempfile.TemporaryDirectory() as directory:
            executor = Executor()
            with self.assertRaises(ValueError):
                storage.batch(executor, select([19]), Path(directory), storage_profile='extended')
            self.assertEqual(executor.events, [])

    def test_finite_samples_keep_receipts_before_next_owned_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory).resolve()
            executor = Executor()
            def check(selected, **kwargs):
                self.assertIs(selected, executor)
                if kwargs['storage_sample'] == 1:
                    self.assertTrue((output / 'sample-00-receipt.json').is_file())
                self.assertTrue(kwargs['reset_assets'])
                return {'storage_sample_index': kwargs['storage_sample']}
            with patch.object(storage, 'preflight', return_value={}), \
                 patch.object(storage, 'check', side_effect=check), \
                 patch.object(storage, 'read_verified_sample', return_value={'same': 'binding'}):
                storage.batch(executor, select([1, 0]), output)
            document = json.loads((output / 'batch-plan.json').read_text())
            self.assertEqual(document['maximum_item_revisions'], 84)
            self.assertEqual([sample['index'] for sample in document['samples']], [0, 1])
            self.assertTrue((output / 'batch-complete.json').is_file())
            self.assertEqual([event[0] for event in executor.events],
                             ['execute', 'execute', 'finish', 'verify', 'verify'])

    def test_failure_stops_later_samples_and_does_not_claim_batch_completion(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory).resolve()
            def check(_executor, **kwargs):
                if kwargs['storage_sample'] == 1:
                    raise ValueError('independent state differs')
                return {'storage_sample_index': 0}
            with patch.object(storage, 'preflight', return_value={}), \
                 patch.object(storage, 'check', side_effect=check), \
                 patch.object(storage, 'read_verified_sample', return_value={}), self.assertRaises(ValueError):
                storage.batch(Executor(), select([0, 1, 2]), output)
            self.assertTrue((output / 'sample-00-receipt.json').is_file())
            self.assertFalse((output / 'sample-02-preflight.json').exists())
            self.assertFalse((output / 'batch-complete.json').exists())

    def test_source_or_profile_drift_stops_before_the_next_sample(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory).resolve()
            with patch.object(storage, 'preflight', return_value={}), \
                 patch.object(storage, 'check', return_value={}), \
                 patch.object(storage, 'read_verified_sample', side_effect=[{'source': 'a'}, {'source': 'b'}]), \
                 self.assertRaisesRegex(ValueError, 'source or execution profile changed'):
                storage.batch(Executor(), select([0, 1, 2]), output)
            self.assertFalse((output / 'sample-02-preflight.json').exists())


if __name__ == '__main__':
    unittest.main()
