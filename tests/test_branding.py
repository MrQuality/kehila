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
        self.assertFalse(self.check_text("kayajan and yajan are unrelated words."))

    def test_skips_binary_files_and_reports_text_line(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "asset.bin").write_bytes(b"\x00\xff")
            self.assertEqual(BRANDING.check_paths(root, ["asset.bin"]), [])
        self.assertIn("README.md:2:", self.check_text("Heading\njql_core")[0])

    def test_rejects_rename_identifiers_case_insensitively(self):
        for name in ("yaja_query", "@yaja/contracts", "YAJA_MONGO_URL", "yaja/task_api",
                     "yaja.local/sync_contract", "YAJA-v0.2.md", "YAJA", "MrQuality/yaja"):
            with self.subTest(name=name):
                self.assertTrue(self.check_text(name.swapcase()))
                self.assertTrue(self.check_text("", "docs/" + name + ".txt"))

    def test_storage_exception_does_not_allow_old_environment(self):
        name = "src/io/task_worker/src/main.rs"
        self.assertFalse(self.check_text(
            'let database = env::var("KEHILA_TASK_DB").unwrap_or_else(|_| "yaja".into());', name))
        self.assertTrue(self.check_text(
            'let database = env::var("YAJA_TASK_DB").unwrap_or_else(|_| "yaja".into());', name))
        self.assertTrue(self.check_text('let title = "YAJA";', name))

    def test_retained_identities_are_path_and_token_scoped(self):
        for name, text in ((".gitignore", ".yaja/"),
                           ("docker-compose.yml", "name: yaja"),
                           ("docker-compose.yml", "POSTGRES_DB: yaja"),
                           ("docker-compose.yml", 'test: ["CMD-SHELL", "pg_isready -U postgres -d yaja"]'),
                           ("docker-compose.yml", "FERRETDB_POSTGRESQL_URL: postgres://postgres:postgres@postgres:5432/yaja"),
                           ("experiments/SP-001/compose.yml", "name: yaja-sp001"),
                           ("experiments/SP-001/compose.yml", "image: localhost/yaja-sp001-cdc:experiment"),
                           ("experiments/SP-001/application.properties", "debezium.source.database.dbname=yaja"),
                           ("experiments/SP-001/manage.py", "['-d', 'yaja', '-c', ddl]"),
                           ("experiments/SP-001/manage.py", "x['name'].startswith('yaja-sp001_')"),
                           ("experiments/SP-001/cases.py", "folder = ROOT / '.yaja/spikes/SP-001'"),
                           ("experiments/SP-001/compose.yml", "FERRETDB_POSTGRESQL_URL: postgres://postgres:disposable-sp001@postgres:5432/yaja"),
                           ("tests/integration/task_path.py", "environment = dict(os.environ, KEHILA_TASK_DB='yaja',"),
                           ("docs/TESTING.md", "the `yaja` database."),
                           ("docs/BRANDING.md", "- The development Compose project and database remain `yaja`. The worker's"),
                           ("docs/BRANDING.md", "- The retained SP-001 reproduction uses `yaja-sp001` for its isolated Compose"),
                           ("docs/BRANDING.md", "  `.yaja/spikes/SP-001/` for local evidence. No machine or volume is renamed."),
                           ("docs/BRANDING.md", "- `.yaja/` remains ignored so existing local artifacts stay excluded."),
                           ("docs/spikes/README.md", "`.yaja/spikes/SP-NNN/`")):
            with self.subTest(name=name):
                self.assertFalse(self.check_text(text, name))
                self.assertTrue(self.check_text(text, "README.md"))
                self.assertTrue(self.check_text(text + " YAJA_QUERY", name))

    def test_storage_and_spike_exceptions_do_not_allow_unrelated_names(self):
        self.assertTrue(self.check_text("image: MrQuality/yaja", "docker-compose.yml"))
        self.assertTrue(self.check_text("image: localhost/yaja-sp001-other:experiment",
                                        "experiments/SP-001/compose.yml"))

    def test_migration_rows_are_exact_exceptions(self):
        rows = (ROOT / "docs/BRANDING.md").read_text(encoding="utf-8").splitlines()
        migration_rows = [row for row in rows if row.startswith("| `") and "yaja" in row.lower()]
        self.assertEqual(len(migration_rows), 7)
        for row in migration_rows:
            with self.subTest(row=row):
                self.assertFalse(self.check_text(row, "docs/BRANDING.md"))
                self.assertTrue(self.check_text(row + " YAJA", "docs/BRANDING.md"))
                self.assertTrue(self.check_text(row[:-1] + "changed |", "docs/BRANDING.md"))
                self.assertTrue(self.check_text(row, "README.md"))

    def test_inventory_works_without_git_and_excludes_local_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wanted = {"README.md", ".gitignore", ".env.example", ".githooks/pre_commit.py",
                      ".github/workflows/ci.yml", "experiments/SP-001/compose.yml", "packages/demo/.env.example"}
            excluded = {"AGENTS.md", ".env", ".githooks/pre-push", ".yaja/evidence.txt",
                        "target/build.txt", "packages/contracts/node_modules/fixture.txt",
                        "tests/__pycache__/fixture.pyc", "tests/test_branding.py", "packages/demo/.env.local"}
            for name in wanted | excluded:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("", encoding="utf-8")
            self.assertEqual(set(BRANDING.source_paths(root)), wanted)


if __name__ == "__main__":
    unittest.main()
