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

    def test_nested_service_machine_and_directory_failures_are_all_retained(self):
        from contextlib import nullcontext
        import shutil
        import temp_work
        import test_services as services
        primary=RuntimeError('test failed')
        compose=OSError('compose failed')
        vm=OSError('VM failed')
        directory=OSError('directory failed')
        remove=shutil.rmtree
        def run(command,**kwargs):
            if 'down' in command: raise compose
        with patch.object(services,'_machine_lease',return_value=nullcontext()), \
                patch.object(services,'_machine_info',return_value={'State':'stopped'}), \
                patch.object(services,'_machine_connections',return_value=['fixture','fixture-root']), \
                patch.object(services,'compose_command',return_value=['podman','compose']), \
                patch.object(services,'run',side_effect=run), \
                patch.object(services,'_containers',return_value=['container']), \
                patch.object(services,'_stop_owned_machine',side_effect=vm), \
                patch.object(temp_work.shutil,'rmtree',side_effect=directory):
            with self.assertRaises(RuntimeError) as caught:
                with temp_work.owned_directory('kehila-error-test-') as folder:
                    self.addCleanup(remove,folder)
                    with services.managed_runtime('podman','fixture',environment={},exclusive=True) as runtime:
                        with services.service_group(runtime,['nats']): raise primary
        self.assertIs(caught.exception,primary)
        self.assertEqual(primary._cleanup_failures,(
            ('Compose project',compose),('Podman machine',vm),('temporary directory',directory)))

    def test_child_entrypoint_reports_cleanup_failures_and_interruptions(self):
        import cleanup_errors
        for error,code in ((RuntimeError('leaf failed'),3),(KeyboardInterrupt(),130)):
            cleanup=OSError('child directory failed')
            retain_cleanup_failure(error,cleanup,'temporary directory')
            def fail(): raise error
            with self.subTest(code=code), patch('sys.stderr',new_callable=io.StringIO) as output:
                self.assertEqual(cleanup_errors.run_main(fail,'ERR_CHILD_CHECK'),code)
            self.assertIn('CLEANUP FAILED (temporary directory): child directory failed',output.getvalue())

    def test_child_entrypoint_preserves_success_and_explicit_exit_codes(self):
        import cleanup_errors
        self.assertIsNone(cleanup_errors.run_main(lambda:None,'ERR_CHILD_CHECK'))
        self.assertEqual(cleanup_errors.run_main(lambda:7,'ERR_CHILD_CHECK'),7)
        def invalid(): raise SystemExit(2)
        with self.assertRaises(SystemExit) as caught: cleanup_errors.run_main(invalid,'ERR_CHILD_CHECK')
        self.assertEqual(caught.exception.code,2)
