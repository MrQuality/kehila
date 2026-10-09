"""SQL client output formatting only; native SP-003 cases verify storage."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[1] / "experiments/SP-003"
SPEC = importlib.util.spec_from_file_location("sp003_run", SOURCE / "run.py")
RUNNER = importlib.util.module_from_spec(SPEC)
with patch.object(sys, "path", [str(SOURCE), *sys.path]), patch.dict(sys.modules):
    SPEC.loader.exec_module(RUNNER)


class SqlOutputTests(unittest.TestCase):
    def test_single_cell_preserves_all_content_whitespace(self):
        for value in ("", " project", "project ", " project ", " ", "\n", "\tproject\n",
                      "\t \n", "\u00a0project\u00a0"):
            with self.subTest(value=value):
                completed = subprocess.CompletedProcess([], 0, value.encode("utf-8") + b"\n", b"")
                with patch.object(RUNNER.subprocess, "run", return_value=completed):
                    self.assertEqual(RUNNER.sql("unused", "SELECT value;"), value)

    def test_command_without_rows_has_empty_output(self):
        completed = subprocess.CompletedProcess([], 0, b"", b"")
        with patch.object(RUNNER.subprocess, "run", return_value=completed):
            self.assertEqual(RUNNER.sql("unused", "CREATE TABLE sample (value text);"), "")
