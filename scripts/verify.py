#!/usr/bin/env python3
"""The shared local/CI verification matrix; missing tools and skipped I/O fail closed."""
from pathlib import Path
import argparse
import os
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from temp_work import owned_directory
from test_services import managed_runtime, service_group, validate_runtime
from verification_budget import command_timeout, validate_seconds, work_budget

ROOT = Path(__file__).resolve().parents[1]


def execute(command, environment):
    print('VERIFY:', ' '.join(command), flush=True)
    timeout = command_timeout(450 if os.name == 'nt' and command[0] == 'cargo' else 300)
    subprocess.run(command, cwd=ROOT, env=environment, check=True, timeout=timeout)


def live_checks(folder, environment, args):
    def run(command):
        execute(command, environment)

    def health(*names):
        run([sys.executable, 'scripts/healthcheck.py', '--wait', '180', '--services', *names])

    workspace = ['cargo', 'test', '--locked', '--workspace']
    task_path = [sys.executable, 'tests/integration/task_path.py', '--run-only', str(folder)]
    if args.external_services:
        health('PostgreSQL', 'FerretDB', 'NATS', 'OpenSearch')
        run([sys.executable, 'tests/integration/nats_probe.py'])
        run(workspace)
        run(task_path)
        return
    with managed_runtime(args.engine, args.podman_machine, exclusive=args.exclusive_podman_machine) as compose:
        with service_group(compose, ['nats']):
            health('NATS')
            run([sys.executable, 'tests/integration/nats_probe.py'])
            run(workspace)
        with service_group(compose, ['opensearch']):
            health('OpenSearch')
        with service_group(compose, ['postgres', 'ferretdb']):
            health('PostgreSQL', 'FerretDB')
            run(task_path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--pure", action="store_true", help="Run unit tests without external services")
    modes.add_argument("--docs", action="store_true", help="Run Python, naming and engineering metadata checks")
    parser.add_argument('--external-services', action='store_true',
                        help='Use already-running disposable services without owning their lifecycle')
    parser.add_argument('--engine', choices=('auto', 'podman', 'docker'),
                        default=os.environ.get('KEHILA_TEST_ENGINE', 'auto'))
    parser.add_argument('--podman-machine', default=os.environ.get('KEHILA_TEST_PODMAN_MACHINE') or None,
                        help='Start this explicit machine after builds; retain shared machines')
    exclusive = os.environ.get('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE', '0')
    if exclusive not in {'0', '1'}:
        parser.error('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE must be 0 or 1')
    parser.add_argument('--exclusive-podman-machine', action='store_true', default=exclusive == '1',
                        help='Promise exclusive VM use for this run; requires an initially stopped named machine')
    parser.add_argument('--timeout', type=float, default=600,
                        help='Total work budget in seconds (at most 600); bounded cleanup runs afterward')
    args = parser.parse_args()
    if args.external_services and (args.pure or args.docs):
        parser.error('--external-services applies only to full verification')
    if args.external_services and (args.podman_machine is not None or args.exclusive_podman_machine):
        parser.error('--external-services cannot own a Podman machine')
    try:
        validate_runtime(args.engine, args.podman_machine, args.exclusive_podman_machine)
        validate_seconds(args.timeout)
    except ValueError as error:
        parser.error(str(error))
    with work_budget(args.timeout):
        run_checks(args)


def run_checks(args):
    environment = dict(os.environ, CARGO_BUILD_JOBS='1')
    environment['GOFLAGS'] = (environment.get('GOFLAGS', '') + ' -p=1').strip()
    commands = [[sys.executable, "-m", "unittest", "discover", "-s", "tests", "-v"],
                [sys.executable, "scripts/check_branding.py"],
                [sys.executable, "scripts/check_engineering.py"]]
    if not args.docs:
        commands.append([sys.executable, "scripts/go_static.py"])
        commands += [["cargo", "test", "--locked", "-p", "kehila_query", "-p", "task_contract", "-p", "project_create_codec"] if args.pure
                     else ["cargo", "test", "--locked", "--workspace", "--no-run"],
                     [sys.executable, "experiments/SP-002/check_seed_oracle.py"],
                     [sys.executable, "experiments/SP-003/check.py"],
                     [sys.executable, "scripts/go_test.py"]]
    for command in commands:
        execute(command, environment)
    if not args.pure and not args.docs:
        with owned_directory('kehila-verify-') as folder:
            execute([sys.executable, 'tests/integration/task_path.py', '--build-only', str(folder)],
                    environment)
            live_checks(folder, environment, args)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        print(f"ERR_TEST_EXECUTION_FAILED: {error}", file=sys.stderr)
        sys.exit(3)
