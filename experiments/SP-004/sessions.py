"""Owned psql participants for the opt-in experiment; no database driver dependency."""
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from cleanup_errors import CleanupStack, retain_cleanup_failure
from temp_work import owned_directory


class SqlFailure(ValueError):
    def __init__(self, state, primary):
        self.state, self.primary = state, primary
        super().__init__(state + ': ' + primary)


class BarrierNotReached(ValueError):
    def __init__(self, holder, waiter):
        self.holder, self.waiter = holder, waiter
        super().__init__('expected actor-fence wait was not reached')


def frame(data, marker):
    if len(data) > 65536:
        raise ValueError('participant output exceeds 64 KiB')
    text = data.decode('utf-8')
    error = re.search(r'^(?:psql:[^\r\n]+:\d+:\s*)?ERROR:[ \t]+([0-9A-Z]{5}):[ \t]*(.*)$',
                      text, re.MULTILINE)
    if error:
        raise SqlFailure(error[1], error[2][:2048])
    lines = text.splitlines(keepends=True)
    for index, line in enumerate(lines):
        if line.endswith('\n') and line.rstrip('\r\n') == marker:
            return ''.join(lines[:index]).strip()
    return None


class Session:
    """At most one in-flight command, 60s lifetime, explicit backend and process cleanup.

    Only constant experiment SQL is admitted. Results are read through an
    independent file handle so inspecting output cannot move the child's cursor.
    Native backend termination also rolls back a pending server-side transaction
    if a local Podman client is interrupted. Cleanup never targets another session.
    """

    def __init__(self, database, user):
        self.database, self.user = database, user
        self.name = 'sp004-' + uuid.uuid4().hex
        self.pending = None
        self.offset = 0
        self.serial = 0
        self.stack = CleanupStack()
        self.expired = threading.Event()
        self.watchdog = None
        self.watchdog_error = None

    def __enter__(self):
        self.stack.__enter__()
        try:
            folder = self.stack.enter_context(owned_directory('sp004-session-'), resource='session files')
            self.path = folder / 'output.log'
            output = self.path.open('wb')
            self.stack.callback(output.close, resource='session output')
            command = [*self.database.podman, 'exec', '-i', self.database.container,
                       'env', 'LC_ALL=C', 'PGAPPNAME=' + self.name,
                       'psql', '-X', '-qAt', '-U', self.user, '-d', 'postgres',
                       '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=verbose']
            if os.name == 'nt':
                command = [sys.executable, str(ROOT / 'scripts/postgresql_snapshots.py'),
                           '--owned-windows-child', json.dumps(command)]
            options = {'creationflags': subprocess.CREATE_NO_WINDOW} if os.name == 'nt' else {'start_new_session': True}
            self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=output,
                                            stderr=output, **options)
            self.stack.callback(self.stop, resource='native participant')
            self.deadline = time.monotonic() + 60
            self.watchdog = threading.Timer(60, self.expire)
            self.watchdog.start()
            self.pid = int(self.execute("SET statement_timeout='20s'; SET lock_timeout='10s'; "
                "SET idle_in_transaction_session_timeout='30s'; SELECT pg_backend_pid();"))
            return self
        except BaseException:
            self.__exit__(*sys.exc_info())
            raise

    def __exit__(self, *failure):
        # Join the supervisor before choosing the outcome: its callback may
        # already be running when cancel() is called. Always finish teardown.
        supervisor_error = None
        try:
            self.stop_watchdog()
        except BaseException as error:
            supervisor_error = error
        kind, error, traceback = failure
        if error is None and self.expired.is_set():
            error = TimeoutError('native participant exceeded 60 seconds')
            kind, traceback = type(error), None
        if supervisor_error is not None:
            if error is None:
                error = supervisor_error
                kind, traceback = type(error), error.__traceback__
            else:
                retain_cleanup_failure(error, supervisor_error, 'participant deadline')
        self.stack.__exit__(kind, error, traceback)
        if failure[1] is None and error is not None:
            raise error
        return False

    def expire(self):
        self.expired.set()
        try:
            self.kill_tree()
        except BaseException as error:
            self.watchdog_error = error

    def stop_watchdog(self):
        if self.watchdog is None:
            return
        self.watchdog.cancel()
        if self.watchdog.ident is not None:
            self.watchdog.join(timeout=2)
        if self.watchdog.is_alive():
            raise ValueError('participant deadline supervisor did not stop')
        if self.watchdog_error is not None:
            raise self.watchdog_error

    def send(self, sql):
        if self.pending is not None:
            raise ValueError('participant already has an in-flight command')
        marker = 'sp004_done_' + str(self.serial + 1)
        data = (sql + '\n\\echo ' + marker + '\n').encode('utf-8')
        if len(data) > 4096:
            raise ValueError('participant command exceeds 4 KiB')
        self.serial += 1
        self.pending = marker
        self.process.stdin.write(data)
        self.process.stdin.flush()

    def poll(self):
        if self.pending is None:
            raise ValueError('participant has no in-flight command')
        if self.expired.is_set() or time.monotonic() >= self.deadline:
            raise TimeoutError('native participant exceeded 60 seconds')
        if self.path.stat().st_size > 65536:
            raise ValueError('participant output exceeds 64 KiB')
        with self.path.open('rb') as stream:
            stream.seek(self.offset)
            data = stream.read(65537)
        result = frame(data, self.pending)
        if result is not None:
            self.offset += len(data)
            self.pending = None
            return result
        if self.process.poll() is not None:
            raise ValueError('native participant exited without result: ' + data.decode('utf-8')[:2048])
        return None

    def wait(self):
        while True:
            result = self.poll()
            if result is not None:
                return result
            time.sleep(0.01)

    def execute(self, sql):
        self.send(sql)
        return self.wait()

    def stop(self):
        # Names are generated above, never accepted from caller input.
        with CleanupStack() as cleanup:
            cleanup.callback(self.stop_process, resource='participant process tree')
            self.database.sql("SELECT pg_terminate_backend(pid,3000) FROM pg_stat_activity "
                              "WHERE application_name='" + self.name + "';")

        remaining = self.database.sql("SELECT count(*) FROM pg_stat_activity WHERE application_name='" + self.name + "';")
        if not re.search(r'^\s*0\s*$', remaining, re.MULTILINE):
            raise ValueError('native participant backend remains after cleanup')

    def stop_process(self):
        with CleanupStack() as cleanup:
            cleanup.callback(self.process.wait, timeout=5, resource='participant reap')
            cleanup.callback(self.process.stdin.close, resource='participant input')
            self.kill_tree()

    def kill_tree(self):
        if os.name == 'nt':
            if self.process.poll() is None:
                self.process.kill()  # Existing supervisor owns the local process tree.
        else:
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass


def blocked(observer, waiter, holder):
    """Observe the real blocker before releasing a controlled transaction."""
    deadline = min(waiter.deadline, holder.deadline, time.monotonic() + 5)
    while time.monotonic() < deadline:
        value = observer.execute('SELECT ' + str(holder.pid) + '=ANY(pg_blocking_pids(' + str(waiter.pid) + '));')
        if value == 't':
            return {'holder': holder.pid, 'waiter': waiter.pid}
        if waiter.poll() is not None:
            raise BarrierNotReached(holder.pid, waiter.pid)
        time.sleep(0.01)
    raise TimeoutError('expected actor-fence blocker was not observed')
