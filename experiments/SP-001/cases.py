"""Live SP-001 cases. Requires the isolated stack; fails closed without services."""
from concurrent.futures import ThreadPoolExecutor
import json
import time
import urllib.error
import urllib.request
import uuid

from manage import command, containers, ROOT

BASE = 'http://127.0.0.1:8080'
PREFIX = 'sp001-' + uuid.uuid4().hex[:10]
RESULTS = []


def http(path, payload=None):
    data = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(BASE + path, data=data, headers={'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(req, timeout=8) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        return error.code, json.load(error)


def wait_for(predicate, timeout=30):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            last = predicate()
            if last:
                return last
        except (OSError, ValueError) as error:
            last = str(error)
        time.sleep(0.2)
    raise AssertionError(f'Condition not reached within {timeout}s; last={last}')


def container(name):
    return next(x['Id'] for x in containers()
                if x['Config']['Labels']['io.podman.compose.service'] == name)


def control(action, name):
    command(['podman', action, container(name)], timeout=45)


def mutate(task, version, operation, title='Synthetic task'):
    return http('/tasks/' + task, {'expected_version': version, 'operation_id': operation,
                                  'title': title, 'status': 'Open'})


def saved(task, version, operation, title='Synthetic task'):
    status, body = mutate(task, version, operation, title)
    assert status == 200, (status, body)
    assert body['version'] == version + 1 and body['sync_token'] is None, body
    code, doc = http('/tasks/' + task)
    assert code == 200 and doc['version'] == version + 1 and doc['title'] == title, (code, doc)
    return body


def visible(task, version):
    def probe():
        status, body = http('/search/' + task)
        return body if status == 200 and body.get('version') == version else False
    return wait_for(probe)


def run_case(name, case):
    started = time.monotonic()
    try:
        evidence = case()
    except Exception as error:
        RESULTS.append({'case': name, 'outcome': 'fail', 'error': str(error),
                        'elapsed_seconds': round(time.monotonic() - started, 3)})
        raise
    RESULTS.append({'case': name, 'outcome': 'pass', 'evidence': evidence,
                    'elapsed_seconds': round(time.monotonic() - started, 3)})
    print(json.dumps(RESULTS[-1]), flush=True)


def c01():
    task = PREFIX + '-c01'
    saved(task, 0, 'create')
    visible(task, 1)
    saved(task, 1, 'update', 'Updated task')
    assert visible(task, 2)['title'] == 'Updated task'
    return 'Versions 1 and 2 persisted through FerretDB and became searchable'


def c02():
    task = PREFIX + '-c02'
    control('stop', 'cdc')
    saved(task, 0, 'create')
    control('kill', 'worker')
    control('start', 'cdc')
    try:
        visible(task, 1)
    finally:
        control('start', 'worker')
    wait_for(lambda: http('/health')[0] == 200)
    return 'CDC paused before save; worker killed after acknowledgment; CDC delivered with worker down'


def c03():
    task = PREFIX + '-c03'
    saved(task, 0, 'create')
    visible(task, 1)
    def checkpoint_exists():
        try:
            command(['podman', 'exec', container('cdc'), 'sh', '-c',
                     'test -s /debezium/data/offsets.dat'], capture=True)
            return True
        except RuntimeError:
            return False
    wait_for(checkpoint_exists)
    control('kill', 'cdc')
    saved(task, 1, 'update', 'Saved during CDC outage')
    control('start', 'cdc')
    assert visible(task, 2)['title'] == 'Saved during CDC outage'
    return 'CDC killed with persisted offset file; version 2 saved offline and delivered after restart'


def c04():
    task = PREFIX + '-c04'
    first = saved(task, 0, 'create')
    status, retry = mutate(task, 0, 'create')
    assert status == 200 and retry == first, (status, retry)
    status, _ = mutate(task, 0, 'create', 'Different content')
    assert status == 409, status
    visible(task, 1)
    saved(task, 1, 'update', 'Newer projection')
    visible(task, 2)
    proofs = []
    for version in (2, 1):
        result = command(['podman', 'exec', container('indexer'), 'python', '/app/service.py',
                          'replay', task, str(version)], capture=True)
        proof = json.loads(result)
        assert proof['ack_after'] >= proof['published_sequence'] > proof['ack_before'], proof
        proofs.append(proof)
    assert visible(task, 2)['title'] == 'Newer projection'
    assert http('/tasks/' + task)[1]['version'] == 2
    return proofs


def c05():
    task = PREFIX + '-c05'
    control('stop', 'indexer')
    saved(task, 0, 'create')
    assert http('/search/' + task)[0] == 404
    control('start', 'indexer')
    visible(task, 1)
    return 'Acknowledged write remained pending while indexer stopped, then became searchable'


def c06():
    task = PREFIX + '-c06'
    saved(task, 0, 'create')
    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = [executor.submit(mutate, task, 1, f'update-{n}', f'Contender {n}') for n in range(2)]
        outcomes = [future.result() for future in futures]
    assert sorted(code for code, _ in outcomes) == [200, 409], outcomes
    winner = next(body for code, body in outcomes if code == 200)
    assert winner['version'] == 2
    projected = visible(task, 2)
    current = http('/tasks/' + task)[1]
    assert current['version'] == 2 and current['title'] == projected['title']
    return {'statuses': sorted(code for code, _ in outcomes), 'saved_version': 2}


if __name__ == '__main__':
    folder = ROOT / '.yaja/spikes/SP-001'
    folder.mkdir(parents=True, exist_ok=True)
    try:
        # Immediate readiness failure is intentional; setup owns its 120-second wait.
        assert http('/health')[0] == 200
        for name, case in [('C01', c01), ('C02', c02), ('C03', c03), ('C04', c04),
                           ('C05', c05), ('C06', c06)]:
            run_case(name, case)
    finally:
        (folder / 'cases.json').write_text(json.dumps(RESULTS, indent=2), encoding='utf-8')
