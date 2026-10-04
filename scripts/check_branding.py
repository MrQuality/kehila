#!/usr/bin/env python3
"""Check current project naming; factual third-party references are allowed."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
OLD = "ya" + "ja"
# Split spellings so the checker can check its own source.
RETIRED = re.compile("|".join(re.escape(term) for term in (
    "Yet Another " + "Jira Alternative",
    "jql" + "_core",
    "Compiled" + "JqlArtifact",
    OLD + "-jql",
)) + r"|(?<![a-z0-9])" + OLD + r"(?![a-z0-9])", re.IGNORECASE)

# Exceptions permit only the retained token in its documented context. They do
# not exempt a file: an obsolete launch variable beside a retained DB still fails.
STORAGE = (
    rf"^\s*name: {OLD}$",
    rf"^\s*POSTGRES_DB: {OLD}$",
    rf"(?<=pg_isready -U postgres -d ){OLD}(?=[\"'])",
)
SP001_FILES = {
    "experiments/SP-001/README.md", "experiments/SP-001/application.properties",
    "experiments/SP-001/cases.py", "experiments/SP-001/compose.yml",
    "experiments/SP-001/manage.py", "docs/spikes/SP-001-task-path.md",
}
MIGRATION_ROWS = (
    f"| `{OLD.upper()}_*` | `KEHILA_*` (same suffix) |",
    f"| `{OLD}_query` / `src/pure/{OLD}_query/` | `kehila_query` / `src/pure/kehila_query/` |",
    f"| `{OLD}/task_api` | `kehila/task_api` |",
    f"| `{OLD}.local/sync_contract` | `kehila.local/sync_contract` |",
    f"| `{OLD}` / `@{OLD}/contracts` | `kehila` / `@kehila/contracts` |",
    f"| `docs/reference/{OLD.upper()}-v0.2.md` | `docs/reference/Kehila-v0.2.md` |",
    f"| `MrQuality/{OLD}` | `MrQuality/kehila` |",
)


def permitted_spans(name, text):
    patterns = []
    if name == "docker-compose.yml":
        patterns.extend(STORAGE)
        patterns.append(rf"(?<=FERRETDB_POSTGRESQL_URL: postgres://postgres:postgres@postgres:5432/){OLD}(?=$)")
    if name == ".gitignore":
        patterns.append(rf"^\.{OLD}/$")
    if name == "src/io/task_worker/src/main.rs":
        patterns.append(rf'(?<=env::var\("KEHILA_TASK_DB"\)\.unwrap_or_else\(\|_\| "){OLD}(?="\.into\(\)\);$)')
    if name == "tests/integration/task_path.py":
        patterns.append(rf"(?<=KEHILA_TASK_DB='){OLD}(?=',)")
    if name == "docs/TESTING.md":
        patterns.append(rf"(?<=the `){OLD}(?=` database\.)")
    if name == "docs/spikes/README.md":
        patterns.append(rf"\.{OLD}/spikes/SP-NNN/")
    if name in SP001_FILES:
        patterns.extend((rf"(?<![a-z0-9]){OLD}-sp001(?:-(?:probe|cdc):experiment)?(?![a-z0-9_-])",
                         rf"(?<=startswith\('){OLD}-sp001_(?='\))",
                         rf"\.{OLD}/spikes/SP-001(?=/|['`\"])"))
        if name == "experiments/SP-001/compose.yml":
            patterns.extend(STORAGE)
            patterns.append(rf"(?<=FERRETDB_POSTGRESQL_URL: postgres://postgres:disposable-sp001@postgres:5432/){OLD}(?=$)")
        if name == "experiments/SP-001/application.properties":
            patterns.append(rf"(?<=debezium.source.database.dbname=){OLD}(?=$)")
        if name == "experiments/SP-001/manage.py":
            patterns.append(rf"(?<='-d', '){OLD}(?=')")
    if name == "docs/BRANDING.md":
        if text.strip() in MIGRATION_ROWS:
            return [(0, len(text))]
        patterns.extend((rf"(?<=database remain `){OLD}(?=`\.)",
                         rf"(?<=uses `){OLD}-sp001(?=`)",
                         rf"(?<=`)\.{OLD}/spikes/SP-001/(?=`)",
                         rf"(?<=- `)\.{OLD}/(?=` remains ignored)"))
    return [match.span() for pattern in patterns for match in re.finditer(pattern, text)]


def check_paths(root, paths):
    errors = []
    for name in sorted(paths):
        if RETIRED.search(name):
            errors.append(f"{name}: retired identifier in path")
        data = (root / name).read_bytes()
        if b"\x00" in data:
            continue
        try:
            content = data.decode("utf-8")
        except UnicodeDecodeError:
            continue
        for line, text in enumerate(content.splitlines(), 1):
            spans = permitted_spans(name, text)
            if any(not any(start <= match.start() and match.end() <= end for start, end in spans)
                   for match in RETIRED.finditer(text)):
                errors.append(f"{name}:{line}: retired project naming")
    return errors


def source_paths(root):
    """Explicit public source inventory, also usable in Git-free hook snapshots."""
    files = [path for path in root.iterdir() if path.is_file()
             and not path.name.startswith(".") and path.name != "AGENTS.md"]
    for name in (".gitignore", ".gitattributes", ".env.example", ".githooks/pre-commit",
                 ".githooks/pre-commit.bat", ".githooks/pre_commit.py"):
        path = root / name
        if path.is_file():
            files.append(path)
    for directory in ("src", "packages", "go", "docs", "scripts", "tests", ".github", "experiments"):
        files.extend(path for path in (root / directory).rglob("*") if path.is_file()
                     and not {"__pycache__", "node_modules", "target", ".git"}.intersection(path.relative_to(root).parts)
                     and (not path.name.startswith(".env") or path.name == ".env.example")
                     and path.name != "AGENTS.md")
    return sorted(path.relative_to(root).as_posix() for path in files
                  if path.relative_to(root).as_posix() != "tests/test_branding.py")


def main():
    errors = check_paths(ROOT, source_paths(ROOT))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Current project naming verified")
    return 0


if __name__ == "__main__":
    sys.exit(main())
