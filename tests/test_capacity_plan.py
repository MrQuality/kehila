"""Finite workload admission and stable cell identity, without running services."""
from dataclasses import FrozenInstanceError
from itertools import product
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import capacity_plan as plan


class CapacityPlanTests(unittest.TestCase):
    def test_exact_matrix_covers_every_combination_once(self):
        cells = plan.cells()
        self.assertEqual(len(cells), 180)
        self.assertEqual([cell.index for cell in cells], list(range(180)))
        expected = set(product(('compact', 'large'), ('shared', 'independent'),
                               ('read', 'edit', 'mixed'), (1, 2, 4, 8, 16), (1, 2, 3)))
        actual = {(cell.profile, cell.layout, cell.mix, cell.concurrency, cell.repetition)
                  for cell in cells}
        self.assertEqual(actual, expected)
        self.assertEqual(len(actual), len(cells))
        self.assertEqual(sum(cell.document()['measured_requests'] for cell in cells), 18_000)
        self.assertEqual(sum(cell.document()['warmup_requests'] for cell in cells), 1_800)

    def test_cell_admission_rejects_coercion_and_out_of_range_values(self):
        for invalid in (True, False, -1, 180, 1.0, '1', None):
            with self.subTest(value=invalid), self.assertRaises(ValueError):
                plan.cell(invalid)
        self.assertEqual(plan.cell(0), plan.cells()[0])
        self.assertEqual(plan.cell(179), plan.cells()[-1])

    def test_selection_is_finite_unique_and_canonical(self):
        self.assertEqual(plan.select((179, 0, 12)),
                         (plan.cell(0), plan.cell(12), plan.cell(179)))
        for invalid in ((), [], (0, 0), (True,), (-1,), (180,), '0', {0, 1}, None):
            with self.subTest(value=invalid), self.assertRaises(ValueError):
                plan.select(invalid)

    def test_profile_documents_preserve_accepted_dimensions_and_synthetic_boundary(self):
        for cell in plan.cells():
            document = cell.document()
            self.assertEqual(document['schema'], 1)
            self.assertEqual(document['index'], cell.index)
            self.assertEqual(document['profile'], cell.profile)
            self.assertIs(document['synthetic_configuration'], True)
            self.assertEqual(document['fields_per_item'], 4 if cell.profile == 'compact' else 128)
            self.assertEqual(document['text_bytes_per_field'], 64 if cell.profile == 'compact' else 1024)
            self.assertEqual(document['projects'], 1 if cell.layout == 'shared' else 4)
            self.assertEqual(document['seeded_items'], 16)
            self.assertEqual(document['authenticated_sessions'], 4)
            self.assertEqual(document['warmup_requests'], 10)
            self.assertEqual(document['measured_requests'], 100)
            self.assertEqual(document['concurrency'], cell.concurrency)
            self.assertEqual(document['mix'], cell.mix)
            self.assertEqual(document['repetition'], cell.repetition)
        self.assertEqual({cell.profile for cell in plan.cells()}, {'compact', 'large'})

    def test_cell_identity_cannot_be_mutated_or_rebound(self):
        cell = plan.cell(0)
        with self.assertRaises(FrozenInstanceError):
            cell.concurrency = 16
        with self.assertRaises(ValueError):
            plan.Cell(0, 'large', 'shared', 'read', 1, 1)


if __name__ == '__main__':
    unittest.main()
