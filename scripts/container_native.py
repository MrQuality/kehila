"""Serial real native product fixture inside an already-owned toolchain runner.

The caller owns provider supervision. This child owns its PostgreSQL dependency
and removes it before its namespace-owning runner can be removed.
"""
import json
import hashlib
from dataclasses import dataclass
import re
import subprocess

from cleanup_errors import retain_cleanup_failure
from verification_budget import cleanup_time
from native_authority_fixture import (AUTH_DATABASES, ITEM_DATABASES, ITEM_OBSERVERS,
    CAPACITY_MODE, CAPACITY_DATABASE, CAPACITY_OBSERVER, STORAGE_MODE, STORAGE_DATABASE,
    STORAGE_HISTORICAL_DATABASE, STORAGE_OBSERVER)
from capacity_plan import cell as planned_capacity_cell
from capacity_results import canonical
from storage_growth_plan import select as storage_samples
from storage_growth_results import validate_sample_observations
from native_database_limits import DEFAULT_LIMITS, storage_limits
from native_fixture_environment import NativeEnvironment


@dataclass(frozen=True)
class NumericToken:
    text: str


def snapshot_document(text):
    def invalid_constant(_value):
        raise ValueError('Nonstandard numeric token in native snapshot')
    # Preserve exact numeric tokens and distinguish numbers from booleans.
    # Ordinary Python equality would equate 1, 1.0 and True.
    return json.loads(text, parse_int=NumericToken, parse_float=NumericToken,
                      parse_constant=invalid_constant)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify_write_trace(expected, log):
    require(isinstance(expected, list) and expected
            and all(isinstance(item, str) and re.fullmatch(
                r'kehila(?:_auth)?\.[a-z_]+ (?:INSERT|UPDATE)', item) for item in expected),
            'Native write trace expectation is invalid')
    observed = [(int(ordinal), table + ' ' + operation)
                for ordinal, table, operation in re.findall(
                    r'NATIVE_WRITE (\d+) (kehila(?:_auth)?\.[a-z_]+) (INSERT|UPDATE)\b', log)]
    require(observed == list(enumerate(expected, 1)),
            'Native write trace differs from the complete ordered write plan')


def selected_item_cases(cases):
    """None selects full qualification; explicit selections never mean full."""
    if cases is None:
        return None
    require(isinstance(cases, (tuple, list)) and bool(cases)
            and all(isinstance(case, str) and case in ITEM_DATABASES for case in cases),
            'Select one or more known native item cases')
    require(len(set(cases)) == len(cases), 'Native item cases must not repeat')
    return tuple(case for case in ITEM_DATABASES if case in cases)


def check(executor, *, item_cases=None, evidence_directory=None, reset_assets=False,
          capacity_cell=None, capacity_observer='sampled', capacity_oracle_control=False,
          capacity_calibration=False, storage_sample=None, storage_profile='default'):
    require(type(capacity_oracle_control) is bool, 'Capacity oracle control must be boolean')
    require(type(capacity_calibration) is bool, 'Capacity calibration selector must be boolean')
    selected_cell = None
    if capacity_cell is not None:
        require(item_cases is None and evidence_directory is not None,
                'Select one explicit capacity cell with complete external evidence retention')
        selected_cell = planned_capacity_cell(capacity_cell)
        require(capacity_observer in {'off', 'stages', 'sampled'}, 'Unknown capacity observation mode')
        require(not capacity_calibration or (capacity_cell == 15 and capacity_observer == 'sampled'),
                'Known-blocker calibration requires separate sampled serial-edit cell 15')
    else:
        require(capacity_observer == 'sampled' and not capacity_oracle_control and not capacity_calibration,
                'Capacity controls require the dedicated cell selector')
    item_cases = selected_item_cases(item_cases)
    selected_storage = None
    require(type(storage_profile) is str and (storage_profile == 'default' or storage_sample is not None),
            'Storage resource profile requires an explicit storage sample')
    if storage_sample is not None:
        require(item_cases is None and selected_cell is None and evidence_directory is not None,
                'Select one explicit storage sample with complete external evidence retention')
        selected_storage = storage_samples([storage_sample])[0]
    limits = DEFAULT_LIMITS if selected_storage is None else storage_limits(storage_sample, storage_profile)
    full = item_cases is None and selected_cell is None and selected_storage is None
    items = ({STORAGE_MODE: STORAGE_DATABASE} if selected_storage is not None else
             {CAPACITY_MODE: CAPACITY_DATABASE} if selected_cell is not None else
             ITEM_DATABASES if full else {case: ITEM_DATABASES[case] for case in item_cases})
    observers = ({STORAGE_MODE: STORAGE_OBSERVER} if selected_storage is not None else
                 {CAPACITY_MODE: CAPACITY_OBSERVER} if selected_cell is not None else ITEM_OBSERVERS)
    selection = {'scope': 'storage-sample' if selected_storage is not None else
                 'capacity-cell' if selected_cell is not None else 'full-native' if full else 'focused-items',
                 'selected_items': list(items), 'observed_items': []}
    if selected_cell is not None:
        selection.update(plan=selected_cell.document(), observer=capacity_observer, calibration=capacity_calibration)
    if selected_storage is not None:
        selection.update(plan=selected_storage.document(), storage_profile=storage_profile)
    def observe_failure(error, environment):
        if selected_storage is not None and environment.attempted:
            # Preserve the actual committed population before destroying this
            # disposable fixture. A failed pair can contain accepted native
            # revisions even when its historical setup rolled back.
            counts = {}
            for role, database in (('native', STORAGE_DATABASE),
                                   ('historical', STORAGE_HISTORICAL_DATABASE)):
                try:
                    with cleanup_time():
                        result = executor.engine('exec', environment.name, 'psql', '-X', '-q', '-A', '-t',
                            '-U', 'postgres', '-d', database, '-v', 'ON_ERROR_STOP=1', '-c',
                            "SET statement_timeout='3s'; SELECT count(*) FROM kehila.item_revisions",
                            capture=True, timeout=15)
                    count = int(result.stdout.strip())
                    require(count >= 0, 'Storage failure population is negative')
                    counts[role] = {'accepted_revisions': count, 'observed': True,
                                    'within_bound': count <= 21}
                except BaseException as diagnostic:
                    counts[role] = {'accepted_revisions': None, 'observed': False,
                                    'failure_class': type(diagnostic).__name__}
            try:
                (environment.folder / 'storage-failure-counts.json').write_text(
                    json.dumps({'schema': 1, 'counts': counts}), encoding='utf-8')
            except BaseException as diagnostic:
                retain_cleanup_failure(error, diagnostic, 'storage failure population retention')

    databases = tuple((*AUTH_DATABASES.values(), *items.values()) if full else
        (*items.values(), STORAGE_HISTORICAL_DATABASE) if selected_storage is not None else items.values())
    environment = NativeEnvironment(executor, databases=databases,
        observers=tuple(observers.values()), selection=selection,
        evidence_directory=evidence_directory, reset_assets=reset_assets, limits=limits,
        failure_observer=observe_failure if selected_storage is not None else None)
    with environment.owned():
        work, control, psql = environment.work, environment.control, environment.psql
        folder, name, identity = environment.folder, environment.name, environment.identity
        # Exercise item capabilities before the existing project/auth journeys;
        # all isolated databases above were copied before any runtime started.
        for mode, database in items.items():
            environment.stage = mode
            command = ['python', 'scripts/native_authority_fixture.py', mode, '--seconds',
                       str(executor.settings['command_seconds'])]
            if selected_cell is not None:
                command += ['--capacity-cell-index', str(selected_cell.index), '--capacity-observer', capacity_observer]
                if capacity_calibration: command.append('--capacity-calibration')
            if selected_storage is not None:
                command += ['--storage-sample-index', str(selected_storage.index), '--storage-profile', storage_profile]
            work(command)
            if selected_cell is not None:
                environment.stage = 'capacity-client-lineage'
                controls = '/work/native-authority/controls/' + CAPACITY_MODE
                command = ['python', 'scripts/capacity_results.py', '--work', controls + '/matrix-work.json',
                           '--owned', controls + '/matrix-owned.json', '--cell', str(selected_cell.index),
                           '--observer', capacity_observer, '--output', controls]
                if capacity_oracle_control: command.append('--negative-control')
                if capacity_calibration: command.append('--calibration')
                work(command)
                control('cp', executor.container + ':' + controls + '/matrix-summary.json', str(folder / 'capacity-summary.json'))
                summary = json.loads((folder / 'capacity-summary.json').read_text(encoding='utf-8'))
                require(type(summary.get('schema')) is int and summary['schema'] == 1
                        and canonical(summary['plan']) == canonical(selected_cell.document())
                        and summary['observer'] == capacity_observer and summary['calibration'] is capacity_calibration,
                        'Capacity consumer returned another workload')
                if capacity_oracle_control:
                    environment.stage = 'capacity-oracle-negative-control'
                    control('cp', executor.container + ':' + controls + '/matrix-negative-expectations.jsonl',
                            str(folder / 'capacity-negative-expectations.jsonl'))
                    control('cp', str(folder / 'capacity-negative-expectations.jsonl'), identity + ':/tmp/native-capacity-expectations.jsonl')
                    try:
                        executor.engine('exec', name, 'psql', '-X', '-U', 'postgres', '-d', database,
                            '-v', 'ON_ERROR_STOP=1', '-f', '/tmp/' + CAPACITY_OBSERVER, capture=True)
                    except subprocess.CalledProcessError as rejected:
                        observed = (rejected.stdout or '') + (rejected.stderr or '')
                        (folder / 'capacity-oracle-negative-control.txt').write_text(observed, encoding='utf-8')
                        require('capacity final state mismatch' in observed,
                                'Capacity negative control failed before the intended state comparison')
                    else:
                        raise ValueError('Capacity SQL accepted an incorrect expected final field value')
                environment.stage = 'capacity-expectations-transfer'
                control('cp', executor.container + ':' + controls + '/matrix-expectations.jsonl',
                        str(folder / 'capacity-expectations.jsonl'))
                require(hashlib.sha256((folder / 'capacity-expectations.jsonl').read_bytes()).hexdigest()
                        == summary['expectations_sha256'], 'Capacity expectation transfer differs')
                control('cp', str(folder / 'capacity-expectations.jsonl'), identity + ':/tmp/native-capacity-expectations.jsonl')
            environment.stage = mode + '-independent-observer'
            (folder / (mode + '-observation.txt')).write_text(
                psql('/tmp/' + observers[mode], database), encoding='utf-8')
            if selected_storage is not None:
                environment.stage = 'storage-independent-historical-observer'
                historical_observation = folder / 'storage-historical-observation.txt'
                historical_observation.write_text(psql('/tmp/' + STORAGE_OBSERVER,
                    STORAGE_HISTORICAL_DATABASE), encoding='utf-8')
                environment.stage = 'storage-expectations-and-census-export'
                control('cp', executor.container + ':/work/native-authority/controls/' + STORAGE_MODE,
                        str(folder / 'storage'))
                environment.stage = 'storage-independent-state-comparison'
                validate_sample_observations(folder / 'storage',
                    folder / (mode + '-observation.txt'), historical_observation, selected_storage)
            selection['observed_items'].append(mode)
            (folder / 'selection.json').write_text(json.dumps(selection), encoding='utf-8')
        if full:
            environment.stage = 'journey'
            work(['python', 'scripts/native_authority_fixture.py', 'journey', '--seconds',
                  str(executor.settings['command_seconds'])])
            environment.stage = 'write-trace-observer'
            control('cp', executor.container + ':/work/native-authority/controls/write-trace.json',
                    str(folder / 'write-trace.json'))
            write_log = control('logs', '--timestamps', identity, both_streams=True)
            (folder / 'write-trace.log').write_text(write_log, encoding='utf-8')
            verify_write_trace(json.loads((folder / 'write-trace.json').read_text(encoding='utf-8')),
                               write_log)
            environment.stage = 'independent-observer'
            observation = psql('/tmp/native-authority-state.sql')
            (folder / 'observation.txt').write_text(observation, encoding='utf-8')
            environment.stage = 'clock-authority-journey'
            work(['python', 'scripts/native_authority_fixture.py', 'clock', '--seconds',
                  str(executor.settings['command_seconds'])])
            environment.stage = 'clock-independent-observer'
            (folder / 'clock-observation.txt').write_text(psql('/tmp/native-clock-state.sql'), encoding='utf-8')
            environment.stage = 'quiescent-copy'
            source_state = psql('/tmp/native_copy_snapshot.sql', tuples=True)
            require(len(source_state.encode('utf-8')) <= 1024**2, 'Synthetic copy observation exceeds bound')
            decoded = snapshot_document(source_state)
            require(isinstance(decoded, dict) and decoded.get('tables') and decoded.get('sequences')
                    and all(key in decoded for key in ('schemas','relations','types','indexes','columns','functions','constraints','triggers','raw_checks')),
                    'Synthetic copy observation is incomplete')
            (folder / 'copy-source-state.txt').write_text(source_state, encoding='utf-8')
            repeated_state = psql('/tmp/native_copy_snapshot.sql', tuples=True)
            (folder / 'copy-source-repeat-state.txt').write_text(repeated_state, encoding='utf-8')
            require(source_state == repeated_state, 'CHECK normalization changed production snapshot or was unstable')
            control('exec', identity, 'sh', '-c',
                    'set -eu; umask 077; mkdir -m 700 /tmp/native-copy; '
                    'pg_dump -U postgres -d postgres --format=custom --schema=kehila '
                    '--schema=kehila_auth --schema=kehila_native_fixture --file=/tmp/native-copy/state.dump; '
                    'test "$(stat -c %a /tmp/native-copy/state.dump)" = 600')
            control('exec', identity, 'createdb', '-U', 'postgres', '--owner=kehila_owner', 'kehila_native_restored')
            control('exec', identity, 'pg_restore', '-U', 'postgres', '-d', 'kehila_native_restored',
                    '--single-transaction', '--exit-on-error', '/tmp/native-copy/state.dump')
            restored_state = psql('/tmp/native_copy_snapshot.sql', 'kehila_native_restored', tuples=True)
            require(len(restored_state.encode('utf-8')) <= 1024**2, 'Restored copy observation exceeds bound')
            (folder / 'copy-restored-state.txt').write_text(restored_state, encoding='utf-8')
            restored_decoded = snapshot_document(restored_state)
            require(isinstance(restored_decoded, dict) and 'raw_checks' in restored_decoded,
                    'Restored copy observation is incomplete')
            # Both raw deparse forms stay in the private snapshots. Only CHECK
            # definitions use PostgreSQL's scratch-object reparse normalization;
            # every other captured field remains an exact comparison.
            decoded.pop('raw_checks')
            restored_decoded.pop('raw_checks')
            require(decoded == restored_decoded, 'Synthetic copy state/schema/authority differs before startup')
            environment.stage = 'restored-native-journey'
            work(['python', 'scripts/native_authority_fixture.py', 'restored', '--seconds',
                  str(executor.settings['command_seconds'])])
            environment.stage = 'restored-independent-observer'
            (folder / 'restored-observation.txt').write_text(
                psql('/tmp/native_restored_state.sql', 'kehila_native_restored'), encoding='utf-8')
            # The final test changes the source database's password. Keep it after
            # every journey that depends on its original fixture credential.
            for mode in ('auth-expiry', 'auth-delivery', 'auth-unknown'):
                environment.stage = mode
                work(['python', 'scripts/native_authority_fixture.py', mode, '--seconds',
                      str(executor.settings['command_seconds'])])
            environment.stage = 'authentication-independent-observer'
            (folder / 'authentication-observation.txt').write_text(
                psql('/tmp/native-clock-state.sql'), encoding='utf-8')
            for mode, database in AUTH_DATABASES.items():
                environment.stage = mode
                work(['python', 'scripts/native_authority_fixture.py', mode, '--seconds',
                      str(executor.settings['command_seconds'])])
                environment.stage = mode + '-independent-observer'
                (folder / (mode + '-observation.txt')).write_text(
                    psql('/tmp/native_auth_fixture_state.sql', database), encoding='utf-8')
    receipt = environment.receipt
    if receipt is not None:
        if selected_cell is not None:
            receipt.update(cell_index=selected_cell.index, observer=capacity_observer,
                calibration=capacity_calibration, independent_state_verified=True,
                oracle_negative_control=capacity_oracle_control)
        if selected_storage is not None:
            receipt.update(storage_sample_index=selected_storage.index,
                independent_state_verified=True, storage_profile=storage_profile)

    if full:
        print('PASS: native durable cardinalities, identity coherence and owned cleanup')
    elif selected_cell is not None:
        print('PASS: finite capacity cell, independent observer and owned cleanup')
    elif selected_storage is not None:
        print('PASS: paired storage sample, independent observers and owned cleanup')
    else:
        print('PASS: focused native items, independent observers and owned cleanup:', ', '.join(items))
    return receipt
