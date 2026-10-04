"""Keep retired product naming out of current source and public documentation."""
from pathlib import Path
import importlib.util
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("branding", ROOT / "scripts/check_branding.py")
BRANDING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BRANDING)


class BrandingTests(unittest.TestCase):
    def check_text(self, text, name="README.md"):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
            return BRANDING.check_paths(root, [name])

    def test_rejects_retired_expansion_case_insensitively(self):
        self.assertTrue(self.check_text("Yet Another JIRA Alternative"))

    def test_rejects_retired_identifiers_in_content_and_paths(self):
        for name in ("jql_core", "CompiledJqlArtifact", "yaja-jql"):
            with self.subTest(name=name):
                self.assertTrue(self.check_text("use " + name))
                self.assertTrue(self.check_text("", "src/" + name + "/lib.rs"))

    def test_allows_factual_references_and_current_names(self):
        self.assertFalse(self.check_text(
            "KEHILA — Project and task management. kehila_query CompiledQueryArtifact. "
            "Jira is a trademark of Atlassian. No JQL compatibility is claimed."))

    def test_skips_binary_files_and_reports_text_line(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "asset.bin").write_bytes(b"\x00\xff")
            self.assertEqual(BRANDING.check_paths(root, ["asset.bin"]), [])
        self.assertIn("README.md:2:", self.check_text("Heading\njql_core")[0])


if __name__ == "__main__":
    unittest.main()
