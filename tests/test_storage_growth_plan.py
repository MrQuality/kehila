"""Finite storage sampling must stay inside its admitted revision envelope."""
from pathlib import Path
import sys
import unittest
from itertools import product

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import storage_growth_plan as plan


class StorageGrowthPlanTests(unittest.TestCase):
    def test_all_sample_identities_match_the_recorded_campaign(self):
        expected = list(product(('compact', 'large', 'near_boundary'),
                                ('repeated', 'sha256_hex'), ('one_field', 'all_fields'), (1, 2)))
        self.assertEqual([(sample.profile, sample.entropy, sample.edit, sample.repetition)
                          for sample in plan.samples()], expected)

    def test_campaign_covers_size_compression_and_edit_locality_with_repetitions(self):
        samples = plan.samples()
        self.assertEqual(len(samples), 24)
        self.assertEqual(sum(sample.revisions for sample in samples), 1008)
        for profile in ('compact', 'large', 'near_boundary'):
            selected = [sample for sample in samples if sample.profile == profile]
            self.assertEqual({(sample.entropy, sample.edit, sample.repetition)
                              for sample in selected},
                             {(entropy, edit, repetition)
                              for entropy in ('repeated', 'sha256_hex')
                              for edit in ('one_field', 'all_fields')
                              for repetition in (1, 2)})
        self.assertTrue(all(sample.items == 2 and sample.edits_per_item == 20
                            and sample.revisions == 42 for sample in samples))
        for sample in samples:
            document = sample.document()
            self.assertEqual(document['roles'], ['native', 'historical'])
            self.assertEqual(document['items_per_role'], 1)
            self.assertEqual(len(document['roles']) * document['revisions_per_role'],
                             sample.revisions)

    def test_near_boundary_is_a_calibration_requirement_not_an_assumed_field_size(self):
        boundary = next(sample for sample in plan.samples() if sample.profile == 'near_boundary')
        self.assertIsNone(boundary.text_bytes)
        self.assertEqual(boundary.document()['minimum_history_bytes'], 943_719)
        self.assertEqual(boundary.document()['maximum_envelope_bytes'], 1_048_576)
        compact = plan.samples()[0]
        self.assertEqual((compact.fields, compact.text_bytes), (4, 64))
        large = next(sample for sample in plan.samples() if sample.profile == 'large')
        self.assertEqual((large.fields, large.text_bytes), (128, 1024))

    def test_only_explicit_nonrepeating_finite_selection_is_admitted(self):
        self.assertEqual([sample.index for sample in plan.select([23, 0])], [0, 23])
        for invalid in ([], [0, 0], [True], [-1], [24], ['0'], '0', {0}, None):
            with self.subTest(value=invalid), self.assertRaises(ValueError):
                plan.select(invalid)

    def test_plan_is_immutable_and_cannot_rebind_a_sample_identity(self):
        from dataclasses import FrozenInstanceError, replace
        sample = plan.samples()[0]
        with self.assertRaises(FrozenInstanceError):
            sample.items = 1000
        with self.assertRaises(ValueError):
            replace(sample, profile='large')
        with self.assertRaises(ValueError):
            replace(sample, items=3)


if __name__ == '__main__':
    unittest.main()
