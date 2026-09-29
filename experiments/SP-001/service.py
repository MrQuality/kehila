"""Disposable Python adapters for SP-001; no production API or auth contract."""
import asyncio
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import re
import sys
import time
import urllib.error
import urllib.request

import nats
from nats.js.api import ConsumerConfig, AckPolicy
from pymongo import MongoClient
from pymongo.errors import DuplicateKeyError

ROLE = sys.argv[1] if len(sys.argv) > 1 else 'worker'
DB = MongoClient('mongodb://ferretdb:27017', serverSelectionTimeoutMS=3000,
                 connectTimeoutMS=3000, socketTimeoutMS=3000, maxPoolSize=4).sp001
STREAM = 'DebeziumStream'
DURABLE = 'sp001-indexer'


def request(url, payload=None, method=None):
    body = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(url, data=body, method=method,
                                 headers={'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(req, timeout=5) as result:
            return result.status, json.load(result)
    except urllib.error.HTTPError as error:
        return error.code, json.load(error)


def validate(payload):
    if not isinstance(payload, dict) or set(payload) != {
            'expected_version', 'operation_id', 'title', 'status'}:
        raise ValueError('Expected version, operation ID, title, and status are required')
    if type(payload['expected_version']) is not int or not 0 <= payload['expected_version'] < 16:
        raise ValueError('Invalid expected version')
    if not isinstance(payload['operation_id'], str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,64}', payload['operation_id']):
        raise ValueError('Invalid operation ID')
    if not isinstance(payload['title'], str) or not 1 <= len(payload['title']) <= 128:
        raise ValueError('Invalid title')
    if payload['status'] not in ('Open', 'Done'):
        raise ValueError('Invalid status')


def mutate(task, payload):
    validate(payload)
    digest = hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()
    for _ in range(3):
        current = DB.tasks.find_one({'_id': task})
        if current:
            for previous in current['operations']:
                if previous['id'] == payload['operation_id']:
                    if previous['digest'] != digest:
                        return 409, {'error': 'operation_id_reused'}
                    return 200, {'id': task, 'version': previous['version'], 'sync_token': None}
        version = current['version'] if current else 0
        if version != payload['expected_version']:
            return 409, {'error': 'version_conflict', 'current_version': version}
        if version >= 16:
            return 422, {'error': 'fixture_operation_limit'}
        operations = (current['operations'] if current else []) + [
            {'id': payload['operation_id'], 'digest': digest, 'version': version + 1}]
        replacement = {'_id': task, 'version': version + 1, 'title': payload['title'],
                       'status': payload['status'], 'operations': operations}
        if current:
            if not DB.tasks.replace_one({'_id': task, 'version': version}, replacement).matched_count:
                continue
        else:
            try:
                DB.tasks.insert_one(replacement)
            except DuplicateKeyError:
                continue
        return 200, {'id': task, 'version': version + 1, 'sync_token': None}
    return 409, {'error': 'concurrent_mutation'}


def public(doc):
    return {'id': doc['_id'], 'version': doc['version'], 'title': doc['title'], 'status': doc['status']}


class Handler(BaseHTTPRequestHandler):
    def handle_request(self, write=False):
        try:
            if self.path == '/health' and not write:
                if ROLE == 'api':
                    status, body = request('http://worker:8080/health')
                else:
                    DB.command('ping')
                    status, body = 200, {'ready': True}
            else:
                match = re.fullmatch(r'/(tasks|search)/(sp001-[A-Za-z0-9_-]{1,80})', self.path)
                if not match:
                    self.send_json(404, {'error': 'unknown_path'})
                    return
                kind, task = match.groups()
                payload = None
                if write:
                    length = int(self.headers.get('Content-Length', '0'))
                    if not 0 < length <= 4096:
                        raise ValueError('Body must be 1..4096 bytes')
                    payload = json.loads(self.rfile.read(length))
                if kind == 'search':
                    if write:
                        self.send_json(405, {'error': 'read_only'})
                        return
                    status, response = request('http://opensearch:9200/sp001/_search',
                                               {'query': {'ids': {'values': [task]}}})
                    if status == 200:
                        hits = response['hits']['hits']
                        status, body = (200, hits[0]['_source']) if hits else (404, {'pending': True})
                    elif status == 404:
                        body = {'pending': True}
                    else:
                        body = {'error': 'search_unavailable'}
                elif ROLE == 'api':
                    status, body = request('http://worker:8080' + self.path, payload)
                elif write:
                    status, body = mutate(task, payload)
                else:
                    doc = DB.tasks.find_one({'_id': task})
                    status, body = (200, public(doc)) if doc else (404, {'error': 'not_found'})
            self.send_json(status, body)
        except (ValueError, TypeError) as error:
            self.send_json(400, {'error': str(error)})
        except Exception as error:
            print(json.dumps({'error': str(error)}), flush=True)
            self.send_json(503, {'error': 'dependency_unavailable'})

    def send_json(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.handle_request()

    def do_POST(self):
        self.handle_request(True)


def decode_event(data):
    event = json.loads(data)
    after = event.get('after')
    if not after:
        raise ValueError('Only insert/update/snapshot events are in scope')
    raw = after.get('_jsonb')
    if raw is None:
        raise ValueError(f'Unverified FerretDB storage shape: {list(after)}')
    document = json.loads(raw) if isinstance(raw, str) else raw
    # Encoding is verified against the pinned FerretDB version before service cases.
    return public(document)


async def indexer():
    connection = await nats.connect('nats://nats:4222', connect_timeout=3,
                                    max_reconnect_attempts=5)
    jetstream = connection.jetstream()
    deadline = time.monotonic() + 120
    while True:
        try:
            subscription = await jetstream.pull_subscribe('sp001.>', durable=DURABLE,
                stream=STREAM, config=ConsumerConfig(ack_policy=AckPolicy.EXPLICIT,
                                                    ack_wait=5, max_ack_pending=32))
            break
        except nats.js.errors.NotFoundError:
            if time.monotonic() > deadline:
                raise
            await asyncio.sleep(0.2)
    while True:
        try:
            messages = await subscription.fetch(1, timeout=1)
        except nats.errors.TimeoutError:
            continue
        for message in messages:
            doc = decode_event(message.data)
            version = doc['version']
            if type(version) is not int or version < 1:
                raise ValueError('Invalid projection version')
            status, response = request(
                f'http://opensearch:9200/sp001/_doc/{doc["id"]}?version={version}&version_type=external&refresh=wait_for',
                doc, 'PUT')
            if status == 409 and response.get('error', {}).get('type') == 'version_conflict_engine_exception':
                pass  # Duplicate/older event; current version is already authoritative.
            elif status not in (200, 201):
                raise RuntimeError(f'Projection rejected: {status} {response}')
            await message.ack_sync(timeout=3)
            print(json.dumps({'indexed': doc['id'], 'version': version,
                              'stream_sequence': message.metadata.sequence.stream}), flush=True)


async def replay(task, version):
    connection = await nats.connect('nats://nats:4222')
    js = connection.jetstream()
    info = await js.stream_info(STREAM)
    message = None
    for sequence in range(info.state.last_seq, max(0, info.state.last_seq - 100), -1):
        candidate = await js.get_msg(STREAM, seq=sequence)
        document = decode_event(candidate.data)
        if document['id'] == task and document['version'] == version:
            message = candidate
            break
    if message is None:
        raise ValueError('Requested fixture event was not found')
    before = (await js.consumer_info(STREAM, DURABLE)).ack_floor.stream_seq
    published = await js.publish(message.subject, message.data)
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        after = (await js.consumer_info(STREAM, DURABLE)).ack_floor.stream_seq
        if after >= published.seq:
            print(json.dumps({'ack_before': before, 'ack_after': after,
                              'published_sequence': published.seq, 'replayed_version': version}))
            await connection.close()
            return
        await asyncio.sleep(0.1)
    raise RuntimeError('Duplicate event was not acknowledged')


if __name__ == '__main__':
    if ROLE in ('api', 'worker'):
        ThreadingHTTPServer(('0.0.0.0', 8080), Handler).serve_forever()
    elif ROLE == 'indexer':
        asyncio.run(indexer())
    elif ROLE == 'replay':
        asyncio.run(replay(sys.argv[2], int(sys.argv[3])))
    elif ROLE == 'seed':
        DB.tasks.insert_one({'_id': sys.argv[2], 'version': 1, 'title': 'Schema fixture',
                            'status': 'Open', 'operations': []})
    else:
        raise ValueError('Unknown role')
