"""Live Go API to Rust worker check with the search service stopped."""
import json
import urllib.error
import urllib.request
import uuid

from live_task_worker import uuid7

BASE = 'http://127.0.0.1:8081'


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
    task = 'gorust-' + uuid.uuid4().hex[:12]
    mutation = {'operation_id': uuid7(), 'expected_version': 0,
                'title': 'Saved through Go and Rust', 'status': 'Open'}
    status, saved = call('/tasks/' + task, mutation)
    assert status == 200 and saved == {'id': task, 'version': 1, 'sync_token': None}, (status, saved)
    assert call('/tasks/' + task, mutation) == (200, saved)
    status, authoritative = call('/tasks/' + task)
    assert status == 200 and authoritative['version'] == 1, (status, authoritative)
    status, search = call('/search/' + task + '?min_version=1')
    assert status == 503 and search['error'] == 'search_unavailable', (status, search)
    assert call('/tasks/' + task)[0] == 200
    print(json.dumps({'task': task, 'saved_version': 1,
                      'authoritative_read': 'available', 'search': 'unavailable'}))


if __name__ == '__main__':
    main()
