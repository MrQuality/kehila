"""Finite structural races through two separate container-local psql logins."""
import asyncio
from contextlib import AsyncExitStack
import json
from pathlib import Path
import re
import time
import uuid

from check_postgresql import require
from verification_settings import load
from cleanup_errors import retain_cleanup_failure


class Session:
    def __init__(self, database, cleanup=None):
        self.database = database
        self.process = None
        self.pending = None
        self.pid = None
        self.backend_start = None
        self.application = 'sp005-' + uuid.uuid4().hex
        self.cleanup_seconds = load()['runner_stop_seconds']
        self.cleanup = cleanup if cleanup is not None else {}
        self.output_bytes = 0
        self.output_overflow = False

    def account(self, raw):
        self.output_bytes += len(raw)
        self.output_overflow |= self.output_bytes > 1024**2

    def remaining(self):
        remaining = self.database.deadline - time.monotonic()
        require(remaining > 0, 'SP-005 concurrency work budget expired')
        return remaining

    async def __aenter__(self):
        self.process = await asyncio.create_subprocess_exec(
            *self.database.podman, 'exec', '-i', self.database.container,
            # Order diagnostics before completion markers inside the container.
            # Merging the local client's streams cannot order Podman's separate
            # remote exec channels. Shell text is fixed; values remain argv.
            'env', 'LC_ALL=C', 'sh', '-c', 'exec "$@" 2>&1', 'sp005-psql',
            'psql', '-X', '-q', '-A', '-t', '-U', 'foundation_serving',
            '-d', 'postgres', '-v', 'ON_ERROR_STOP=0', '-v', 'VERBOSITY=verbose',
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT, limit=4096)
        try:
            milliseconds = max(1, int(self.remaining() * 1000))
            await self.query(f"SET application_name='{self.application}'; "
                f"SET statement_timeout={milliseconds}; SET lock_timeout={milliseconds}; "
                f"SET idle_in_transaction_session_timeout={milliseconds};")
            result = await self.query('SELECT json_build_array(pid,backend_start::text) FROM pg_stat_activity WHERE pid=pg_backend_pid();')
            require(len(result) == 1, 'SP-005 backend identity absent')
            identity = json.loads(result[0])
            require(isinstance(identity, list) and len(identity) == 2
                    and type(identity[0]) is int and identity[0] > 0
                    and isinstance(identity[1], str)
                    and re.fullmatch(r'[0-9:+. -]+', identity[1]), 'SP-005 backend identity invalid')
            self.pid, self.backend_start = identity
            return self
        except BaseException as error:
            await self.__aexit__(type(error), error, error.__traceback__)
            raise

    async def send(self, sql):
        require(self.pending is None, 'SP-005 command already in flight')
        self.pending = 'SP005_DONE_' + uuid.uuid4().hex
        self.process.stdin.write((sql + '\n\\echo ' + self.pending + '\n').encode('utf-8'))
        await asyncio.wait_for(self.process.stdin.drain(), self.remaining())

    async def finish(self, expected_error=None):
        require(self.pending is not None, 'SP-005 has no command in flight')
        marker, self.pending = self.pending, None
        lines = []
        while True:
            raw = await asyncio.wait_for(self.process.stdout.readline(), self.remaining())
            require(raw, 'SP-005 participant ended before command acknowledgement')
            self.account(raw)
            require(not self.output_overflow, 'SP-005 participant diagnostic limit exceeded')
            text = raw.decode('utf-8').rstrip('\r\n')
            if text == marker:
                break
            lines.append(text)
        errors = [re.fullmatch(r'(?:psql:[^\r\n]+:\d+:\s*)?ERROR:\s+([0-9A-Z]{5}):\s+(.*)', line) for line in lines]
        errors = [match for match in errors if match]
        if expected_error is None:
            require(not errors, 'SP-005 participant SQL failed')
        else:
            state, constraint = expected_error
            def metadata(label):
                return [match[1] for line in lines if (match := re.fullmatch(
                    re.escape(label) + r':\s+([^\s]+)\s*', line))]
            require(len(errors) == 1 and errors[0][1] == state
                    and metadata('CONSTRAINT NAME') == [constraint]
                    and metadata('SCHEMA NAME') == ['kehila']
                    and metadata('TABLE NAME') == ['projects'],
                    'SP-005 expected primary uniqueness rejection absent')
        return [line for line in lines if line]

    async def query(self, sql, expected_error=None):
        await self.send(sql)
        return await self.finish(expected_error)

    async def __aexit__(self, _kind, primary, _traceback):
        process = self.process
        if process is None:
            return
        failure = primary
        if not self.cleanup:
            now = time.monotonic()
            self.cleanup.update(soft=now + self.cleanup_seconds / 2,
                                hard=now + self.cleanup_seconds)

        async def until(deadline, work):
            return await asyncio.wait_for(work, max(0, deadline - time.monotonic()))

        async def discard():
            # Never accumulate remaining output, even after a protocol failure.
            while raw := await process.stdout.read(4096):
                self.account(raw)

        async def reap(deadline):
            await until(deadline, asyncio.gather(discard(), process.wait()))
        try:
            if process.returncode is None:
                process.stdin.write(b'\\q\n')
                await until(self.cleanup['soft'], process.stdin.drain())
                process.stdin.close()
            await reap(self.cleanup['soft'])
            require(process.returncode == 0, 'SP-005 participant did not exit normally')
            require(not self.output_overflow, 'SP-005 participant diagnostic limit exceeded')
        except BaseException as error:
            if failure is None:
                failure = error
            else:
                retain_cleanup_failure(failure, error, 'SP-005 participant')
        finally:
            try:
                if process.returncode is None:
                    process.kill()
                    await reap(self.cleanup['hard'])
            except BaseException as error:
                if failure is None:
                    failure = error
                else:
                    retain_cleanup_failure(failure, error, 'SP-005 participant termination')
        if primary is None and failure is not None:
            raise failure


def create(actor, target, token, handle):
    # These are fixed synthetic case values, never client-supplied SQL.
    require(all(re.fullmatch(r'[a-z0-9-]+', value) for value in (actor, target, token, handle)),
            'SP-005 case identifier is invalid')
    return f"SELECT sp005.fixture_create('{actor}','{target}','{token}','{handle}');"


def observe(database, sql):
    rows = database.sql('\\set QUIET on\n\\pset tuples_only on\n\\pset format unaligned\n' + sql).splitlines()
    return [line for line in rows if line not in {'SET', ''}]


async def blocked(database, waiter, holder):
    while True:
        waiter.remaining()
        rows = observe(database, f"SELECT {holder.pid}=ANY(pg_blocking_pids(a.pid)) "
            f"FROM pg_stat_activity a WHERE a.pid={waiter.pid} AND a.application_name='{waiter.application}' "
            f"AND a.backend_start::text='{waiter.backend_start}' "
            "AND a.usename='foundation_serving' AND a.datname=current_database() "
            f"AND EXISTS (SELECT FROM pg_stat_activity h WHERE h.pid={holder.pid} "
            f"AND h.application_name='{holder.application}' AND h.backend_start::text='{holder.backend_start}' "
            "AND h.usename='foundation_serving' AND h.datname=current_database());")
        require(len(rows) == 1, 'SP-005 participant identity changed')
        if rows == ['t']:
            return
        await asyncio.sleep(0)  # Yield; the database blocking relation is the oracle.


def rolled_back(database, session, actor, target):
    require(observe(database, f"SELECT state='idle' AND xact_start IS NULL FROM pg_stat_activity "
        f"WHERE pid={session.pid} AND application_name='{session.application}' AND backend_start::text='{session.backend_start}';") == ['t'],
        'SP-005 rollback did not release transaction ownership')
    require(observe(database, f"SELECT count(*) FROM kehila.operations WHERE actor_id='{actor}' "
        f"AND target_project_id='{target}';") == ['0'], 'SP-005 failed creation left a core')
    database.sql(f"BEGIN; SELECT * FROM kehila.lock_actor('{actor}'); ROLLBACK;", user='foundation_serving')


async def exercise(database):
    database.sql(Path(__file__).with_name('concurrent_create.sql').read_text(encoding='utf-8'))
    async with AsyncExitStack() as stack:
        cleanup = {}
        second = await stack.enter_async_context(Session(database, cleanup))
        # Release the lock holder first on exceptional exit, before waiting for
        # the blocked participant's psql process to consume its quit command.
        first = await stack.enter_async_context(Session(database, cleanup))
        await first.query('BEGIN;')
        await first.query(create('actor', 'sp005-independent-a', 'same-token', 'b' * 32))
        await second.query('BEGIN;')
        await second.query(create('sp005-other', 'sp005-independent-b', 'same-token', 'b' * 32))
        await second.query('COMMIT;')
        require(observe(database, "SELECT EXISTS (SELECT FROM sp005.creation_intents m JOIN kehila.projects p "
            "ON p.creation_operation_row_id=m.operation_row_id AND p.project_id=m.target_project_id "
            "WHERE m.actor_id='sp005-other' AND m.target_project_id='sp005-independent-b') "
            "AND NOT EXISTS (SELECT FROM kehila.projects WHERE project_id='sp005-independent-a');") == ['t'],
            'SP-005 second actor did not become independently visible before first release')
        require(observe(database, f"SELECT state='idle in transaction' FROM pg_stat_activity WHERE pid={first.pid};") == ['t'],
                'SP-005 first actor was not held while second committed')
        await first.query('COMMIT;')
        for suffix, commit in [('committed', True), ('rolled-back', False)]:
            target, handle = 'sp005-race-' + suffix, ('c' if commit else 'd') * 32
            await first.query('BEGIN;')
            await first.query(create('actor', target, 'race-token', handle))
            await second.query('BEGIN;')
            await second.send(create('sp005-other', target, 'race-token', handle))
            await blocked(database, second, first)
            await first.query('COMMIT;' if commit else 'ROLLBACK;')
            if commit:
                await second.finish(('23505', 'projects_pkey'))
                await second.query('ROLLBACK;')
                rolled_back(database, second, 'sp005-other', target)
                await second.query('BEGIN;')
                await second.query(create('sp005-other', target + '-retry', 'race-token', handle))
            else:
                await second.finish()
                rolled_back(database, first, 'actor', target)
            await second.query('COMMIT;')
    expected = [
        ['actor', 'sp005-independent-a', 'same-token', 'b' * 32],
        ['sp005-other', 'sp005-independent-b', 'same-token', 'b' * 32],
        ['actor', 'sp005-race-committed', 'race-token', 'c' * 32],
        ['sp005-other', 'sp005-race-committed-retry', 'race-token', 'c' * 32],
        ['sp005-other', 'sp005-race-rolled-back', 'race-token', 'd' * 32],
    ]
    rows = observe(database, "SELECT coalesce(json_agg(row_to_json(observed)),'[]') FROM "
        "(SELECT m.actor_id,m.target_project_id,o.token,m.intent_handle FROM sp005.creation_intents m "
        "JOIN kehila.operations o USING(operation_row_id) JOIN kehila.projects p ON "
        "p.creation_operation_row_id=o.operation_row_id AND p.created_by=m.actor_id AND p.project_id=m.target_project_id "
        "WHERE m.target_project_id LIKE 'sp005-%' ORDER BY m.target_project_id) observed;")
    require(len(rows) == 1, 'SP-005 final observation absent')
    actual = [[row[key] for key in ('actor_id','target_project_id','token','intent_handle')]
              for row in json.loads(rows[0])]
    require(sorted(actual) == sorted(expected), 'SP-005 concurrent creation attribution differs')
    for table, count in [('projects',5),('configuration_revisions',5),('project_grants',5),
            ('project_grant_events',5),('statuses',10),('workflows',5),('workflow_statuses',10),
            ('workflow_phase_changes',5),('work_item_types',5),('type_workflows',5),('fields',5)]:
        require(observe(database, f"SELECT count(*) FROM kehila.{table} WHERE project_id LIKE 'sp005-%';") == [str(count)],
                'SP-005 case relational cardinality differs: ' + table)
    for query in [
            "SELECT count(*) FROM kehila.operations WHERE target_project_id LIKE 'sp005-%';",
            "SELECT count(*) FROM kehila.operation_payloads p JOIN kehila.operations o USING(operation_row_id) WHERE o.target_project_id LIKE 'sp005-%';",
            "SELECT count(*) FROM sp005.creation_intents WHERE target_project_id LIKE 'sp005-%';"]:
        require(observe(database, query) == ['5'], 'SP-005 case success cardinality differs')
    require(observe(database, "SELECT count(*) FROM kehila.projects p LEFT JOIN sp005.creation_intents m ON "
        "m.operation_row_id=p.creation_operation_row_id AND m.target_project_id=p.project_id "
        "AND m.actor_id=p.created_by WHERE p.project_id LIKE 'sp005-%' AND m.operation_row_id IS NULL;") == ['0'],
        'SP-005 case project lacks its attributed mapping')
    require(observe(database, "SELECT count(*) FROM pg_stat_activity WHERE "
        "application_name LIKE 'sp005-%';") == ['0'], 'SP-005 participants remain connected')
    return ['independent-actor-progress', 'committed-target-collision-full-rollback-fresh-target',
            'rolled-back-target-competitor-success']
