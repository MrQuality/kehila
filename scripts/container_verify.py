#!/usr/bin/env python3
"""Host lifecycle coordinator for the unchanged matrix in a Linux container."""
import argparse
import os
from pathlib import Path
import sys

from cleanup_errors import run_main
from container_executor import ContainerExecutor, image_reference
from test_services import managed_runtime, service_group, validate_daemon_environment, validate_runtime
from verification_budget import validate_seconds, work_budget
from verification_settings import load
from verify import check_commands
from container_native import check as native_authority, selected_item_cases
from native_authority_fixture import NATIVE_TEST_COMMAND, ITEM_DATABASES
import time
from verification_ownership import GROUPS, ServiceOwnership

ROOT = Path(__file__).resolve().parents[1]


def prepare_dependencies(executor, seconds):
    key = executor.cache.key
    executor.execute(['python', 'scripts/container_dependencies.py', 'clear', key])
    executor.execute(['cargo', 'fetch', '--locked', '--target', 'x86_64-unknown-linux-gnu'],
                     limit=seconds)
    executor.execute(['go', 'mod', 'download', 'all'], limit=seconds)
    executor.verify_source()
    executor.execute(['python', 'scripts/container_dependencies.py', 'mark', key])


def checks(executor, args, ownership=None):
    def services(selected):
        if ownership is None:
            return service_group(executor.runtime, selected)
        return service_group(executor.runtime, selected, ownership=ownership)

    item_cases = getattr(args, 'native_item', None)
    if getattr(args, 'native_authority', False) or item_cases is not None:
        executor.execute([*NATIVE_TEST_COMMAND, '--no-run'])
        executor.finish_builds()
        if item_cases is None:
            native_authority(executor)
        else:
            native_authority(executor, item_cases=item_cases)
        executor.verify_source()
        return
    for command in check_commands(args):
        executor.execute(command)
    if args.docs or args.pure or getattr(args, 'development', False):
        return
    executor.execute([sys.executable, 'tests/integration/task_path.py', '--build-only', '/work/task-path'])
    executor.execute([*NATIVE_TEST_COMMAND, '--no-run'])
    executor.finish_builds()
    native_authority(executor)
    executor.verify_source()
    with services(['nats']) as containers:
        with executor.service_network(containers) as addresses:
            host = addresses['nats']
            environment = {'KEHILA_NATS_HOST': host, 'KEHILA_NATS_PORT': '4222',
                           'KEHILA_NATS_ADDRESS': host + ':4222'}
            executor.execute([sys.executable, 'scripts/healthcheck.py', '--host', host,
                              '--wait', '180', '--services', 'NATS'])
            executor.execute([sys.executable, 'tests/integration/nats_probe.py'], environment)
            executor.execute(['cargo', 'test', '--locked', '-p', 'event_dispatcher'], environment)
    with services(['opensearch']) as containers:
        with executor.service_network(containers) as addresses:
            executor.execute([sys.executable, 'scripts/healthcheck.py', '--host', addresses['opensearch'],
                              '--wait', '180', '--services', 'OpenSearch'])
    with services(['postgres', 'ferretdb']) as containers:
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
    modes.add_argument('--development', action='store_true',
                       help='Run shared unit/tooling/page checks without provisioning external services')
    modes.add_argument('--prepare-dependencies', action='store_true')
    modes.add_argument('--native-authority', action='store_true',
                       help='Only compile and run the real native PostgreSQL/HTTP/TLS product fixture')
    modes.add_argument('--native-item', action='append', choices=tuple(ITEM_DATABASES),
                       help='Run a focused WorkItem case with its independent observer; repeat for multiple cases')
    parser.add_argument('--timeout', type=float)
    parser.add_argument('--ownership-file', type=Path,
                        default=os.environ.get('KEHILA_VERIFICATION_OWNERSHIP_FILE'),
                        help='Optional new external host-control record')
    parser.add_argument('--resource-owner', default=os.environ.get('KEHILA_VERIFICATION_RESOURCE_OWNER'),
                        help='Optional fresh caller-known 128-bit lowercase hex resource owner')
    parser.add_argument('--image', default=os.environ.get('KEHILA_VERIFICATION_IMAGE'))
    parser.add_argument('--engine', choices=['podman'], default='podman')
    parser.add_argument('--podman-machine', default=os.environ.get('KEHILA_TEST_PODMAN_MACHINE') or None)
    exclusive = os.environ.get('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE', '0')
    if exclusive not in {'0', '1'}:
        parser.error('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE must be 0 or 1')
    parser.add_argument('--exclusive-podman-machine', action='store_true', default=exclusive == '1')
    args = parser.parse_args()
    try:
        args.native_item = selected_item_cases(args.native_item)
    except ValueError as error:
        parser.error(str(error))
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
                                         owner=args.resource_owner,
                                         dependencies=not args.docs,
                                         preparing=args.prepare_dependencies)
            ownership = None
            if args.ownership_file is not None:
                # Reserve every applicable service group before any creation.
                # The external host record is never copied into the runner.
                full = not (args.docs or args.pure or args.development or args.prepare_dependencies
                            or args.native_authority or args.native_item is not None)
                ownership = ServiceOwnership.create(args.ownership_file, runtime, ROOT,
                                                    executor.owner, GROUPS if full else ())
            with executor.owned():
                if args.prepare_dependencies:
                    prepare_dependencies(executor, maximum)
                else:
                    if executor.cache is not None:
                        executor.execute(['python', 'scripts/container_dependencies.py',
                                          'check', executor.cache.key])
                    executor.finish_builds()
                    checks(executor, args, ownership)
    print('CONTAINER PHASE SECONDS:', time.monotonic() - started, flush=True)


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_CONTAINER_VERIFICATION'))
