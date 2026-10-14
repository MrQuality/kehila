"""Production-codec/native JSONB bridge; called only after foundation preflight."""
import json
import os
import re
import signal
import subprocess
import sys
import time
import tempfile
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RECORD_LIMIT = 1024 * 1024
READ_LIMIT = RECORD_LIMIT + 2 * 16384
FRAME_LIMIT = READ_LIMIT * 4 + 4096


def windows_child(command):
    """Own the command and its descendants before executing any command code.

    The supervisor's job handle is deliberately kept until process teardown.
    Exiting or killing the supervisor closes it and kills remaining descendants.
    No descendant can race job assignment: only this supervisor exists until then.
    """
    import ctypes
    from ctypes import wintypes

    class BasicLimits(ctypes.Structure):
        _fields_ = [('process_time', ctypes.c_int64), ('job_time', ctypes.c_int64),
                    ('flags', wintypes.DWORD), ('minimum', ctypes.c_size_t),
                    ('maximum', ctypes.c_size_t), ('active', wintypes.DWORD),
                    ('affinity', ctypes.c_size_t), ('priority', wintypes.DWORD),
                    ('scheduling', wintypes.DWORD)]

    class ExtendedLimits(ctypes.Structure):
        _fields_ = [('basic', BasicLimits), ('io', ctypes.c_uint64 * 6),
                    ('process_memory', ctypes.c_size_t), ('job_memory', ctypes.c_size_t),
                    ('peak_process', ctypes.c_size_t), ('peak_job', ctypes.c_size_t)]

    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
    kernel.CreateJobObjectW.restype = wintypes.HANDLE
    kernel.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int,
                                               ctypes.c_void_p, wintypes.DWORD]
    kernel.SetInformationJobObject.restype = wintypes.BOOL
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    kernel.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    kernel.AssignProcessToJobObject.restype = wintypes.BOOL
    job = kernel.CreateJobObjectW(None, None)
    limits = ExtendedLimits()
    limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE; no breakaway.
    if not (job and kernel.SetInformationJobObject(job, 9, ctypes.byref(limits),
                                                   ctypes.sizeof(limits))
            and kernel.AssignProcessToJobObject(job, kernel.GetCurrentProcess())):
        raise OSError('cannot establish snapshot process ownership')
    return subprocess.call(command, stdin=sys.stdin.buffer, stdout=sys.stdout.buffer,
                           stderr=sys.stderr.buffer, creationflags=subprocess.CREATE_NO_WINDOW)


def bounded(command, data=b'', *, limit=FRAME_LIMIT, timeout=60):
    """One deadline covers parent exit and both pipes; cleanup owns descendants."""
    chunks = [bytearray(), bytearray()]
    changed = threading.Event()
    failures = []
    overflow = threading.Event()
    with tempfile.TemporaryFile() as source:
        source.write(data)
        source.seek(0)
        options = ({'creationflags': subprocess.CREATE_NO_WINDOW}
                   if os.name == 'nt' else {'start_new_session': True})
        launched = ([sys.executable, str(Path(__file__).resolve()),
                     '--owned-windows-child', json.dumps(command)]
                    if os.name == 'nt' else command)
        process = subprocess.Popen(launched, cwd=ROOT, stdin=source, bufsize=0,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, **options)

        def drain(stream, index, cap):
            try:
                while True:
                    chunk = stream.read(min(65536, cap + 1 - len(chunks[index])))
                    if not chunk:
                        break
                    chunks[index].extend(chunk)
                    if len(chunks[index]) > cap:
                        overflow.set()
                        break
            except Exception as error:
                failures.append(error)
            finally:
                changed.set()

        readers = [threading.Thread(target=drain, args=(process.stdout, 0, limit), daemon=True),
                   threading.Thread(target=drain, args=(process.stderr, 1, 65536), daemon=True)]
        failure = None
        deadline = time.monotonic() + timeout
        try:
            for reader in readers:
                reader.start()
            while process.poll() is None or any(reader.is_alive() for reader in readers):
                if overflow.is_set():
                    raise ValueError('snapshot subprocess output overflow')
                if failures:
                    raise ValueError('snapshot subprocess pipe read failed') from failures[0]
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise subprocess.TimeoutExpired(command, timeout)
                changed.wait(min(remaining, 0.02))
                changed.clear()
            if overflow.is_set():
                raise ValueError('snapshot subprocess output overflow')
            if failures:
                raise ValueError('snapshot subprocess pipe read failed') from failures[0]
        except BaseException as error:
            failure = error
        finally:
            cleanup_error = None
            try:
                if os.name == 'nt':
                    if process.poll() is None:
                        process.kill()  # Closing the supervisor's job kills its tree.
                else:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)  # Also after parent exit.
                    except ProcessLookupError:
                        pass
                process.wait(timeout=2)
            except (OSError, subprocess.SubprocessError) as error:
                cleanup_error = error
            cleanup_deadline = time.monotonic() + 2
            for reader in readers:
                if reader.ident is not None:
                    reader.join(timeout=max(0, cleanup_deadline - time.monotonic()))
            if any(reader.is_alive() for reader in readers):
                cleanup_error = ValueError('snapshot subprocess pipe did not close')
            for stream in (process.stdout, process.stderr):
                try:
                    stream.close()
                except OSError as error:
                    cleanup_error = error
            if failure is not None:
                raise failure from cleanup_error
            if cleanup_error is not None:
                raise cleanup_error
        return subprocess.CompletedProcess(command, process.returncode,
                                           bytes(chunks[0]), bytes(chunks[1]))


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sql_text(hex_value):
    require(isinstance(hex_value, str) and len(hex_value) <= READ_LIMIT * 2
            and re.fullmatch(r'(?:[0-9a-f]{2})*', hex_value) is not None,
            'invalid snapshot hex fixture')
    return "convert_from(decode('" + hex_value + "','hex'),'UTF8')"


def projection(project, configuration, *, row=None):
    # SQL text is generated solely from fixed expressions/validated hex, not raw JSON.
    identity = 'project_id' if row else "'snapshot-bridge'"
    revision = 'revision::text' if row else "(p->'value'->>'configuration_revision')"
    version = 'snapshot_codec_version' if row else '1'
    return f"""SELECT json_build_object(
      'project', CASE WHEN octet_length(({project})::text) <= {READ_LIMIT}
        THEN encode(convert_to(({project})::text,'UTF8'),'hex') ELSE NULL END,
      'configuration', CASE WHEN octet_length(({configuration})::text) <= {READ_LIMIT}
        THEN encode(convert_to(({configuration})::text,'UTF8'),'hex') ELSE NULL END,
      'project_id', {identity}, 'revision', {revision}, 'codec_version', {version})
      FROM {row or 'snapshots'};"""


class SnapshotCheck:
    def __init__(self, database, binary):
        self.database, self.binary = database, binary

    def oracle(self, mode, label, frame=None, *, reject=None):
        data = b'' if frame is None else json.dumps(frame).encode('utf-8')
        require(len(data) <= FRAME_LIMIT, 'oracle input frame overflow')
        command_mode = f"reject_{reject}" if reject else mode
        result = bounded([str(self.binary), command_mode, label], data)
        if reject:
            require(result.returncode == 0 and not result.stderr
                    and result.stdout == f"PASS: expected production codec rejection: {command_mode}\n".encode(),
                    'expected production codec rejection absent')
        else:
            require(result.returncode == 0, 'snapshot oracle failed')
        return result.stdout

    def sql(self, statement, *, state=None, message=None):
        require(message is None or state is not None, 'expected message requires SQLSTATE')
        db = self.database
        result = bounded(db.podman + ['exec', '-i', db.container, 'env', 'LC_ALL=C',
            'psql', '-X', '-qAt', '-U', 'foundation_serving', '-d', 'postgres',
            '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=verbose'],
            ("SET statement_timeout='10s';\n" + statement).encode('utf-8'))
        if state:
            error = re.search(rb'^ERROR:[ \t]+([0-9A-Z]{5}):[^\S\r\n]*(.*)$', result.stderr, re.MULTILINE)
            require(result.returncode == 3 and error and error.group(1).decode() == state
                    and (message is None or message.encode() in error.group(2)),
                    'expected snapshot SQL rejection absent')
            return None
        require(result.returncode == 0, 'snapshot SQL execution failed')
        frame = json.loads(result.stdout)
        require(isinstance(frame, dict) and set(frame) == {
            'project', 'configuration', 'project_id', 'revision', 'codec_version'},
            'invalid snapshot SQL row frame')
        return frame

    def standalone(self, fixture, *, project='p', configuration='c'):
        return self.sql('WITH snapshots AS (SELECT ' + sql_text(fixture['project']) +
            '::jsonb AS p,' + sql_text(fixture['configuration']) + '::jsonb AS c)\n' +
            projection(project, configuration))

    def relational(self, fixture, *, revision=None, row_revision=None, next_sequence=None,
                   stored_version=1, actor='actor', state=None, message=None):
        revision = fixture['revision'] if revision is None else revision
        row_revision = revision if row_revision is None else row_revision
        next_sequence = fixture['revision'] if next_sequence is None else next_sequence
        require(re.fullmatch(r'[0-9]{1,20}', revision) is not None, 'invalid fixture revision')
        require(re.fullmatch(r'[0-9]{1,20}', row_revision) is not None, 'invalid row revision')
        require(isinstance(next_sequence, str) and re.fullmatch(r'[0-9]{1,20}', next_sequence) is not None,
                'invalid fixture sequence')
        require(type(stored_version) is int and stored_version in {1, 2}, 'invalid stored version')
        require(actor in {'actor', 'absent'}, 'invalid fixture actor')
        # Structural parent/attribution fixtures, not a complete application adapter.
        statement = f"""BEGIN;
        INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_project_id,
          token,request_codec_version,request_sha256,required_grant,grant_policy_version,
          replay_origin_ms,replay_deadline_ms)
        VALUES ('actor','project_create','project','snapshot-bridge','snapshot',1,
          decode(repeat('01',32),'hex'),'project_create',1,200,7776000200)
        RETURNING operation_row_id \\gset
        INSERT INTO kehila.operation_payloads VALUES (:operation_row_id,'\\x01',1,'{{}}');
        INSERT INTO kehila.projects(project_id,name,current_prefix,estimate_unit,
          configuration_revision,next_sequence,created_by,creation_operation_row_id,seed_profile)
        SELECT p->'value'->>'id',p->'value'->>'name',p->'value'->>'prefix',
          p->'value'->>'estimate_unit',{revision},{next_sequence},'actor',:operation_row_id,'m1_v1'
        FROM (SELECT {sql_text(fixture['project'])}::jsonb AS p) AS encoded_project;
        INSERT INTO kehila.configuration_revisions VALUES ('snapshot-bridge',{row_revision},{stored_version},
          {sql_text(fixture['configuration'])}::jsonb,{sql_text(fixture['project'])}::jsonb,
          '{actor}',:operation_row_id,'project_create','project');
        SET CONSTRAINTS ALL IMMEDIATE;
        {projection('project_snapshot', 'configuration_snapshot', row="kehila.configuration_revisions WHERE project_id='snapshot-bridge'")}
        ROLLBACK;"""
        return self.sql(statement, state=state, message=message)

    def run(self):
        for label in ('seed', 'ordered', 'escaped', 'zero', 'wide', 'maximum', 'limit'):
            fixture = json.loads(self.oracle('fixture', label))
            frame = self.standalone(fixture)
            self.oracle('verify', label, frame)
            if label in {'seed', 'wide', 'maximum'}:
                self.oracle('verify', label, self.relational(fixture))
            if label == 'limit':
                require(len(frame['configuration']) // 2 == RECORD_LIMIT,
                        'native fixture did not reach exact JSONB read ceiling')
        seed = json.loads(self.oracle('fixture', 'seed'))
        self.relational(seed, revision='0', state='23514', message='projects_configuration_revision_check')
        self.relational(seed, next_sequence='0', state='23514', message='projects_next_sequence_check')
        self.relational(seed, row_revision='0', state='23514', message='configuration_revisions_revision_check')
        self.relational(seed, actor='absent', state='23503',
                        message='on table "configuration_revisions" violates foreign key constraint')
        self.oracle('verify', 'seed', self.relational(seed, revision='2'), reject='revision')
        self.oracle('verify', 'seed', self.relational(seed, stored_version=2), reject='version')
        for expression, rejection in (
            ("jsonb_set(p,'{value,id}','\"wrong\"')", 'identity'),
            ("jsonb_set(p,'{value,configuration_revision}','\"2\"')", 'revision'),
            ("jsonb_set(p,'{value,archived}','true')", 'archival'),
            ("jsonb_set(p,'{codec_version}','2')", 'envelope'),
        ):
            self.oracle('verify', 'seed', self.standalone(seed, project=expression), reject=rejection)
        for expression, rejection in (
            ("jsonb_set(c,'{value,project_id}','\"wrong\"')", 'identity'),
            ("jsonb_set(c,'{value,revision}','\"2\"')", 'revision'),
            ("jsonb_set(c,'{value,project_archived}','true')", 'archival'),
            ("jsonb_set(c,'{codec_version}','2')", 'envelope'),
        ):
            self.oracle('verify', 'seed', self.standalone(seed, configuration=expression), reject=rejection)
        overflow = json.loads(self.oracle('fixture', 'overflow'))
        frame = self.standalone(overflow)
        require(len(frame['configuration']) // 2 == RECORD_LIMIT + 1,
                'JSONB expansion overflow fixture was not exact')
        require(len(overflow['configuration']) // 2 <= RECORD_LIMIT,
                'compact overflow fixture already exceeds codec ceiling')
        self.oracle('verify', 'overflow', frame, reject='limit')
        nul = json.loads(self.oracle('fixture', 'nul'))
        self.sql('SELECT ' + sql_text(nul['configuration']) + '::jsonb;', state='22P05',
                 message='unsupported Unicode escape sequence')
        print('PASS: production snapshot codecs, JSONB exact-limit/overflow/NUL, '
              'ordered reconstruction, attributed revision rows and incoherence rejection')


def run(database):
    target = ROOT / 'target'
    target.mkdir(exist_ok=True)
    require(target.resolve().is_relative_to(ROOT.resolve()),
            'native temporary build target must stay inside the checkout')
    with tempfile.TemporaryDirectory(prefix='native-snapshot-', dir=target) as temporary:
        build = bounded(['cargo', 'build', '--locked', '-p', 'project_snapshot_check',
                         '--target-dir', temporary, '-j', '1'], limit=1024 * 1024, timeout=180)
        require(build.returncode == 0, 'production snapshot helper build failed')
        binary = Path(temporary) / 'debug' / ('project_snapshot_check.exe'
                 if os.name == 'nt' else 'project_snapshot_check')
        require(binary.is_file(), 'production snapshot helper artifact missing')
        SnapshotCheck(database, binary).run()


if __name__ == '__main__':
    try:
        if os.name != 'nt' or len(sys.argv) != 3 or sys.argv[1] != '--owned-windows-child':
            raise ValueError('internal snapshot supervisor invocation required')
        sys.exit(windows_child(json.loads(sys.argv[2])))
    except (OSError, ValueError, subprocess.SubprocessError):
        print('ERR_SNAPSHOT_PROCESS: owned command could not execute', file=sys.stderr)
        sys.exit(3)
