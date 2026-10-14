"""The live phase consumes prepared binaries without invoking a compiler."""
import importlib.util
import os
import time
from contextlib import nullcontext
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

integration = Path(__file__).resolve().parent / 'integration'
sys.path.insert(0, str(integration))
spec = importlib.util.spec_from_file_location('task_path_build', integration / 'task_path.py')
task_path = importlib.util.module_from_spec(spec)
spec.loader.exec_module(task_path)


class PreparationTests(unittest.TestCase):
    def test_default_mode_prepares_then_executes_inside_the_owned_directory(self):
        folder = Path('unused-fixture')
        order = []
        with patch.object(sys, 'argv', ['task_path.py']), \
                patch.object(task_path, 'owned_directory', return_value=nullcontext(folder)) as owned, \
                patch.object(task_path, 'prepare', side_effect=lambda path: order.append(('prepare', path))), \
                patch.object(task_path, 'exercise', side_effect=lambda path: order.append(('exercise', path))):
            task_path.main()
        owned.assert_called_once_with('kehila-task-path-')
        self.assertEqual(order, [('prepare', folder), ('exercise', folder)])

    def test_build_failure_prevents_copy_or_go_build(self):
        with patch.object(task_path.subprocess, 'run',
                          side_effect=subprocess.CalledProcessError(1, ['cargo'])) as run, \
                patch.object(task_path.shutil, 'copy2') as copy:
            with self.assertRaises(subprocess.CalledProcessError):
                task_path.prepare(Path('unused'))
        self.assertEqual(run.call_count, 1)
        copy.assert_not_called()

    def test_run_only_dispatch_does_not_build(self):
        with tempfile.TemporaryDirectory() as folder, \
                patch.object(sys, 'argv', ['task_path.py', '--run-only', folder]), \
                patch.object(task_path, 'prepare') as prepare, \
                patch.object(task_path, 'exercise') as exercise:
            task_path.main()
            prepare.assert_not_called()
            exercise.assert_called_once_with(Path(folder).resolve())

    def test_missing_binaries_fail_before_process_or_database_access(self):
        with tempfile.TemporaryDirectory() as folder, \
                patch.object(task_path.subprocess, 'run') as run, \
                patch.object(task_path.subprocess, 'Popen') as launch:
            with self.assertRaisesRegex(ValueError, 'Both task-path executables'):
                task_path.exercise(Path(folder))
            run.assert_not_called()
            launch.assert_not_called()

    def test_build_only_dispatch_does_not_run_live_cases(self):
        with tempfile.TemporaryDirectory() as folder, \
                patch.object(sys, 'argv', ['task_path.py', '--build-only', folder]), \
                patch.object(task_path, 'prepare') as prepare, \
                patch.object(task_path, 'exercise') as exercise:
            task_path.main()
            prepare.assert_called_once_with(Path(folder).resolve())
            exercise.assert_not_called()

    def test_expired_child_budget_stops_launched_worker_and_runs_collection_cleanup(self):
        import verification_budget as budget
        processes=[]
        native_launch=subprocess.Popen
        native_bound=task_path.bound_base
        def launch(command,**kwargs):
            process=native_launch([sys.executable,'-c','import time; time.sleep(30)'],**kwargs)
            processes.append(process)
            self.addCleanup(task_path.stop,process)
            return process
        def expire(log,process):
            os.environ['KEHILA_VERIFICATION_DEADLINE']=str(time.monotonic()-1)
            return native_bound(log,process)
        with tempfile.TemporaryDirectory() as directory:
            folder=Path(directory)
            extension='.exe' if os.name=='nt' else ''
            for binary in ('task_worker','task-api'): (folder/(binary+extension)).touch()
            with patch.dict(os.environ,KEHILA_VERIFICATION_DEADLINE=str(time.monotonic()+10)), \
                    patch.object(task_path.subprocess,'Popen',side_effect=launch), \
                    patch.object(task_path.subprocess,'run') as run, \
                    patch.object(task_path,'bound_base',side_effect=expire):
                with self.assertRaises(budget.DeadlineExceeded): task_path.exercise(folder)
            self.assertEqual(len(processes),1)
            self.assertIsNotNone(processes[0].poll())
            self.assertEqual(run.call_args.args[0][-1],'drop-test-collection')
            self.assertEqual(run.call_args.kwargs['timeout'],15)
