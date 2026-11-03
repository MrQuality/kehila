#!/usr/bin/env python3
"""Host lifecycle coordinator for the unchanged matrix in a Linux container."""
import argparse
import json
import os
from pathlib import Path
import sys

from cleanup_errors import run_main
from container_executor import ContainerExecutor, image_reference
from test_services import managed_runtime, service_group, validate_daemon_environment, validate_runtime
from verification_budget import validate_seconds, work_budget
from verification_settings import load
from verify import check_commands
from container_native import check as native_authority
import time

ROOT = Path(__file__).resolve().parents[1]


def prepare_dependencies(executor, seconds):
    key = executor.cache.key
    executor.execute(['python', 'scripts/container_dependencies.py', 'clear', key])
    executor.execute(['cargo', 'fetch', '--locked', '--target', 'x86_64-unknown-linux-gnu'],
                     limit=seconds)
    executor.execute(['go', 'mod', 'download', 'all'], limit=seconds)
    executor.verify_source()
    executor.execute(['python', 'scripts/container_dependencies.py', 'mark', key])


def checks(executor, args):
    if getattr(args, 'native_authority', False):
        executor.execute(['cargo', 'test', '--locked', '-p', 'kehila_authority', '--lib', '--no-run'])
        executor.finish_builds()
        native_authority(executor)
        executor.verify_source()
        return
    for command in check_commands(args):
        executor.execute(command)
    if args.docs or args.pure:
        return
    executor.execute([sys.executable, 'tests/integration/task_path.py', '--build-only', '/work/task-path'])
    executor.finish_builds()
    native_authority(executor)
    executor.verify_source()
    with service_group(executor.runtime, ['nats']) as containers:
        with executor.service_network(containers) as addresses:
            host = addresses['nats']
            environment = {'KEHILA_NATS_HOST': host, 'KEHILA_NATS_PORT': '4222',
                           'KEHILA_NATS_ADDRESS': host + ':4222'}
            executor.execute([sys.executable, 'scripts/healthcheck.py', '--host', host,
                              '--wait', '180', '--services', 'NATS'])
            executor.execute([sys.executable, 'tests/integration/nats_probe.py'], environment)
            executor.execute(['cargo', 'test', '--locked', '-p', 'event_dispatcher'], environment)
    with service_group(executor.runtime, ['opensearch']) as containers:
        with executor.service_network(containers) as addresses:
            executor.execute([sys.executable, 'scripts/healthcheck.py', '--host', addresses['opensearch'],
                              '--wait', '180', '--services', 'OpenSearch'])
    with service_group(executor.runtime, ['postgres', 'ferretdb']) as containers:
        with executor.service_network(containers) as addresses:
            for service, name in (('postgres', 'PostgreSQL'), ('ferretdb', 'FerretDB')):
                executor.execute([sys.executable, 'scripts/healthcheck.py', '--host', addresses[service],
                                  '--wait', '180', '--services', name])
            executor.execute([sys.executable, 'tests/integration/task_path.py', '--run-only', '/work/task-path'],
                             {'KEHILA_TEST_MONGO_URL': 'mongodb://' + addresses['ferretdb'] + ':27017'})
    executor.verify_source()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--docs', action='store_true')
    modes.add_argument('--pure', action='store_true')
    modes.add_argument('--prepare-dependencies', action='store_true')
    modes.add_argument('--native-authority', action='store_true',
                       help='Only compile and run the real native PostgreSQL/HTTP/TLS product fixture')
    parser.add_argument('--timeout', type=float)
    parser.add_argument('--ownership-file', type=Path,
                        default=os.environ.get('KEHILA_VERIFICATION_OWNERSHIP_FILE'),
                        help='Optional new external host-control record')
    parser.add_argument('--image', default=os.environ.get('KEHILA_VERIFICATION_IMAGE'))
    parser.add_argument('--engine', choices=['podman'], default='podman')
    parser.add_argument('--podman-machine', default=os.environ.get('KEHILA_TEST_PODMAN_MACHINE') or None)
    exclusive = os.environ.get('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE', '0')
    if exclusive not in {'0', '1'}:
        parser.error('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE must be 0 or 1')
    parser.add_argument('--exclusive-podman-machine', action='store_true', default=exclusive == '1')
    args = parser.parse_args()
    validate_runtime(args.engine, args.podman_machine, args.exclusive_podman_machine)
    validate_daemon_environment(os.environ, args.podman_machine)
    settings = load()
    maximum = settings['dependency_prepare_seconds'] if args.prepare_dependencies else settings['gate_seconds']
    args.timeout = maximum if args.timeout is None else args.timeout
    validate_seconds(args.timeout, maximum=maximum)
    args.image = image_reference(args.image or settings['runner_image'])
    if (ROOT / '.git').exists() or (ROOT / 'AGENTS.md').exists():
        raise ValueError('Container verification requires the hook-exported public source snapshot')
    if args.ownership_file is not None:
        record = args.ownership_file
        if not record.is_absolute() or ROOT == record.resolve() or ROOT in record.resolve().parents:
            raise ValueError('Ownership record must be a new absolute external file')
        if record.exists():
            raise ValueError('Ownership record already exists')
    started = time.monotonic()
    with work_budget(args.timeout, maximum=maximum):
        with managed_runtime(args.engine, args.podman_machine, environment=os.environ,
                             exclusive=args.exclusive_podman_machine) as runtime:
            executor = ContainerExecutor(runtime, settings, args.image, ROOT,
                                         dependencies=not args.docs,
                                         preparing=args.prepare_dependencies)
            if args.ownership_file is not None:
                # Only the host coordinator writes this; workloads cannot access it.
                with args.ownership_file.open('x', encoding='utf-8') as record:
                    json.dump({'owner': executor.owner}, record)
            with executor.owned():
                if args.prepare_dependencies:
                    prepare_dependencies(executor, maximum)
                else:
                    if executor.cache is not None:
                        executor.execute(['python', 'scripts/container_dependencies.py',
                                          'check', executor.cache.key])
                    executor.finish_builds()
                    checks(executor, args)
    print('CONTAINER PHASE SECONDS:', time.monotonic() - started, flush=True)


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_CONTAINER_VERIFICATION'))
