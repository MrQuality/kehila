"""Repeatable Go/Rust regression against disposable FerretDB (no CDC required).

Builds and owns two workers, a Go API, a fault proxy, and one unique collection.
Requires a running single-host FerretDB at KEHILA_TEST_MONGO_URL (default localhost).
"""
from concurrent.futures import ThreadPoolExecutor
from contextlib import ExitStack
import argparse
import json
import os
from pathlib import Path
import re
import select
import shutil
import socket
import socketserver
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

from live_task_worker import uuid7

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from temp_work import owned_directory
from verification_budget import command_timeout, cleanup_time, DeadlineExceeded


def call(base, path, payload=None, headers=None):
    body = json.dumps(payload).encode() if payload is not None else None
    request = urllib.request.Request(base + path, data=body,
                                    headers=headers or {'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(request, timeout=command_timeout(8)) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        return error.code, json.load(error)


def wait_ready(base, processes, timeout=25):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        command_timeout(timeout)
        if any(process.poll() is not None for process in processes):
            raise AssertionError('A task-path process exited before readiness')
        try:
            if call(base, '/health') == (200, {'ready': True}):
                return
        except DeadlineExceeded:
            raise
        except OSError:
            pass
        time.sleep(0.1)
    raise AssertionError('Task-path readiness deadline exceeded')


def bound_base(log_path, process, timeout=25):
    """Discover the address already owned by this child; never probe/release a port."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        command_timeout(timeout)
        with log_path.open('rb') as log:
            output = log.read(8193)
        if len(output) > 8192:
            raise AssertionError('Child startup output exceeds 8 KiB')
        if process.poll() is not None:
            raise AssertionError('Child exited before address discovery: ' +
                                 output.decode('utf-8', errors='replace'))
        lines = [line for line in output.splitlines(keepends=True)
                 if line.startswith(b'KEHILA_BOUND_ADDR=') and line.endswith(b'\n')]
        if len(lines) > 1:
            raise AssertionError('Child reported multiple bound addresses')
        if lines:
            match = re.fullmatch(rb'KEHILA_BOUND_ADDR=127\.0\.0\.1:([0-9]{1,5})\r?\n', lines[0])
            if match is None or not 1 <= int(match.group(1)) <= 65535:
                raise AssertionError('Child reported an invalid bound address')
            return 'http://127.0.0.1:' + str(int(match.group(1)))
        time.sleep(0.05)
    raise AssertionError('Child bound-address deadline exceeded')


class FaultProxy(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, target):
        self.target = target
        self.paused = threading.Event()
        self.stopping = threading.Event()
        super().__init__(('127.0.0.1', 0), Relay)
        self.thread = threading.Thread(target=self.serve_forever, daemon=True)
        self.thread.start()

    def close(self):
        self.stopping.set()
        self.shutdown()
        self.server_close()
        self.thread.join(timeout=3)


class Relay(socketserver.BaseRequestHandler):
    def handle(self):
        try:
            with socket.create_connection(self.server.target, timeout=2) as upstream:
                self.request.settimeout(1)
                upstream.settimeout(1)
                sockets = [self.request, upstream]
                while not self.server.stopping.is_set():
                    if self.server.paused.is_set():
                        self.server.stopping.wait(0.05)
                        continue
                    for source in select.select(sockets, [], [], 0.1)[0]:
                        data = source.recv(65536)
                        if not data:
                            return
                        (upstream if source is self.request else self.request).sendall(data)
        except OSError:
            return


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def race(bases, task, requests):
    barrier = threading.Barrier(2)

    def contender(index):
        barrier.wait(timeout=5)
        return call(bases[index], '/tasks/' + task, requests[index])

    with ThreadPoolExecutor(max_workers=2) as executor:
        return list(executor.map(contender, range(2)))


def mutation(version=0, title='Task'):
    return {'operation_id': uuid7(), 'expected_version': version,
            'title': title, 'status': 'Open'}


def concurrency(bases):
    for iteration in range(12):
        task = 'competing-' + str(iteration)
        results = race(bases, task, [mutation(), mutation()])
        assert sorted(code for code, _ in results) == [200, 409], results
        assert next(body for code, body in results if code == 409)['error'] == 'version_conflict'
        results = race(bases, task, [mutation(1, 'A'), mutation(1, 'B')])
        assert sorted(code for code, _ in results) == [200, 409], results
        assert call(bases[0], '/tasks/' + task)[1]['version'] == 2

        task = 'identical-' + str(iteration)
        first = mutation()
        results = race(bases, task, [first, first])
        saved = {'id': task, 'version': 1, 'sync_token': None}
        assert results == [(200, saved), (200, saved)], results
        assert call(bases[1], '/tasks/' + task, mutation(1))[0] == 200
        assert call(bases[0], '/tasks/' + task, first) == (200, saved)

        task = 'reused-' + str(iteration)
        first = mutation()
        results = race(bases, task, [first, {**first, 'title': 'Changed'}])
        assert sorted(code for code, _ in results) == [200, 409], results
        assert next(body for code, body in results if code == 409)['error'] == 'operation_id_reused', results
        assert call(bases[1], '/tasks/' + task)[1]['version'] == 1
    print('PASS: 48 competing create/update/retry/reuse pairs across two workers', flush=True)


def boundary(base):
    first = mutation()
    for headers, status in [
            ({'Content-Type': 'text/plain'}, 415),
            ({'Content-Type': 'application/json', 'Origin': 'https://example.invalid'}, 403),
            ({'Content-Type': 'application/json', 'Origin': base}, 403),
            ({'Content-Type': 'application/json', 'Sec-Fetch-Site': 'cross-site'}, 403),
            ({'Content-Type': 'application/json', 'Host': 'example.invalid'}, 403)]:
        assert call(base, '/tasks/forbidden', first, headers)[0] == status
    assert call(base, '/tasks/forbidden')[0] == 404
    address = urllib.parse.urlsplit(base)
    with socket.create_connection((address.hostname, address.port), timeout=command_timeout(6)) as slow:
        slow.sendall(('POST /tasks/slow HTTP/1.1\r\nHost: ' + address.netloc +
                      '\r\nContent-Type: application/json\r\nContent-Length: 4096\r\n\r\n{').encode())
        started = time.monotonic()
        assert call(base, '/health')[0] == 200
        assert call(base, '/tasks/forbidden')[0] == 404
        assert time.monotonic() - started < 2
        slow.settimeout(command_timeout(6))
        assert b'408' in slow.recv(4096).split(b'\r\n', 1)[0]
        assert time.monotonic() - started < 5
    assert call(base, '/health')[0] == 200
    print('PASS: direct-worker guards and stalled-body deadline with concurrent reads', flush=True)


def database_outage(bases, proxy, processes):
    proxy.paused.set()
    try:
        started = time.monotonic()
        with ThreadPoolExecutor(max_workers=2) as executor:
            health = executor.submit(call, bases[0], '/health')
            read = executor.submit(call, bases[0], '/tasks/identical-0')
            assert call(bases[1], '/tasks/identical-0')[0] == 200
            assert health.result()[0] == 503
            assert read.result()[0] == 503
        assert time.monotonic() - started < 7
    finally:
        proxy.paused.clear()
    wait_ready(bases[0], processes)
    assert call(bases[0], '/tasks/identical-0')[0] == 200
    print('PASS: stalled database requests terminate and worker recovers', flush=True)


def prepare(folder):
    """Build both tested executables before any external service is required."""
    subprocess.run(['cargo', 'build', '--locked', '--jobs', '1', '-p', 'task_worker'],
                   cwd=ROOT, check=True, timeout=command_timeout(240))
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--format-version', '1'], cwd=ROOT, timeout=command_timeout(30)))
    extension = '.exe' if os.name == 'nt' else ''
    shutil.copy2(Path(metadata['target_directory']) / 'debug' / ('task_worker' + extension),
                 folder / ('task_worker' + extension))
    subprocess.run(['go', 'build', '-p', '1', '-o', str(folder / ('task-api' + extension)),
                    './go/io/task_api/cmd/task-api'], cwd=ROOT, check=True, timeout=command_timeout(90))


def exercise(folder):
    uri = os.environ.get('KEHILA_TEST_MONGO_URL', 'mongodb://127.0.0.1:27017')
    parsed = urllib.parse.urlsplit(uri)
    if parsed.scheme != 'mongodb' or not parsed.hostname or ',' in parsed.netloc:
        raise ValueError('The fault test requires a single-host mongodb URI')
    extension = '.exe' if os.name == 'nt' else ''
    worker = str(folder / ('task_worker' + extension))
    api = str(folder / ('task-api' + extension))
    if not all(Path(binary).is_file() for binary in (worker, api)):
        raise ValueError('Both task-path executables must be prepared before live verification')
    environment = dict(os.environ, KEHILA_MONGO_URL=uri, KEHILA_TASK_DB='yaja',
                       KEHILA_TASK_COLLECTION='kehila_test_' + uuid.uuid4().hex)
    with ExitStack() as owned:
        # Register cleanup before installation, including partially created indexes.
        def drop_collection():
            with cleanup_time():
                subprocess.run([worker, 'drop-test-collection'], env=environment, check=True, timeout=15)
        owned.callback(drop_collection)
        subprocess.run([worker, 'install-indexes'], env=environment, check=True, timeout=command_timeout(15))
        proxy = FaultProxy((parsed.hostname, parsed.port or 27017))
        owned.callback(proxy.close)
        processes = []

        def launch(command, env):
            command_timeout(1)
            log = owned.enter_context(open(Path(folder) / ('process-' + str(len(processes)) + '.log'), 'w+'))
            process = subprocess.Popen(command, env=env, cwd=ROOT, stdout=log, stderr=log,
                                       creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
            owned.callback(stop, process)
            processes.append(process)
            return process, Path(log.name)

        bases = []
        for index in range(2):
            worker_env = dict(environment, KEHILA_WORKER_LISTEN_ADDR='127.0.0.1:0')
            if index == 0:
                credentials = parsed.netloc.rsplit('@', 1)[0] + '@' if '@' in parsed.netloc else ''
                worker_env['KEHILA_MONGO_URL'] = urllib.parse.urlunsplit(parsed._replace(
                    netloc=credentials + '127.0.0.1:' + str(proxy.server_address[1])))
            process, log_path = launch([worker], worker_env)
            bases.append(bound_base(log_path, process))
            wait_ready(bases[-1], processes)
        concurrency(bases)
        boundary(bases[0])
        database_outage(bases, proxy, processes)
        # Reserve an unserved socket to make search unavailability deterministic.
        unavailable = owned.enter_context(socket.socket())
        unavailable.bind(('127.0.0.1', 0))
        process, log_path = launch(
            [api], dict(environment, KEHILA_LISTEN_ADDR='127.0.0.1:0',
                        KEHILA_WORKER_URL=bases[0],
                        KEHILA_SEARCH_URL='http://127.0.0.1:' + str(unavailable.getsockname()[1])))
        api_base = bound_base(log_path, process)
        wait_ready(api_base, processes)
        first = mutation()
        saved = call(api_base, '/tasks/go-rust', first,
                     {'Content-Type': 'application/json', 'Origin': api_base,
                      'Sec-Fetch-Site': 'same-origin'})
        assert saved == (200, {'id': 'go-rust', 'version': 1, 'sync_token': None}), saved
        assert call(api_base, '/tasks/go-rust', first) == saved
        assert call(api_base, '/search/go-rust?min_version=1') == (503, {'error': 'search_unavailable'})
        assert call(api_base, '/tasks/go-rust')[1]['version'] == 1
        print('PASS: Go-to-Rust save/replay/read with search unavailable', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--build-only', type=Path, metavar='DIRECTORY')
    modes.add_argument('--run-only', type=Path, metavar='DIRECTORY')
    args = parser.parse_args()
    if args.build_only is not None:
        prepare(args.build_only.resolve(strict=True))
    elif args.run_only is not None:
        exercise(args.run_only.resolve(strict=True))
    else:
        with owned_directory('kehila-task-path-') as folder:
            prepare(folder)
            exercise(folder)


if __name__ == '__main__':
    main()
