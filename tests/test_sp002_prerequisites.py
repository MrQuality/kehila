import importlib.util
import json
import subprocess
import sys
from pathlib import Path
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "experiments/SP-002"
SPEC = importlib.util.spec_from_file_location("phase_prerequisites", SOURCE / "phase_prerequisites.py")
PREREQUISITES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PREREQUISITES)


class PhasePrerequisiteTests(unittest.TestCase):
    def test_missing_artifacts_name_the_ordered_runner(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaisesRegex(RuntimeError, "database.dump.*manage.py"):
                PREREQUISITES.require_restore_artifacts(Path(temp))

    def test_restore_must_have_passed_before_additional_cases(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in ["connection.json", "runtime.json", "database.dump"]:
                (root / name).touch()
            result = root / "phase-c-results.json"
            for rows in [[], [dict(case="C03.clean-cluster-restore", status="FAIL")]]:
                result.write_text(json.dumps(rows), encoding="utf-8")
                with self.assertRaisesRegex(RuntimeError, "successful recovery restore"):
                    PREREQUISITES.require_restore_artifacts(root)
            result.write_text(json.dumps([dict(case="C03.clean-cluster-restore", status="PASS")]), encoding="utf-8")
            PREREQUISITES.require_restore_artifacts(root)

    def test_additional_phase_without_fixtures_names_the_runner_before_io(self):
        code = """
import pathlib, sys, tempfile, types
sys.path.insert(0, sys.argv[1])
sys.modules['psycopg'] = types.ModuleType('psycopg')
sys.modules['psycopg.types'] = types.ModuleType('psycopg.types')
module = types.ModuleType('psycopg.types.json')
module.Jsonb = object
sys.modules[module.__name__] = module
import additional_cases
with tempfile.TemporaryDirectory() as temp:
    additional_cases.ROOT = pathlib.Path(temp)
    try:
        additional_cases.main()
    except RuntimeError as error:
        if 'manage.py' not in str(error):
            raise
    else:
        raise RuntimeError('Missing phase fixtures accepted')
"""
        result = subprocess.run(
            [sys.executable, "-c", code, str(SOURCE)],
            capture_output=True, text=True, timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
