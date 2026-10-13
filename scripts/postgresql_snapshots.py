"""Production-codec/native JSONB bridge; called only after foundation preflight."""
import json
import os
import re
import signal
import subprocess
import tempfile
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RECORD_LIMIT = 1024 * 1024
READ_LIMIT = RECORD_LIMIT + 2 * 16384
FRAME_LIMIT = READ_LIMIT * 4 + 4096


def bounded(command, data=b'', *, limit=FRAME_LIMIT, timeout=60):
    """Bound both pipes while draining concurrently; own process through exit."""
    chunks = [bytearray(), bytearray()]
    overflow = threading.Event()
    with tempfile.TemporaryFile() as source:
        source.write(data)
        source.seek(0)
        options = ({'creationflags': subprocess.CREATE_NEW_PROCESS_GROUP | subprocess.CREATE_NO_WINDOW}
                   if os.name == 'nt' else {'start_new_session': True})
        with subprocess.Popen(command, cwd=ROOT, stdin=source,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, **options) as process:
            termination = threading.Lock()

            def terminate():
                with termination:
                    if process.poll() is not None:
                        return
                    if os.name == 'nt':
                        stopped = subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                            capture_output=True, timeout=5, creationflags=subprocess.CREATE_NO_WINDOW)
                        if stopped.returncode != 0 and process.poll() is None:
                            process.kill()
                    else:
                        try:
                            os.killpg(process.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass

            def drain(stream, index, cap):
                while True:
                    chunk = stream.read(min(65536, cap + 1 - len(chunks[index])))
                    if not chunk:
                        break
                    chunks[index].extend(chunk)
                    if len(chunks[index]) > cap:
                        overflow.set()
                        terminate()
                        break
            readers = [threading.Thread(target=drain, args=(process.stdout, 0, limit)),
                       threading.Thread(target=drain, args=(process.stderr, 1, 65536))]
            for reader in readers:
                reader.start()
            try:
                process.wait(timeout=timeout)
            finally:
                if process.poll() is None:
                    terminate()
                    process.wait(timeout=5)
                for reader in readers:
                    reader.join(timeout=5)
                if any(reader.is_alive() for reader in readers):
                    raise ValueError('snapshot subprocess pipe did not close')
            if overflow.is_set():
                raise ValueError('snapshot subprocess output overflow')
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

    def oracle(self, mode, label, frame=None, *, reject=False):
        data = b'' if frame is None else json.dumps(frame).encode('utf-8')
        require(len(data) <= FRAME_LIMIT, 'oracle input frame overflow')
        result = bounded([str(self.binary), mode, label], data)
        if reject:
            require(result.returncode == 3 and not result.stdout
                    and result.stderr.startswith(b'ERR_SNAPSHOT_ORACLE:'),
                    'expected production codec rejection absent')
        else:
            require(result.returncode == 0, 'snapshot oracle failed')
        return result.stdout

    def sql(self, statement, *, state=None):
        db = self.database
        result = bounded(db.podman + ['exec', '-i', db.container, 'env', 'LC_ALL=C',
            'psql', '-X', '-qAt', '-U', 'foundation_serving', '-d', 'postgres',
            '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=verbose'],
            ("SET statement_timeout='10s';\n" + statement).encode('utf-8'))
        if state:
            error = re.search(rb'^ERROR:[ \t]+([0-9A-Z]{5}):', result.stderr, re.MULTILINE)
            require(result.returncode == 3 and error and error.group(1).decode() == state,
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

    def relational(self, fixture, *, revision=None, row_revision=None, actor='actor', state=None):
        revision = fixture['revision'] if revision is None else revision
        row_revision = revision if row_revision is None else row_revision
        require(re.fullmatch(r'[0-9]{1,20}', revision) is not None, 'invalid fixture revision')
        require(re.fullmatch(r'[0-9]{1,20}', row_revision) is not None, 'invalid row revision')
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
          p->'value'->>'estimate_unit',{revision},{revision},'actor',:operation_row_id,'m1_v1'
        FROM (SELECT {sql_text(fixture['project'])}::jsonb AS p) AS encoded_project;
        INSERT INTO kehila.configuration_revisions VALUES ('snapshot-bridge',{row_revision},1,
          {sql_text(fixture['configuration'])}::jsonb,{sql_text(fixture['project'])}::jsonb,
          '{actor}',:operation_row_id,'project_create','project');
        SET CONSTRAINTS ALL IMMEDIATE;
        {projection('project_snapshot', 'configuration_snapshot', row="kehila.configuration_revisions WHERE project_id='snapshot-bridge'")}
        ROLLBACK;"""
        return self.sql(statement, state=state)

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
        self.relational(seed, revision='0', state='23514')
        self.relational(seed, row_revision='0', state='23514')
        self.relational(seed, actor='absent', state='23503')
        for expression in (
            "jsonb_set(p,'{value,id}','\"wrong\"')",
            "jsonb_set(p,'{value,configuration_revision}','\"2\"')",
            "jsonb_set(p,'{value,archived}','true')",
            "jsonb_set(p,'{codec_version}','2')",
        ):
            self.oracle('verify', 'seed', self.standalone(seed, project=expression), reject=True)
        for expression in (
            "jsonb_set(c,'{value,project_id}','\"wrong\"')",
            "jsonb_set(c,'{value,revision}','\"2\"')",
            "jsonb_set(c,'{value,project_archived}','true')",
            "jsonb_set(c,'{codec_version}','2')",
        ):
            self.oracle('verify', 'seed', self.standalone(seed, configuration=expression), reject=True)
        overflow = json.loads(self.oracle('fixture', 'overflow'))
        frame = self.standalone(overflow)
        require(len(frame['configuration']) // 2 == RECORD_LIMIT + 1,
                'JSONB expansion overflow fixture was not exact')
        require(len(overflow['configuration']) // 2 <= RECORD_LIMIT,
                'compact overflow fixture already exceeds codec ceiling')
        self.oracle('verify', 'overflow', frame, reject=True)
        nul = json.loads(self.oracle('fixture', 'nul'))
        self.sql('SELECT ' + sql_text(nul['configuration']) + '::jsonb;', state='22P05')
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
