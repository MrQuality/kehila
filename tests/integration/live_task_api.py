"""Live Go API boundary check against a running task worker and search stack.

Set YAJA_API_URL to a loopback Go task API. This is intentionally separate from
the default suite until the production worker and service orchestration exist.
"""
import json
import os
import time
import urllib.error
import urllib.request
import uuid

BASE = os.environ.get('YAJA_API_URL', 'http://127.0.0.1:8081')


def call(path, payload=None):
    body = json.dumps(payload).encode() if payload is not None else None
    request = urllib.request.Request(BASE + path, data=body,
                                     headers={'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        return error.code, json.load(error)


def main():
    task = 'sp001-go-' + uuid.uuid4().hex[:12]
    mutation = {'expected_version': 0, 'operation_id': 'create',
                'title': 'Go API live boundary', 'status': 'Open'}
    status, saved = call('/tasks/' + task, mutation)
    assert status == 200 and saved == {'id': task, 'version': 1, 'sync_token': None}, (status, saved)
    status, replay = call('/tasks/' + task, mutation)
    assert status == 200 and replay == saved, (status, replay)
    status, authoritative = call('/tasks/' + task)
    assert status == 200 and authoritative['version'] == 1, (status, authoritative)
    deadline = time.monotonic() + 30
    while True:
        status, projected = call('/search/' + task + '?min_version=1')
        if status == 200 and projected['version'] >= 1:
            break
        assert status == 404 and projected == {'pending': True}, (status, projected)
        if time.monotonic() >= deadline:
            raise AssertionError('Projection did not reach saved version within test deadline')
        time.sleep(0.2)
    print(json.dumps({'task': task, 'saved_version': saved['version'],
                      'projected_version': projected['version'], 'replay': 'pass'}))


if __name__ == '__main__':
    main()
