import io
import os
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from cleanup_errors import retain_cleanup_failure,print_failure

class CleanupErrorsTests(unittest.TestCase):
    def test_multiple_teardown_failures_preserve_primary_and_original_cause(self):
        primary=RuntimeError('test failed')
        original=ValueError('original cause')
        primary.__cause__=original
        for resource in ('Compose project','Podman machine','temporary directory'):
            cleanup=OSError(resource+' failed')
            cleanup.__context__=primary
            retain_cleanup_failure(primary,cleanup,resource)
        with patch('sys.stderr',new_callable=io.StringIO) as output:
            print_failure('ERR_TEST_EXECUTION_FAILED',primary)
        self.assertIs(primary.__cause__,original)
        self.assertEqual(output.getvalue().splitlines(),[
            'ERR_TEST_EXECUTION_FAILED: test failed','CAUSE: original cause',
            'CLEANUP FAILED (Compose project): Compose project failed',
            'CLEANUP FAILED (Podman machine): Podman machine failed',
            'CLEANUP FAILED (temporary directory): temporary directory failed'])

    def test_reporting_a_cyclic_cause_is_bounded(self):
        primary=RuntimeError('test failed');cleanup=OSError('cleanup failed')
        primary.__cause__=cleanup;cleanup.__cause__=primary
        retain_cleanup_failure(primary,cleanup,'fixture')
        with patch('sys.stderr',new_callable=io.StringIO) as output:
            print_failure('FAIL',primary)
        self.assertEqual(len(output.getvalue().splitlines()),2)

    def test_verifier_cli_reports_primary_and_each_cleanup_failure(self):
        root=Path(__file__).resolve().parents[1]
        code = """
import runpy, subprocess, sys
from cleanup_errors import retain_cleanup_failure
error=RuntimeError('primary')
retain_cleanup_failure(error,OSError('compose failure'),'Compose project')
retain_cleanup_failure(error,OSError('VM failure'),'Podman machine')
def fail(*args,**kwargs): raise error
subprocess.run=fail
sys.argv=['verify.py','--docs']
runpy.run_path('scripts/verify.py',run_name='__main__')
"""
        result=subprocess.run([sys.executable,'-c',code],cwd=root,
                              env=dict(os.environ,PYTHONPATH=str(root/'scripts')),
                              capture_output=True,text=True,timeout=10)
        self.assertEqual(result.returncode,3,result.stderr)
        for message in ('ERR_TEST_EXECUTION_FAILED: primary',
                        'CLEANUP FAILED (Compose project): compose failure',
                        'CLEANUP FAILED (Podman machine): VM failure'):
            self.assertIn(message,result.stderr)
