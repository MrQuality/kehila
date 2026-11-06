#!/usr/bin/env python3
"""Container-only synthetic native fixture; generated assets stay under /work.

This fixture uses real database TLS and the product's listener connection path.
It is not browser, protected-startup, installation or corpus qualification.
"""
import argparse
import base64
import json
import math
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys

from verification_budget import command_timeout

ROOT = Path(__file__).resolve().parents[1]
WORK = Path('/work/native-authority')
# Operating-deadline qualification uses optimized code; ordinary debug tests
# remain in the shared matrix. Preparation and execution must select one profile.
NATIVE_TEST_COMMAND = ('cargo', 'test', '--release', '--offline', '--locked',
                       '-p', 'kehila_authority', '--lib')
TEST = 'coordinator::jobs::tests::project::native_project_creation_and_reconciliation'
CLOCK_TEST = 'coordinator::jobs::tests::project::clock::native_clock_retention_and_authority'
RESTORE_TEST = 'coordinator::jobs::tests::project::restored::native_restored_coherence'
AUTH_DATABASES = {
    'auth-owned': 'native_auth_owned',
    'auth-revoke': 'native_auth_revoke',
    'auth-fenced': 'native_auth_fenced',
    'auth-read': 'native_auth_read',
    'auth-lifetime': 'native_auth_lifetime',
}
ITEM_FIXTURES = {
    'item-read': ('native_item_read', 'native_item_read_state.sql'),
    'item-pending': ('native_item_pending', 'native_item_pending_state.sql'),
}
ITEM_DATABASES = {mode: database for mode, (database, _) in ITEM_FIXTURES.items()}
ITEM_OBSERVERS = {mode: observer for mode, (_, observer) in ITEM_FIXTURES.items()}


def prepare(seconds):
    if os.name != 'posix' or WORK.exists() or ROOT == WORK or ROOT in WORK.parents:
        raise ValueError('Fresh external Linux fixture required')
    WORK.mkdir(mode=0o700)
    pg = WORK / 'postgres'
    pg.mkdir(mode=0o700)
    (WORK / 'controls').mkdir(mode=0o700)
    with (WORK / 'preparation.log').open('xb') as log:
        def openssl(*args):
            result = subprocess.run(['openssl', *args], cwd=WORK, stdout=log,
                                    stderr=log, timeout=command_timeout(seconds))
            if result.returncode:
                raise RuntimeError('Native certificate preparation failed')
        openssl('genpkey', '-algorithm', 'EC', '-pkeyopt', 'ec_paramgen_curve:P-256', '-out', 'root-key.pem')
        openssl('req', '-new', '-x509', '-sha256', '-key', 'root-key.pem', '-out', 'root.pem',
                '-days', '2', '-subj', '/CN=Kehila Native Fixture CA',
                '-addext', 'basicConstraints=critical,CA:TRUE,pathlen:0',
                '-addext', 'keyUsage=critical,keyCertSign,cRLSign')
        openssl('genpkey', '-algorithm', 'EC', '-pkeyopt', 'ec_paramgen_curve:P-256',
                '-out', 'postgres/server-key.pem')
        openssl('req', '-new', '-sha256', '-key', 'postgres/server-key.pem',
                '-out', 'server.csr', '-subj', '/CN=auth-db')
        (WORK / 'server.ext').write_text('basicConstraints=critical,CA:FALSE\n'
            'keyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:auth-db\n',
            encoding='utf-8')
        openssl('x509', '-req', '-sha256', '-in', 'server.csr', '-CA', 'root.pem',
                '-CAkey', 'root-key.pem', '-set_serial', '1', '-out', 'postgres/server.pem',
                '-days', '2', '-extfile', 'server.ext')
        openssl('x509', '-in', 'root.pem', '-outform', 'DER', '-out', 'root.der')
        openssl('verify', '-CAfile', 'root.pem', '-purpose', 'sslserver',
                '-verify_hostname', 'auth-db', 'postgres/server.pem')
    runtime_password = secrets.token_urlsafe(32)
    (pg / 'admin-password').write_text(secrets.token_urlsafe(32), encoding='ascii')
    verifier = ('$argon2id$v=19$m=65536,t=3,p=1$AQEBAQEBAQEBAQEBAQEBAQ$'
                '6d50UfWIhEf6nFrBV/eh7X/wOi7ToEKiEY6e0UdWGa8')
    # The generated secret alphabet excludes SQL quotation characters.
    assert re.fullmatch(r'[A-Za-z0-9_-]{43}', runtime_password)
    (pg / 'setup.sql').write_text(
        "CREATE ROLE kehila_auth_runtime_login LOGIN NOINHERIT NOSUPERUSER NOCREATEDB "
        "NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD '" + runtime_password + "';\n"
        "GRANT kehila_auth_runtime TO kehila_auth_runtime_login;\nSET ROLE kehila_auth_admin;\n"
        "SELECT * FROM kehila_auth.enroll('installation','owner','native-enrollment','" + verifier + "');\n"
        "RESET ROLE; CREATE SCHEMA kehila_native_fixture AUTHORIZATION kehila_owner;\n"
        "REVOKE ALL ON SCHEMA kehila_native_fixture FROM PUBLIC; SET ROLE kehila_owner;\n"
        "CREATE TABLE kehila_native_fixture.marker(value text NOT NULL);\n"
        "INSERT INTO kehila_native_fixture.marker VALUES ('disposable native fixture');\n"
        "REVOKE ALL ON kehila_native_fixture.marker FROM PUBLIC; RESET ROLE;\n",
        encoding='utf-8')
    encoded_root = base64.b64encode((WORK / 'root.der').read_bytes()).decode('ascii')
    (WORK / 'input').write_text('127.0.0.1:5432\n' + runtime_password + '\n' + encoded_root +
                              '\ninstallation\nnative-project-attempt\n', encoding='ascii')
    for file in WORK.rglob('*'):
        file.chmod(0o700 if file.is_dir() else 0o600)


SPECS = {
    'item-read': ('coordinator::jobs::tests::project::item_reads::native_authorized_item_reads', 'item-read-passed'),
    'item-pending': ('coordinator::jobs::tests::project::item_pending::native_pending_creation_lifecycle', 'item-pending-passed'),
    'journey': (TEST, 'passed'),
    'clock': (CLOCK_TEST, 'clock-passed'),
    'restored': (RESTORE_TEST, 'restore-passed'),
    'auth-expiry': ('coordinator::jobs::tests::expiry::native_cli_expiry_over_tls', None),
    'auth-delivery': ('coordinator::jobs::tests::delivery::native_cli_delivery_without_peer_close', None),
    'auth-unknown': ('coordinator::jobs::tests::unknown_commit::native_unknown_commit_consumers', 'passed'),
    'auth-owned': ('coordinator::jobs::tests::native_owned_login', 'passed'),
    'auth-revoke': ('coordinator::jobs::tests::revoke_others::native_revoke_others', 'passed'),
    'auth-fenced': ('postgres::transaction::tests::native_fenced_roundtrip', 'passed'),
    'auth-read': ('postgres::tests::native_cancelled_read', 'passed'),
    'auth-lifetime': ('postgres::tests::native_lifetime', 'passed'),
}


def failure_metadata(mode, returncode, summary, marker_required, marker_present):
    # Only closed labels, numeric observations and known public source locations.
    modes = set(SPECS)
    labels = {'pre_session', 'pre_csrf', 'login', 'state', 'logout',
              'password_change', 'revoke_others', 'project', 'item', 'item_pending'}
    jobs = []
    for label, elapsed in re.findall(r'^owned_job ([a-z_]+) elapsed_us=([0-9]{1,12})$',
                                     summary, re.MULTILINE):
        if label in labels:
            jobs.append({'operation': label, 'elapsed_us': int(elapsed)})
    crate = ROOT / 'src/io/kehila_authority'
    locations = {}
    for source in (crate / 'src').rglob('*.rs'):
        public = source.relative_to(ROOT).as_posix()
        locations[public] = public
        locations[source.relative_to(crate).as_posix()] = public
    panic = None
    for source, line, column in re.findall(
            r'panicked at ([^\s:]+\.rs):([0-9]{1,8}):([0-9]{1,8})', summary):
        if source in locations:
            panic = {'source': locations[source], 'line': int(line), 'column': int(column)}
            break
    return {'event': 'native_fixture_failure', 'stage': 'completed_cargo_result',
            'mode': mode if mode in modes else 'unknown', 'cargo_exit': returncode,
            'success_summary': 'test result: ok. 1 passed; 0 failed;' in summary,
            'marker_required': marker_required, 'marker_present': marker_present,
            'jobs': jobs[:64], 'jobs_truncated': len(jobs) > 64,
            'public_panic_location': panic}


def journey(seconds, mode='journey'):
    test, marker = SPECS[mode]
    controls = WORK / 'controls'
    databases = {**AUTH_DATABASES, **ITEM_DATABASES}
    if mode in databases:
        controls /= mode
        controls.mkdir(mode=0o700)
    environment = dict(os.environ, AUTH001_DB_CONTROLS=str(controls))
    if mode in databases:
        environment['KEHILA_NATIVE_FIXTURE_DATABASE'] = databases[mode]
    elif 'KEHILA_NATIVE_FIXTURE_DATABASE' in environment:
        raise ValueError('Unexpected authentication fixture database override')
    if mode in {'journey', 'clock', 'restored'} or mode in databases:
        environment['KEHILA_NATIVE_FIXTURE_ADMIN_PASSWORD_FILE'] = str(WORK / 'postgres/admin-password')
    marker_path = controls / marker if marker else None
    if mode == 'auth-unknown' and marker_path.exists():
        marker_path.unlink()  # This test must create its own final marker.
    fields = (WORK / 'input').read_bytes().splitlines()
    if len(fields) != 5:
        raise ValueError('Expected the closed native fixture input')
    fields[4] += b'-' + mode.encode('ascii')
    incoming_path = WORK / (mode + '-input')
    with incoming_path.open('xb') as prepared:
        prepared.write(b'\n'.join(fields) + b'\n')
    incoming_path.chmod(0o600)
    with incoming_path.open('rb') as incoming, (WORK / (mode + '.log')).open('xb') as output:
        result = subprocess.run([*NATIVE_TEST_COMMAND, test,
            '--', '--ignored', '--exact', '--nocapture'],
            cwd=ROOT, env=environment, stdin=incoming, stdout=output, stderr=output,
            timeout=command_timeout(seconds))
    with (WORK / (mode + '.log')).open('rb') as log:
        summary_bytes = log.read(1024**2 + 1)
    if len(summary_bytes) > 1024**2:
        raise RuntimeError('Native journey diagnostic limit exceeded')
    summary = summary_bytes.decode('utf-8')
    if result.returncode or 'test result: ok. 1 passed; 0 failed;' not in summary \
            or (marker_path is not None and not marker_path.is_file()):
        failure = RuntimeError('Native authority journey failed; external diagnostics retained')
        try:
            print(json.dumps(failure_metadata(mode, result.returncode, summary,
                marker_path is not None, marker_path is not None and marker_path.is_file())),
                file=sys.stderr, flush=True)
        except Exception:
            pass  # Optional reporting must not replace the primary failure.
        raise failure from None
    print('PASS: provisioned native fixture ' + mode)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['prepare', *SPECS])
    parser.add_argument('--seconds', type=float, required=True)
    args = parser.parse_args()
    if not math.isfinite(args.seconds) or args.seconds <= 0:
        parser.error('Positive command allowance required')
    if args.mode == 'prepare':
        prepare(args.seconds)
    else:
        journey(args.seconds, args.mode)


if __name__ == '__main__':
    main()
