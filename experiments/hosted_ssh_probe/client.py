"""Local owner for a single temporary direct-SSH experiment.

Requires an already reviewed/pushed source commit, explicit experiment admission
and an empty external storage directory. It never merges or changes PR state.
All command output and endpoint metadata stay in the external storage directory.
"""
import argparse
import hashlib
from datetime import datetime
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import sys
import time
import traceback
import urllib.request

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
try:
    from postgresql_snapshots import bounded
finally:
    sys.path.pop(0)

from probe import (PORT, WORKFLOW, admit_descriptor, envelope_from_archive,
                   public_ipv4, public_key, read_json, require)

REPOSITORY = 'MrQuality/kehila'
VARIABLES = ('SP007_DIRECT_SSH_SHA', 'SP007_DIRECT_SSH_REF', 'SP007_DIRECT_SSH_PHASE')
SECRET = 'SP007_DIRECT_SSH_CONFIG'


class Experiment:
    def __init__(self, storage, sha, openssl):
        self.storage, self.sha, self.openssl = storage, sha, openssl
        self.tags, self.runs, self.variables, self.secret_metadata = [], [], {}, None
        self.variable_history = {}
        self.secret_pending = False
        self.secret_window = None
        self.ssh = shutil.which('ssh')
        require(self.ssh is not None)
        self.config = {}

    def journal(self):
        state = dict(sha=self.sha, tags=self.tags, runs=self.runs, variables=self.variables,
                     variable_history=self.variable_history, secret_metadata=self.secret_metadata,
                     secret_pending=self.secret_pending, secret_window=self.secret_window)
        pending = self.storage / 'ownership.pending'
        pending.write_text(json.dumps(state, indent=2))
        pending.replace(self.storage / 'ownership.json')

    def command(self, command, *, data=None, timeout=30, limit=4 * 1024 * 1024):
        # Reuse the existing bounded descendant owner; no snapshot workload runs.
        try:
            result = bounded(command, data=data or b'', timeout=timeout, limit=limit,
                             scratch_dir=self.storage)
        except subprocess.TimeoutExpired as error:
            with (self.storage / 'commands.log').open('ab') as log:
                log.write((error.stdout or b'') + (error.stderr or b''))
            raise
        with (self.storage / 'commands.log').open('ab') as log:
            log.write((str(len(result.stdout)) + ' ' + str(len(result.stderr)) + '\n').encode())
            log.write(result.stdout + result.stderr)
        require(result.returncode == 0)
        return result.stdout

    def api(self, path, *, method='GET', fields=None, raw=False):
        command = ['gh', 'api', '--method', method, 'repos/' + REPOSITORY + path]
        for key, value in (fields or {}).items():
            command += ['-f', key + '=' + value]
        result = self.command(command)
        return result if raw or not result else json.loads(result)

    def prepare(self):
        existing = self.api('/actions/variables?per_page=100')
        require(existing['total_count'] <= 100 and
                not any(v['name'] in VARIABLES for v in existing['variables']))
        existing = self.api('/actions/secrets?per_page=100')
        require(existing['total_count'] <= 100 and not any(s['name'] == SECRET for s in existing['secrets']))
        self.command([self.openssl, 'req', '-new', '-newkey', 'rsa:3072', '-x509', '-nodes',
            '-subj', '/CN=disposable-ssh-probe-recipient', '-days', '1',
            '-addext', 'basicConstraints=critical,CA:FALSE',
            '-addext', 'keyUsage=critical,keyEncipherment',
            '-keyout', str(self.storage / 'recipient.key'),
            '-out', str(self.storage / 'recipient.crt')])
        keygen = str(Path(self.ssh).with_name('ssh-keygen.exe' if os.name == 'nt' else 'ssh-keygen'))
        self.command([keygen, '-q', '-t', 'ed25519', '-N', '', '-C', '',
                      '-f', str(self.storage / 'client_key')])
        certificate = (self.storage / 'recipient.crt').read_bytes()
        request = urllib.request.Request('https://www.cloudflare.com/cdn-cgi/trace',
                                         headers={'User-Agent': 'Kehila-SSH-research'})
        with urllib.request.urlopen(request, timeout=10) as response:
            trace = response.read(4097)
        require(len(trace) <= 4096)
        (self.storage / 'local-egress-observation.txt').write_bytes(trace)
        fields = dict(line.split('=', 1) for line in trace.decode('ascii').splitlines() if '=' in line)
        self.config = dict(sha=self.sha, nonce=secrets.token_hex(32),
            recipient_pem=certificate.decode('ascii'), recipient_sha256=hashlib.sha256(certificate).hexdigest(),
            client_ipv4=public_ipv4(fields['ip']),
            client_public_key=public_key(' '.join((self.storage / 'client_key.pub').read_text().split()[:2])))

    def launch(self, phase):
        require(phase in {'control', 'live', 'admission'})
        tag = ('sp007-direct-probe-' + phase + '-' + time.strftime('%Y%m%d%H%M%S', time.gmtime())
               + '-' + secrets.token_hex(8))
        ref = 'refs/tags/' + tag
        require(not self.api('/git/matching-refs/tags/' + tag))
        self.config['ref'] = ref
        values = dict(zip(VARIABLES, (self.sha, ref, phase)))
        for name, value in values.items():
            updating = name in self.variables
            if updating:
                current = self.api('/actions/variables/' + name)
                require(current['value'] == self.variables[name])
            self.variables[name] = value
            self.variable_history.setdefault(name, []).append(value)
            self.journal() # Intent precedes mutation; lost responses remain reconcilable.
            if updating:
                self.api('/actions/variables/' + name, method='PATCH', fields={'value': value})
            else:
                self.api('/actions/variables', method='POST', fields={'name': name, 'value': value})
        # stdin avoids putting even public keys/session/source metadata in argv.
        if self.secret_metadata:
            require(self.api('/actions/secrets/' + SECRET) == self.secret_metadata)
        self.secret_pending = True
        self.secret_window = [time.time() - 10, time.time() + 40]
        self.journal()
        self.command(['gh', 'secret', 'set', SECRET, '--repo', REPOSITORY],
                     data=json.dumps(self.config).encode())
        self.secret_metadata = self.api('/actions/secrets/' + SECRET)
        self.secret_pending = False
        self.journal()
        # Record intended ownership before the mutation, including ambiguous API failures.
        self.tags.append(ref)
        self.journal()
        self.api('/git/refs', method='POST', fields={'ref': ref, 'sha': self.sha})
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            matching = self.discover_runs(ref)
            if matching:
                run = matching[0]
                require(run['head_sha'] == self.sha and run['event'] == 'push'
                        and run['run_attempt'] == 1 and run['actor']['login'] == 'MrQuality')
                (self.storage / (phase + '-run.json')).write_text(json.dumps(run))
                return run['id']
            time.sleep(3)
        raise TimeoutError('owned run was not observed')

    def discover_runs(self, ref):
        runs = self.api('/actions/runs?event=push&head_sha=' + self.sha + '&per_page=100')
        require(runs['total_count'] <= 100)
        matching = [r for r in runs['workflow_runs'] if r['head_branch'] == ref.removeprefix('refs/tags/')
                    and r['path'] == WORKFLOW]
        require(len(matching) <= 1)
        for run in matching:
            require(run['head_sha'] == self.sha and run['event'] == 'push'
                    and run['run_attempt'] == 1 and run['actor']['login'] == 'MrQuality')
            if run['id'] not in self.runs:
                self.runs.append(run['id'])
                self.journal()
        return matching

    def receive(self, run_id, phase):
        artifact_name = 'ssh-probe-' + ('control' if phase == 'control' else 'endpoint')
        member = 'control.der' if phase == 'control' else 'descriptor.der'
        deadline = time.monotonic() + 420
        while time.monotonic() < deadline:
            run = self.api('/actions/runs/' + str(run_id))
            require(run['head_sha'] == self.sha and run['run_attempt'] == 1 and run['path'] == WORKFLOW)
            artifacts = self.api('/actions/runs/' + str(run_id) + '/artifacts?per_page=100')
            require(artifacts['total_count'] <= 100)
            selected = [a for a in artifacts['artifacts'] if a['name'] == artifact_name]
            require(len(selected) <= 1)
            if selected:
                artifact = selected[0]
                producer = artifact['workflow_run']
                require(not artifact['expired'] and producer['id'] == run_id
                        and producer['head_sha'] == self.sha and artifact['size_in_bytes'] <= 36864)
                raw = self.api('/actions/artifacts/' + str(artifact['id']) + '/zip', raw=True)
                envelope = envelope_from_archive(raw, artifact['digest'], member)
                (self.storage / (phase + '-artifact.json')).write_text(json.dumps(artifact))
                encrypted = self.storage / (phase + '.der')
                encrypted.write_bytes(envelope)
                plain = self.storage / (phase + '-decoded.json')
                try:
                    self.command([self.openssl, 'cms', '-decrypt', '-binary', '-inform', 'DER',
                        '-in', str(encrypted), '-recip', str(self.storage / 'recipient.crt'),
                        '-inkey', str(self.storage / 'recipient.key'), '-out', str(plain)])
                except BaseException:
                    plain.unlink(missing_ok=True) # Never parse partial unauthenticated output.
                    raise
                result = read_json(plain.read_bytes())
                expected = dict(version=1, repository=REPOSITORY, sha=self.sha,
                    ref=self.config['ref'], workflow=WORKFLOW, run_id=str(run_id), attempt=1,
                    nonce=self.config['nonce'], recipient_sha256=self.config['recipient_sha256'])
                if phase == 'control':
                    control_expected = dict(expected, control='synthetic-only')
                    require(set(result) == set(control_expected) and
                            all(type(result[k]) is type(v) and result[k] == v
                                for k, v in control_expected.items()))
                else:
                    admit_descriptor(result, expected, time.time())
                # Single consumption; replays never re-enter this function.
                self.delete_artifacts(run_id)
                return result
            require(run['status'] != 'completed')
            time.sleep(3)
        raise TimeoutError('encrypted descriptor unavailable')

    def delete_artifacts(self, run_id):
        path = '/actions/runs/' + str(run_id) + '/artifacts?per_page=100'
        result = self.api(path)
        require(result['total_count'] <= 100)
        names = {'ssh-probe-control', 'ssh-probe-endpoint'}
        for artifact in result['artifacts']:
            if artifact['name'] in names:
                self.api('/actions/artifacts/' + str(artifact['id']), method='DELETE')
        require(not any(a['name'] in names for a in self.api(path)['artifacts']))

    def wait_complete(self, run_id, timeout=660):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            run = self.api('/actions/runs/' + str(run_id))
            require(run['head_sha'] == self.sha and run['run_attempt'] == 1)
            if run['status'] == 'completed':
                return run
            time.sleep(3)
        raise TimeoutError('owned run completion not observed')

    def connect(self, descriptor):
        known = self.storage / 'known_hosts'
        known.write_text('[' + descriptor['address'] + ']:' + str(PORT) + ' ' +
                         descriptor['host_public_key'] + '\n', encoding='ascii')
        options = {'BatchMode': 'yes', 'IdentitiesOnly': 'yes', 'IdentityAgent': 'none',
            'StrictHostKeyChecking': 'yes', 'UserKnownHostsFile': '"' + known.as_posix() + '"',
            'GlobalKnownHostsFile': 'none', 'KnownHostsCommand': 'none',
            'HostKeyAlgorithms': 'ssh-ed25519', 'PubkeyAcceptedAlgorithms': 'ssh-ed25519',
            'PasswordAuthentication': 'no', 'KbdInteractiveAuthentication': 'no',
            'ProxyCommand': 'none', 'ProxyJump': 'none', 'ControlMaster': 'no',
            'ControlPath': 'none', 'ForwardAgent': 'no', 'ConnectTimeout': '3', 'ConnectionAttempts': '1'}
        command = [self.ssh, '-F', 'none', '-T', '-i', str(self.storage / 'client_key')]
        for key, value in options.items():
            command += ['-o', key + '=' + value]
        command += ['-p', str(PORT), '-W', '127.0.0.1:31337', 'probe@' + descriptor['address']]
        request = ('GET /' + self.config['nonce'] + ' HTTP/1.0\r\nHost: synthetic\r\n\r\n').encode('ascii')
        attempts = []
        for _ in range(3):
            if time.time() + 5 >= descriptor['deadline']:
                break
            try:
                raw = self.command(command, data=request, timeout=5, limit=32768)
                (self.storage / ('ssh-response-' + str(len(attempts)))).write_bytes(raw)
                head, separator, body = raw.partition(b'\r\n\r\n')
                passed = bool(separator and head.startswith(b'HTTP/1.0 200 ') and
                              body == self.config['nonce'].encode('ascii'))
                attempts.append(passed)
                if passed:
                    break
            except (ValueError, subprocess.SubprocessError):
                attempts.append(False)
        (self.storage / 'local-attempts.json').write_text(json.dumps(attempts))
        return any(attempts)

    def cleanup(self):
        failures = []
        # Reconcile dispatch even if ref creation succeeded but its response/discovery failed.
        for ref in self.tags:
            try:
                matching = self.discover_runs(ref)
                refs = self.api('/git/matching-refs/' + ref.removeprefix('refs/'))
                exact = [r for r in refs if r['ref'] == ref]
                # An accepted ref mutation can precede Actions run registration.
                deadline = time.monotonic() + 120
                while exact and not matching and time.monotonic() < deadline:
                    time.sleep(3)
                    matching = self.discover_runs(ref)
                require(not exact or matching)
            except Exception:
                failures.append('run_discovery_cleanup')
        for run_id in self.runs:
            try:
                run = self.api('/actions/runs/' + str(run_id))
                if run['status'] != 'completed':
                    self.api('/actions/runs/' + str(run_id) + '/cancel', method='POST')
                    self.wait_complete(run_id)
            except Exception:
                failures.append('run_cleanup')
            try:
                self.delete_artifacts(run_id)
            except Exception:
                failures.append('artifact_cleanup')
        for ref in self.tags:
            try:
                refs = self.api('/git/matching-refs/' + ref.removeprefix('refs/'))
                exact = [r for r in refs if r['ref'] == ref]
                require(len(exact) <= 1)
                if not exact:
                    continue
                tag = exact[0]
                require(tag['object']['sha'] == self.sha and tag['object']['type'] == 'commit')
                self.api('/git/refs/' + ref.removeprefix('refs/'), method='DELETE')
            except Exception:
                failures.append('tag_cleanup')
        for name, value in self.variables.items():
            try:
                listing = self.api('/actions/variables?per_page=100')
                require(listing['total_count'] <= 100)
                if not any(v['name'] == name for v in listing['variables']):
                    continue
                require(self.api('/actions/variables/' + name)['value'] in self.variable_history[name])
                self.api('/actions/variables/' + name, method='DELETE')
            except Exception:
                failures.append('variable_cleanup')
        if self.secret_metadata or self.secret_pending:
            try:
                listing = self.api('/actions/secrets?per_page=100')
                require(listing['total_count'] <= 100)
                if not any(s['name'] == SECRET for s in listing['secrets']):
                    self.secret_pending = False
                    self.secret_metadata = None
                else:
                    self.cleanup_secret()
            except Exception:
                failures.append('secret_cleanup')
        for name in ('recipient.key', 'client_key'):
            try:
                (self.storage / name).unlink(missing_ok=True)
            except Exception:
                failures.append('local_key_cleanup')
        return failures

    def cleanup_secret(self):
        metadata = self.api('/actions/secrets/' + SECRET)
        if self.secret_pending:
            # Under the admitted single-operator scope, reconcile only this
            # write window and original creation identity. This is not CAS.
            updated = datetime.fromisoformat(metadata['updated_at'].replace('Z', '+00:00')).timestamp()
            require(self.secret_window[0] <= updated <= self.secret_window[1])
            if self.secret_metadata:
                require(metadata['created_at'] == self.secret_metadata['created_at'])
        else:
            require(metadata == self.secret_metadata)
        self.api('/actions/secrets/' + SECRET, method='DELETE')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--storage', type=Path, required=True)
    parser.add_argument('--sha', required=True)
    parser.add_argument('--openssl', required=True)
    parser.add_argument('--admission-only', action='store_true',
                        help='Run loopback-only hosted admission; never launch external SSH.')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    storage = args.storage.resolve()
    require(storage != root and root not in storage.parents and not storage.exists()
            and re.fullmatch(r'[0-9a-f]{40}', args.sha))
    storage.mkdir(mode=0o700)
    experiment = Experiment(storage, args.sha, args.openssl)
    result = dict(admission_only=args.admission_only, admission=False, control=False,
                  local_ssh=False, runner_response=False, reachability=False)
    try:
        experiment.prepare()
        if args.admission_only:
            admission = experiment.launch('admission')
            require(experiment.wait_complete(admission)['conclusion'] == 'success')
            result['admission'] = True
        else:
            control = experiment.launch('control')
            experiment.receive(control, 'control')
            require(experiment.wait_complete(control)['conclusion'] == 'success')
            result['control'] = True
            print('Linux-to-local encryption control passed; starting bounded SSH experiment.', flush=True)
            live = experiment.launch('live')
            descriptor = experiment.receive(live, 'live')
            result['local_ssh'] = experiment.connect(descriptor)
            require(experiment.wait_complete(live)['conclusion'] == 'success')
            logs = experiment.command(['gh', 'run', 'view', str(live), '--repo', REPOSITORY, '--log'])
            (storage / 'workflow.log').write_bytes(logs)
            reports = []
            for line in logs.decode('utf-8').splitlines():
                start = line.find('{"admission_only":')
                if start >= 0:
                    reports.append(json.loads(line[start:]))
            require(len(reports) == 1 and reports[0]['residual_check'])
            result['runner_response'] = reports[0].get('synthetic_responses', 0) > 0
            result['reachability'] = result['local_ssh'] and result['runner_response']
    except Exception as error:
        result['failure_class'] = type(error).__name__
        with (storage / 'failure.log').open('a') as log:
            traceback.print_exc(file=log)
    finally:
        result['cleanup_failures'] = experiment.cleanup()
        (storage / 'result.json').write_text(json.dumps(result, indent=2))
        print(json.dumps(result, sort_keys=True), flush=True)
    passed = result['admission'] if args.admission_only else result['reachability']
    return 0 if passed and not result['cleanup_failures'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
