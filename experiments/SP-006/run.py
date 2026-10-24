#!/usr/bin/env python3
"""Opt-in, owned PostgreSQL/OpenSearch rebuild proof; synthetic data only."""
import argparse
from contextlib import ExitStack
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'experiments/SP-004'))
from check_postgresql import preflight
from cleanup_errors import run_main
from sessions import Session, blocked
from test_services import managed_runtime, service_group
from verification_budget import command_timeout, work_budget
from protocol import Rebuild, Search, document, literal, require


class Database:
    def __init__(self, runtime, container):
        self.podman, self.container, self.environment = runtime.engine, container, runtime.environment

    def sql(self, statement):
        require(len(statement.encode()) <= 32768, 'fixture SQL bound exceeded')
        command = [*self.podman, 'exec', '-i', self.container, 'psql', '-XqAt', '-U', 'postgres',
                   '-d', 'postgres', '-v', 'ON_ERROR_STOP=1']
        result = subprocess.run(command, input="SET statement_timeout='10s'; SET lock_timeout='5s'; " + statement,
                                capture_output=True, text=True, env=self.environment, timeout=command_timeout(20))
        require(result.returncode == 0, 'fixture SQL failed: ' + result.stderr[:2048])
        require(len(result.stdout.encode()) <= 65536, 'fixture SQL response too large')
        return result.stdout.strip()

    def reset(self):
        self.sql('DROP SCHEMA IF EXISTS sp006 CASCADE;' + (SOURCE / 'schema.sql').read_text())

    def put(self, key, value):
        return int(self.sql('SELECT sp006.put(' + literal(key) + ',' + literal(value) + ');'))


def setup(db, prefix):
    db.reset()
    search = Search(prefix)
    old = prefix + '_old'
    search.index(old)
    status, body = search.request('POST', '/_aliases', {'actions': [{'add': {'index': old, 'alias': search.alias}}]})
    require(status == 200 and body.get('acknowledged'), 'initial alias failed')
    db.sql('INSERT INTO sp006.targets VALUES (' + literal(old) + ",'active'); INSERT INTO sp006.control VALUES(true,"
           + literal(old) + ",NULL,'live',false);")
    db.put('a', 'aone'); db.put('b', 'bone')
    return search, old


def sequence_counterexample(db):
    db.reset()
    db.sql('CREATE TABLE sp006.sequence_example (id bigint GENERATED ALWAYS AS IDENTITY, value text);')
    with Session(db, 'postgres') as slow, Session(db, 'postgres') as fast, Session(db, 'postgres') as snapshot:
        slow.execute("BEGIN; INSERT INTO sp006.sequence_example(value) VALUES('slow');")
        fast.execute("INSERT INTO sp006.sequence_example(value) VALUES('fast');")
        head = int(snapshot.execute('BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; SELECT max(id) FROM sp006.sequence_example;'))
        require(head == 2, 'sequence schedule not reached')
        slow.execute('COMMIT;')
        missed = int(db.sql("SELECT count(*) FROM sp006.sequence_example WHERE value='slow' AND id<=" + str(head) + ';'))
        after = int(db.sql('SELECT count(*) FROM sp006.sequence_example WHERE id>' + str(head) + ';'))
        require(missed == 1 and after == 0, 'unsafe sequence counterexample absent')
        snapshot.execute('ROLLBACK;')
    return {'case': 'sequence_is_not_commit_frontier', 'passed': True}


def live_rebuild(db, prefix):
    search, old = setup(db, prefix)
    target = prefix + '_new'
    with ExitStack() as stack:
        coordinator, writer, observer, snapshot = [stack.enter_context(Session(db, 'postgres')) for _ in range(4)]
        rebuild = Rebuild(db, coordinator, search)
        rebuild.drain(old)
        writer.execute("BEGIN; SELECT sp006.put('a','atwo');")
        rebuild.begin_fence()
        blocked(observer, coordinator, writer)
        writer.execute('COMMIT;')
        rebuild.finish_fence()
        rebuild.register(target, snapshot)
        # The snapshot is already established; later changes must get durable delivery rows.
        db.put('b', 'btwo')
        event = int(db.sql("SELECT max(event_id) FROM sp006.events WHERE doc_id='b';"))
        require(rebuild.delivery(event, target) == 'applied', 'newer delivery not applied')
        rebuild.snapshot(target, snapshot)  # Older snapshot B must not overwrite B2.
        require(search.read(target, 'b')['_source']['value'] == 'btwo', 'snapshot regressed newer event')
        db.put('a', 'athree')
        event = int(db.sql("SELECT max(event_id) FROM sp006.events WHERE doc_id='a';"))
        require(rebuild.delivery(event, target, acknowledge=False) == 'applied', 'crash boundary not reached')
        require(rebuild.delivery(event, target) == 'duplicate', 'index-before-ack retry not idempotent')
        try:
            search.publish(target, document('a', 3, 'corrupted'))
        except ValueError as error:
            require('same-version content mismatch' in str(error), 'wrong conflict failure')
        else:
            raise ValueError('same-version corruption accepted')
        # A high revision hides stale B under the rejected MAX-per-project test.
        db.put('b', 'bthree')
        source_max = int(db.sql('SELECT max(revision) FROM sp006.documents;'))
        indexed_max = max(search.read(target, key)['_version'] for key in ('a','b'))
        require(source_max == indexed_max == 3 and search.read(target, 'b')['_version'] == 2,
                'unsafe maximum counterexample absent')
        rebuild.fence()
        rebuild.cutover(target)
        require(search.alias_target() == target, 'cutover did not move alias')
        require(int(db.sql('SELECT count(*) FROM sp006.deliveries WHERE NOT acknowledged AND target=' + literal(target) + ';')) == 0,
                'cutover left target delivery gaps')
        status, result = search.request('GET', '/' + search.alias + '/_search?size=8')
        require(status == 200 and result['hits']['total']['value'] == 2, 'refreshed alias is not searchable')
    return {'case': 'fenced_snapshot_live_delivery_cutover', 'passed': True,
            'writer_block_observed': True, 'max_revision_counterexample': True,
            'duplicate_after_missing_ack': True, 'same_version_corruption_rejected': True}


def alias_recovery(db, prefix):
    search, old = setup(db, prefix)
    target = prefix + '_new'
    with Session(db, 'postgres') as coordinator, Session(db, 'postgres') as snapshot:
        rebuild = Rebuild(db, coordinator, search)
        rebuild.fence(); rebuild.register(target, snapshot); rebuild.snapshot(target, snapshot)
        rebuild.fence()
        try:
            rebuild.cutover(target, crash_after_alias=True)
        except InterruptedError:
            pass
        else:
            raise ValueError('alias interruption not reached')
    require(search.alias_target() == target and db.sql('SELECT phase FROM sp006.control;') == 'switching',
            'ambiguous durable alias boundary absent')
    db.put('a', 'aftercrash')
    require(int(db.sql('SELECT count(*) FROM sp006.deliveries WHERE event_id=(SELECT max(event_id) FROM sp006.events);')) == 2,
            'switching crash lost a delivery target')
    with Session(db, 'postgres') as coordinator:
        rebuild = Rebuild(db, coordinator, search)
        rebuild.fence(); rebuild.cutover(target)
        require(search.alias_target() == target and db.sql('SELECT phase FROM sp006.control;') == 'live',
                'alias recovery failed')
    return {'case': 'coordinator_loss_after_alias_before_metadata', 'passed': True}


def snapshot_loss(db, prefix):
    search, old = setup(db, prefix)
    target = prefix + '_lost'
    with ExitStack() as stack:
        coordinator = stack.enter_context(Session(db, 'postgres'))
        snapshot = stack.enter_context(Session(db, 'postgres'))
        rebuild = Rebuild(db, coordinator, search)
        rebuild.fence(); rebuild.register(target, snapshot)
        db.sql('SELECT pg_terminate_backend(' + str(snapshot.pid) + ',3000);')
        try:
            rebuild.snapshot(target, snapshot)
        except ValueError:
            pass
        else:
            raise ValueError('lost snapshot was reused')
        rebuild.fence()
        try:
            rebuild.cutover(target)
        except ValueError as error:
            require('incomplete or lost snapshot' in str(error), 'wrong incomplete-snapshot failure')
        else:
            raise ValueError('incomplete snapshot promoted')
        rebuild.abandon_snapshot(target)
        require(search.alias_target() == old, 'lost snapshot changed active index')
        db.put('c', 'fresh')
        fresh = prefix + '_fresh'
        source = stack.enter_context(Session(db, 'postgres'))
        rebuild.fence(); rebuild.register(fresh, source); rebuild.snapshot(fresh, source)
        rebuild.fence(); rebuild.cutover(fresh)
        require(search.read(fresh, 'c')['_source']['value'] == 'fresh', 'fresh epoch missing current data')
    return {'case': 'lost_snapshot_requires_fresh_epoch', 'passed': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--podman-machine', required=True)
    parser.add_argument('--exclusive-podman-machine', action='store_true')
    args = parser.parse_args()
    require(args.exclusive_podman_machine, 'SP-006 requires exclusive disposable machine use')
    temporary = Path(tempfile.gettempdir()).resolve()
    require(not temporary.is_relative_to(ROOT), 'actual temporary root must be outside checkout')
    for name in ('TEMP', 'TMP'):
        require(bool(os.environ.get(name)) and Path(os.environ[name]).resolve() == temporary,
                'TEMP/TMP must explicitly select the same external temporary root')
    preflight()
    with work_budget(600), managed_runtime('podman', args.podman_machine, exclusive=args.exclusive_podman_machine) as runtime:
        preflight()
        with service_group(runtime, ['postgres','opensearch'], compose_file=SOURCE / 'compose.yml') as containers:
            preflight()
            subprocess.run([sys.executable, str(ROOT / 'scripts/healthcheck.py'), '--wait','120','--services','PostgreSQL','OpenSearch'],
                           check=True, env=runtime.environment, timeout=command_timeout(130))
            db = Database(runtime, containers['postgres'])
            results = [sequence_counterexample(db)]
            for case in (live_rebuild, alias_recovery, snapshot_loss):
                preflight()
                results.append(case(db, 'sp006_' + uuid.uuid4().hex[:12]))
            print(json.dumps({'passed': len(results), 'cases': results}, sort_keys=True))


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_SP006_CHECK'))
