"""Bounded, local-only SP-001 runtime commands; host Python has no dependencies."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
ENV = dict(os.environ)
if sys.platform == 'win32':
    ENV['CONTAINER_CONNECTION'] = os.environ.get('SP001_CONNECTION', 'yaja-sp001')
COMPOSE = ['podman-compose', '--no-ansi', '-p', 'yaja-sp001', '-f', str(HERE / 'compose.yml')]
SERVICES = ['postgres', 'ferretdb', 'nats', 'opensearch', 'cdc', 'worker', 'api', 'indexer']
LIMITS = dict(zip(SERVICES, [256, 128, 128, 1536, 768, 128, 128, 128]))
IMAGES = ['docker.io/library/postgres:16.13', 'ghcr.io/ferretdb/ferretdb:1.24.2',
          'docker.io/library/nats:2.10.26-alpine',
          'docker.io/opensearchproject/opensearch:2.19.1',
          'docker.io/library/python:3.12.12-slim', 'quay.io/debezium/server:3.2.4.Final']


def command(args, timeout=120, capture=False):
    result = subprocess.run(args, cwd=HERE, env=ENV, timeout=timeout,
                            text=True, encoding='utf-8', errors='replace',
                            stdout=subprocess.PIPE if capture else None,
                            stderr=subprocess.PIPE if capture else None)
    if result.returncode:
        raise RuntimeError(f'{args[0]} failed ({result.returncode}): {result.stderr or ""}')
    return result.stdout


def available_mib():
    if sys.platform == 'win32':
        class Memory(ctypes.Structure):
            _fields_ = [('length', ctypes.c_ulong), ('load', ctypes.c_ulong)] + [
                (name, ctypes.c_ulonglong) for name in
                ('total', 'available', 'page_total', 'page_available', 'virtual_total',
                 'virtual_available', 'extended')]
        value = Memory()
        value.length = ctypes.sizeof(value)
        if not ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(value)):
            raise OSError('Cannot read physical memory')
        return value.available // (1024 * 1024)
    values = dict(line.split(':', 1) for line in Path('/proc/meminfo').read_text().splitlines())
    return int(values['MemAvailable'].split()[0]) // 1024


def containers():
    ids = command(['podman', 'ps', '-aq', '--filter',
                   'label=io.podman.compose.project=yaja-sp001'], capture=True).split()
    return json.loads(command(['podman', 'inspect', *ids], capture=True)) if ids else []


def check_limits(names=None):
    observed = {}
    for item in containers():
        name = item['Config']['Labels']['io.podman.compose.service']
        if names is not None and name not in names:
            continue
        memory = item['HostConfig']['Memory'] // (1024 * 1024)
        if memory != LIMITS[name]:
            raise RuntimeError(f'{name}: memory limit {memory} != {LIMITS[name]} MiB')
        if item['HostConfig']['MemorySwap'] != item['HostConfig']['Memory']:
            raise RuntimeError(f'{name}: swap limit differs from the memory limit')
        if item['State'].get('OOMKilled'):
            raise RuntimeError(f'{name}: OOM killed')
        observed[name] = memory
    print(json.dumps({'enforced_limits_mib': observed}))
    return observed


def start_services(names):
    # podman-compose 1.6.0 ignores memswap_limit; enforce it before processes run.
    for name in names:
        command(COMPOSE + [f'--podman-run-args=--memory-swap={LIMITS[name]}m',
                           'up', '--no-start', '--no-deps', '--force-recreate', name], timeout=120)
    check_limits(names)
    command(COMPOSE + ['start', *names], timeout=120)


def stop_existing():
    ids = [item['Id'] for item in containers() if item['State']['Running']]
    if ids:
        command(['podman', 'stop', '--time', '5', *ids], timeout=90)


def preflight():
    available = available_mib()
    required = sum(LIMITS.values()) + 1024
    if available < required:
        raise RuntimeError(f'Need {required} MiB host available after VM startup, found {available}')
    if shutil.disk_usage(ROOT).free < 8 * 1024 ** 3:
        raise RuntimeError('Need at least 8 GiB of host disk headroom')
    info = json.loads(command(['podman', 'info', '--format', 'json'], capture=True))
    if info['host']['memTotal'] < (sum(LIMITS.values()) + 768) * 1024 ** 2:
        raise RuntimeError('VM memory below runtime budget')
    if 'memory' not in info['host'].get('cgroupControllers', []):
        raise RuntimeError('Memory controller unavailable')
    for port in [5432, 27017, 4222, 8222, 9200, 8080]:
        with socket.socket() as probe:
            probe.bind(('127.0.0.1', port))
    if sys.platform == 'win32':
        machine = ENV['CONTAINER_CONNECTION']
        output = command(['podman', 'machine', 'ssh', machine,
                          'sysctl -n vm.max_map_count; df -Pk /home/user/.local/share/containers/storage'], capture=True)
        if int(output.splitlines()[0]) < 262144:
            raise RuntimeError('vm.max_map_count below 262144')
        free_kib = int(output.splitlines()[-1].split()[3])
        if free_kib < 8 * 1024 ** 2:
            raise RuntimeError('Insufficient container storage')
    else:
        if int(Path('/proc/sys/vm/max_map_count').read_text()) < 262144:
            raise RuntimeError('vm.max_map_count below 262144')
        free_kib = shutil.disk_usage(info['store']['graphRoot']).free // 1024
        if free_kib < 8 * 1024 ** 2:
            raise RuntimeError('Insufficient container storage')
    images = json.loads(command(['podman', 'image', 'inspect', *IMAGES], capture=True))
    record = {'host_available_mib': available,
              'container_storage_free_gib': round(free_kib / 1024 ** 2, 2),
              'engine_version': info['version']['Version'],
              'client_version': info.get('Client', info['version'])['Version'],
              'compose_provider': command(['podman-compose', '--version'], capture=True).strip(),
              'images': [{'names': x['RepoTags'], 'digests': x['RepoDigests'],
                          'id': x['Id']} for x in images]}
    folder = ROOT / '.yaja/spikes/SP-001'
    folder.mkdir(parents=True, exist_ok=True)
    (folder / 'preflight.json').write_text(json.dumps(record, indent=2), encoding='utf-8')
    print(json.dumps(record, indent=2))


def wait_ready(probe, timeout=120):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            if probe():
                return
        except (OSError, RuntimeError, ValueError) as error:
            last = str(error)
        time.sleep(0.25)
    raise RuntimeError(f'Readiness timeout ({timeout}s): {last}')


def http_json(url, payload=None, method=None):
    request = urllib.request.Request(url, data=json.dumps(payload).encode() if payload else None,
                                     method=method, headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=5) as response:
        return json.load(response)


def setup():
    preflight()
    try:
        start_services(['postgres'])
        postgres = next(x['Id'] for x in containers()
                        if x['Config']['Labels']['io.podman.compose.service'] == 'postgres')
        wait_ready(lambda: command(['podman', 'exec', postgres, 'pg_isready', '-U', 'postgres'], capture=True) is not None)
        start_services(['ferretdb', 'nats', 'opensearch', 'worker', 'api'])
        check_limits()
        wait_ready(lambda: http_json('http://127.0.0.1:8080/health').get('ready'))
        worker = next(x['Id'] for x in containers()
                      if x['Config']['Labels']['io.podman.compose.service'] == 'worker')
        seed = 'sp001-seed-' + str(time.time_ns())
        command(['podman', 'exec', worker, 'python', '/app/service.py', 'seed', seed])
        # A real mapping probe, retained with synthetic values only.
        sql = "SELECT table_name FROM information_schema.tables WHERE table_schema='sp001' ORDER BY table_name"
        tables = command(['podman', 'exec', postgres, 'psql', '-U', 'postgres', '-d', 'yaja', '-Atc', sql], capture=True).split()
        task_tables = [x for x in tables if x.startswith('tasks_') and x.replace('_', '').isalnum()]
        if len(task_tables) != 1:
            raise RuntimeError(f'Unexpected FerretDB task table mapping: {tables}')
        table = task_tables[0]
        row = command(['podman', 'exec', postgres, 'psql', '-U', 'postgres', '-d', 'yaja', '-Atc',
                       f"SELECT _jsonb FROM sp001.{table} WHERE _jsonb->>'_id'='{seed}'"], capture=True)
        document = json.loads(row)
        if not {'_id', 'title', 'status', 'version'} <= set(document) or type(document['version']) is not int:
            raise RuntimeError('FerretDB physical representation differs from the bounded adapter')
        (ROOT / '.yaja/spikes/SP-001/mapping.json').write_text(json.dumps(
            {'table': table, 'row': document}, indent=2), encoding='utf-8')
        print(json.dumps({'observed_task_table': table, 'seed_version': document['version']}), flush=True)
        # FerretDB's JSONB expression index is not a PostgreSQL replica identity.
        # Configure only this experiment's table; never publish FerretDB metadata.
        ddl = f'''ALTER TABLE sp001.{table} REPLICA IDENTITY FULL;
DO $$ BEGIN
  IF EXISTS (SELECT 1 FROM pg_publication WHERE pubname='sp001' AND puballtables) THEN
    DROP PUBLICATION sp001;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_publication WHERE pubname='sp001') THEN
    CREATE PUBLICATION sp001 FOR TABLE sp001.{table};
  END IF;
END $$;'''
        command(['podman', 'exec', postgres, 'psql', '-v', 'ON_ERROR_STOP=1', '-U', 'postgres',
                 '-d', 'yaja', '-c', ddl], capture=True)
        wait_ready(lambda: http_json('http://127.0.0.1:9200/_cluster/health')['status'] in ('green', 'yellow'))
        try:
            http_json('http://127.0.0.1:9200/sp001',
                      {'settings': {'number_of_shards': 1, 'number_of_replicas': 0}}, 'PUT')
        except urllib.error.HTTPError as error:
            body = json.load(error)
            if error.code != 400 or body.get('error', {}).get('type') != 'resource_already_exists_exception':
                raise
        if available_mib() < 1920:
            raise RuntimeError('Need 1920 MiB host available before adding CDC and indexer')
        start_services(['cdc', 'indexer'])
        check_limits()
        wait_ready(lambda: http_json('http://127.0.0.1:8080/search/' + seed).get('version') == 1)
        print('SP001_SETUP_PASSED', flush=True)
    except Exception as error:
        print(f'SETUP_FAILURE: {error}', flush=True)
        try:
            command(COMPOSE + ['logs', '--tail', '15'], timeout=60)
        finally:
            stop_existing()
        raise


def monitored_test():
    observed = check_limits()
    if set(observed) != set(SERVICES):
        raise RuntimeError('All eight containers must exist before cases')
    samples = []
    container_samples = []
    errors = []
    stop = threading.Event()

    def monitor():
        while not stop.is_set():
            try:
                free = available_mib()
                samples.append({'elapsed': round(time.monotonic() - started, 2),
                                'host_available_mib': free})
                if free < 1024:
                    errors.append('Host available RAM below 1024 MiB; stopping experiment')
                    stop_existing()
                    return
            except Exception as error:
                errors.append(str(error))
                return
            stop.wait(1)

    def sample_containers():
        while not stop.is_set():
            try:
                stats = json.loads(command(['podman', 'stats', '--no-stream', '--format', 'json'],
                                           timeout=15, capture=True))
                container_samples.append({'elapsed': round(time.monotonic() - started, 2),
                    'containers': [{'name': x['name'], 'memory': x['mem_usage']}
                                   for x in stats if x['name'].startswith('yaja-sp001_')]})
            except Exception as error:
                errors.append('Container sampling: ' + str(error))
                return
            stop.wait(5)

    started = time.monotonic()
    passed = False
    thread = threading.Thread(target=monitor, daemon=True)
    stats_thread = threading.Thread(target=sample_containers, daemon=True)
    thread.start()
    stats_thread.start()
    try:
        command([sys.executable, str(HERE / 'cases.py')], timeout=600)
        check_limits()
        if errors:
            raise RuntimeError('; '.join(errors))
        passed = True
    finally:
        stop.set()
        thread.join(timeout=125)
        stats_thread.join(timeout=20)
        folder = ROOT / '.yaja/spikes/SP-001'
        folder.mkdir(parents=True, exist_ok=True)
        (folder / 'memory.json').write_text(json.dumps(samples, indent=2), encoding='utf-8')
        (folder / 'container-memory.json').write_text(json.dumps(container_samples, indent=2), encoding='utf-8')
        print(json.dumps({'minimum_host_available_mib': min(
            (x['host_available_mib'] for x in samples), default=None), 'monitor_errors': errors}))
        if not passed:
            try:
                command(COMPOSE + ['logs', '--tail', '20'], timeout=60)
            finally:
                stop_existing()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['setup', 'preflight', 'config', 'pull', 'build', 'up', 'stop',
                                         'status', 'limits', 'test', 'logs'])
    parser.add_argument('services', nargs='*')
    args = parser.parse_args()
    if any(name not in SERVICES for name in args.services):
        parser.error('Unknown service')
    print(json.dumps({'action': args.action, 'host_available_mib': available_mib()}), flush=True)
    if args.action == 'setup':
        setup()
    elif args.action == 'preflight':
        preflight()
    elif args.action == 'config':
        command(COMPOSE + ['config'])
    elif args.action == 'pull':
        for image in IMAGES:
            if available_mib() < 1024:
                raise RuntimeError('Insufficient host headroom during image preparation')
            command(['podman', 'pull', image], timeout=600)
    elif args.action == 'build':
        for containerfile, tag in [('Containerfile', 'localhost/yaja-sp001-probe:experiment'),
                                   ('Containerfile.cdc', 'localhost/yaja-sp001-cdc:experiment')]:
            command(['podman', 'build', '--memory', '768m', '--memory-swap', '768m',
                     '-f', containerfile, '-t', tag, '.'], timeout=600)
    elif args.action == 'up':
        names = args.services or SERVICES
        existing = {x['Config']['Labels']['io.podman.compose.service'] for x in containers()
                    if x['State']['Running']}
        needed = sum(LIMITS[x] for x in names if x not in existing) + 1024
        if available_mib() < needed:
            raise RuntimeError(f'Need {needed} MiB available before this stage')
        start_services(names)
        check_limits()
    elif args.action == 'stop':
        if args.services:
            command(COMPOSE + ['stop', *args.services])
        else:
            stop_existing()
    elif args.action == 'status':
        command(COMPOSE + ['ps'])
        command(['podman', 'stats', '--no-stream', '--format', 'json'])
    elif args.action == 'limits':
        check_limits()
    elif args.action == 'test':
        monitored_test()
    elif args.action == 'logs':
        command(COMPOSE + ['logs', '--tail', '60', *(args.services or SERVICES)])


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f'SP001_FAILED: {error}', file=sys.stderr)
        sys.exit(1)
