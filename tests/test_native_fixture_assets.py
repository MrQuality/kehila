"""Ownership and evidence transfer for generated, disposable fixture assets."""
from pathlib import Path
import os
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import native_fixture_assets as assets

OWNER = 'a' * 32


class FixtureAssetTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.parent = Path(self.temporary.name)
        self.work = self.parent / 'native-authority'
        self.work.mkdir()
        assets.write_owner(self.work, OWNER)
        (self.work / 'controls').mkdir()
        (self.work / 'controls' / 'result.json').write_text('{"passed":true}', encoding='utf-8')

    def test_manifest_verifies_a_complete_copy_and_rejects_mutated_evidence(self):
        digest = assets.write_manifest(self.work, OWNER)
        exported = self.parent / 'export'
        shutil.copytree(self.work, exported)
        assets.verify_export(exported, OWNER, digest)
        (exported / 'controls' / 'result.json').write_text('{"passed":false}', encoding='utf-8')
        with self.assertRaises(ValueError):
            assets.verify_export(exported, OWNER, digest)

    def test_unknown_owner_missing_file_extra_file_and_wrong_digest_are_rejected(self):
        digest = assets.write_manifest(self.work, OWNER)
        with self.assertRaises(ValueError):
            assets.verify_export(self.work, 'b' * 32, digest)
        with self.assertRaises(ValueError):
            assets.verify_export(self.work, OWNER, '0' * 64)
        file = self.work / 'controls' / 'result.json'
        original = file.read_bytes()
        file.unlink()
        with self.assertRaises(ValueError):
            assets.verify_export(self.work, OWNER, digest)
        file.write_bytes(original)
        (self.work / 'extra').write_bytes(b'unexpected')
        with self.assertRaises(ValueError):
            assets.verify_export(self.work, OWNER, digest)

    def test_ownership_cannot_be_claimed_over_existing_material_or_replaced(self):
        with self.assertRaises(ValueError):
            assets.write_owner(self.work, OWNER)
        occupied = self.parent / 'occupied'
        occupied.mkdir()
        (occupied / 'unowned').write_bytes(b'preserve')
        with self.assertRaises(ValueError):
            assets.write_owner(occupied, OWNER)
        for invalid in ('', 'a' * 31, 'A' * 32, None):
            fresh = self.parent / ('fresh-' + str(len(str(invalid))))
            fresh.mkdir(exist_ok=True)
            with self.subTest(owner=invalid), self.assertRaises(ValueError):
                assets.write_owner(fresh, invalid)
        self.assertEqual((occupied / 'unowned').read_bytes(), b'preserve')

    @unittest.skipUnless(os.name == 'posix', 'Linux fixture link and special-file controls')
    def test_links_and_special_files_cannot_be_exported_as_owned_regular_assets(self):
        outside = self.parent / 'outside'
        outside.write_bytes(b'outside')
        link = self.work / 'link'
        link.symlink_to(outside)
        with self.assertRaises(ValueError):
            assets.write_manifest(self.work, OWNER)
        link.unlink()
        os.link(outside, link)
        with self.assertRaises(ValueError):
            assets.write_manifest(self.work, OWNER)
        link.unlink()
        os.mkfifo(link)
        with self.assertRaises(ValueError):
            assets.write_manifest(self.work, OWNER)
        self.assertEqual(outside.read_bytes(), b'outside')

    def test_reset_cannot_be_redirected_to_another_directory(self):
        digest = assets.write_manifest(self.work, OWNER)
        with patch.object(assets, 'WORK', self.work), self.assertRaises(ValueError):
            assets.reset(OWNER, digest)
        self.assertTrue((self.work / 'controls' / 'result.json').is_file())

    def test_source_and_actual_build_output_must_be_separate_from_generated_assets(self):
        source = self.parent / 'public-source'
        target = self.parent / 'build-target'
        source.mkdir()
        target.mkdir()
        self.assertEqual(assets._separate_paths(self.work, source, target), self.work)
        for protected_source, protected_target in (
                (self.work, target), (self.parent, target),
                (source, self.work / 'controls'), (source, self.parent)):
            with self.subTest(source=protected_source, target=protected_target), self.assertRaises(ValueError):
                assets._separate_paths(self.work, protected_source, protected_target)

    @unittest.skipUnless(os.name == 'posix', 'Linux fixture target-alias control')
    def test_build_output_alias_into_generated_assets_is_rejected_without_deletion(self):
        source = self.parent / 'public-source'
        source.mkdir()
        target = self.parent / 'build-target'
        target.symlink_to(self.work / 'controls', target_is_directory=True)
        with self.assertRaises(ValueError):
            assets._separate_paths(self.work, source, target)
        self.assertTrue((self.work / 'controls' / 'result.json').is_file())


if __name__ == '__main__':
    unittest.main()
