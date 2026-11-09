import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import container_capacity as capacity
from capacity_plan import select


class Executor:
    def __init__(self): self.events = []
    def execute(self, command): self.events.append(('compile', command))
    def finish_builds(self): self.events.append(('finish',))
    def verify_source(self): self.events.append(('verify',))


class CapacityBatchTests(unittest.TestCase):
    def test_observer_order_rotates_without_dropping_or_duplicating_cells(self):
        jobs = capacity.observation_jobs(select([0,1,2]),'sampled',True)
        self.assertEqual([(entry.index,mode) for entry,mode in jobs], [
            (0,'off'),(0,'stages'),(0,'sampled'),(1,'stages'),(1,'sampled'),(1,'off'),
            (2,'sampled'),(2,'off'),(2,'stages')])
        self.assertEqual(len({(entry.index,mode) for entry,mode in jobs}),9)

    def test_calibration_is_separate_single_sampled_serial_edit_selection(self):
        self.assertEqual(capacity.selection([15],'sampled',True)[0].index,15)
        for indices, observer in (([0],'sampled'),([15,16],'sampled'),([15],'off')):
            with self.subTest(indices=indices,observer=observer),self.assertRaises(ValueError):
                capacity.selection(indices,observer,True)

    def test_compiles_once_and_retains_receipts_before_next_fresh_cell(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory).resolve()
            executor = Executor()
            def check(selected, **kwargs):
                self.assertIs(selected, executor)
                index = kwargs['capacity_cell']
                if index == 1: self.assertTrue((output / 'cell-000-receipt.json').is_file())
                self.assertTrue(kwargs['reset_assets'])
                self.assertEqual(kwargs['capacity_observer'], 'sampled')
                self.assertEqual(kwargs['capacity_oracle_control'], index == 0)
                return {'cell_index': index, 'reset_completed': True}
            with patch.object(capacity, 'check', side_effect=check), patch.object(capacity,'read_measurement',
                    return_value={'source_sha256':'a'*64,'environment':{}}) as read:
                capacity.batch(executor, select([1, 0]), 'sampled', output, oracle_control=True)
            self.assertEqual(read.call_count,2)
            self.assertEqual([event[0] for event in executor.events], ['compile', 'finish', 'verify', 'verify'])
            self.assertEqual(json.loads((output / 'batch-complete.json').read_text())['cells'], [0, 1])

    def test_cell_failure_preserves_previous_receipt_and_never_marks_batch_complete(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory).resolve()
            def check(executor, **kwargs):
                if kwargs['capacity_cell'] == 1: raise RuntimeError('independent state failed')
                return {'cell_index': 0}
            with patch.object(capacity, 'check', side_effect=check), patch.object(capacity,'read_measurement',
                    return_value={'source_sha256':'a'*64,'environment':{}}), self.assertRaises(RuntimeError):
                capacity.batch(Executor(), select([0, 1, 2]), 'off', output)
            self.assertTrue((output / 'cell-000-receipt.json').exists())
            self.assertFalse((output / 'batch-complete.json').exists())
            self.assertFalse((output / 'cell-002-receipt.json').exists())

    def test_output_rejects_existing_source_and_git_destinations(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / 'source'; source.mkdir()
            for path in (root, source / 'results', root / 'missing' / 'results'):
                with self.subTest(path=path), self.assertRaises(ValueError): capacity.admit_output(path, source)
            destination = root / 'results'
            self.assertEqual(capacity.admit_output(destination, source), destination)
            (root / '.git').mkdir()
            with self.assertRaises(ValueError): capacity.admit_output(destination, source)


if __name__ == '__main__': unittest.main()
