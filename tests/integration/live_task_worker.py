"""Live Rust task worker check against an installed disposable FerretDB collection."""
from concurrent.futures import ThreadPoolExecutor
import json
import secrets
import time
import urllib.error
import urllib.request
import uuid

BASE = 'http://127.0.0.1:8082'


def uuid7():
    milliseconds = time.time_ns() // 1_000_000
    bits = ((milliseconds & ((1 << 48) - 1)) << 80)
    bits |= 7 << 76
    bits |= secrets.randbits(12) << 64
    bits |= 2 << 62
    bits |= secrets.randbits(62)
    return str(uuid.UUID(int=bits))


def call(path, payload=None):
    body = json.dumps(payload).encode() if payload is not None else None
    request = urllib.request.Request(BASE + path, data=body,
                                     headers={'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(request, timeout=8) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        return error.code, json.load(error)


def main():
    task = 'rust-' + uuid.uuid4().hex[:12]
    first = {'operation_id': uuid7(), 'expected_version': 0,
             'title': 'Created', 'status': 'Open'}
    status, saved = call('/tasks/' + task, first)
    assert status == 200 and saved == {'id': task, 'version': 1, 'sync_token': None}, (status, saved)
    assert call('/tasks/' + task, first) == (200, saved)
    changed = {**first, 'title': 'Changed with reused ID'}
    assert call('/tasks/' + task, changed)[1]['error'] == 'operation_id_reused'
    stale = {**changed, 'operation_id': uuid7()}
    status, conflict = call('/tasks/' + task, stale)
    assert status == 409 and conflict == {'error': 'version_conflict', 'current_version': 1}
    updates = [{**first, 'operation_id': uuid7(), 'expected_version': 1,
                'title': f'Contender {number}'} for number in range(2)]
    with ThreadPoolExecutor(max_workers=2) as executor:
        results = list(executor.map(lambda payload: call('/tasks/' + task, payload), updates))
    assert sorted(code for code, _ in results) == [200, 409], results
    assert call('/tasks/' + task, first) == (200, saved)
    status, current = call('/tasks/' + task)
    assert status == 200 and current['version'] == 2, (status, current)
    assert current['title'] in {'Contender 0', 'Contender 1'}
    print(json.dumps({'task': task, 'saved_version': current['version'],
                      'conflicts': [body for code, body in results if code == 409]}))


if __name__ == '__main__':
    main()
