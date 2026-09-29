"""The documented pure command must execute every pure Rust crate."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('verify', ROOT / 'scripts/verify.py')
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class PureMatrixTests(unittest.TestCase):
    def test_task_contract_failure_fails_the_pure_command(self):
        seen = []

        def run(command, **kwargs):
            seen.append(command)
            if command[0] == 'cargo' and 'task_contract' in command:
                raise subprocess.CalledProcessError(1, command)

        with patch.object(sys, 'argv', ['verify.py', '--pure']), patch.object(VERIFY.subprocess, 'run', run):
            with self.assertRaises(subprocess.CalledProcessError):
                VERIFY.main()
        self.assertTrue(any('task_contract' in command for command in seen))
