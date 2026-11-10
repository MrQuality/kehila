#!/usr/bin/env python3
"""Run an explicitly selected finite paired storage sample batch with Podman."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import time

from cleanup_errors import run_main
from container_capacity import admit_output, write_new
from container_executor import ContainerExecutor, image_reference
from container_native import check
from native_authority_fixture import NATIVE_TEST_COMMAND, STORAGE_MODE
from native_fixture_assets import verify_export
from storage_growth_plan import select
from storage_growth_results import require, validate_sample_observations
from native_database_limits import storage_limits
from test_services import managed_runtime, validate_daemon_environment, validate_runtime
from verification_budget import validate_seconds, work_budget
from verification_settings import load

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE_LIMIT = 2 * 1024**3
SAMPLE_EVIDENCE_RESERVE = 512 * 1024**2
DISK_RESERVE = 2 * 1024**3


def read_verified_sample(output, receipt, sample):
    require(receipt.get('schema') == 1 and receipt.get('storage_sample_index') == sample.index
            and receipt.get('selected_items') == [STORAGE_MODE]
            and receipt.get('independent_state_verified') is True
            and receipt.get('reset_completed') is True, 'Storage sample receipt is incomplete')
    owner = json.loads((output / '.fixture-owner.json').read_text(encoding='utf-8'))['owner']
    verify_export(output, owner, receipt['evidence_sha256'])
    validate_sample_observations(output / 'storage', output / (STORAGE_MODE + '-observation.txt'),
                                 output / 'storage-historical-observation.txt', sample)
    source = hashlib.sha256((output / 'source-sha256.json').read_bytes()).hexdigest()
    profile = json.loads((output / 'execution-profile.json').read_text(encoding='utf-8'))
    environment = {key: profile[key] for key in (
        'schema', 'runner_image', 'runner_memory_bytes', 'runner_pids', 'runner_nano_cpus',
        'runner_cpu_period', 'runner_cpu_quota', 'database_image', 'database_memory_bytes',
        'database_pids', 'database_nano_cpus', 'database_cpu_period', 'database_cpu_quota', 'database_tmpfs')}
    return {'source_inventory_sha256': source, 'execution_profile': environment}


def preflight(executor, output):
    allocated = sum(path.stat().st_size for path in output.rglob('*') if path.is_file())
    require(allocated + SAMPLE_EVIDENCE_RESERVE <= EVIDENCE_LIMIT, 'Storage evidence budget exhausted')
    require(shutil.disk_usage(output).free >= DISK_RESERVE + SAMPLE_EVIDENCE_RESERVE,
            'Storage external evidence reserve unavailable')
    probe = executor.engine('exec', executor.container, 'python', '-c',
                            "import shutil; print(shutil.disk_usage('/work').free)", capture=True)
    available = int(probe.stdout.strip())
    require(available >= DISK_RESERVE + SAMPLE_EVIDENCE_RESERVE,
            'Storage runner/provider disk reserve unavailable')
    return {'external_evidence_bytes': allocated, 'provider_free_bytes': available,
            'external_free_bytes': shutil.disk_usage(output).free}


def batch(executor, selected, output, *, storage_profile='default'):
    selected = select([sample.index for sample in selected])
    limits = [storage_limits(sample.index, storage_profile) for sample in selected]
    require(all(value == limits[0] for value in limits), 'Storage batch resource limits differ')
    write_new(output / 'batch-plan.json', {'schema': 1, 'samples': [sample.document() for sample in selected],
              'maximum_item_revisions': sum(sample.revisions for sample in selected),
              'measurement_seconds_limit': 7200, 'evidence_bytes_limit': EVIDENCE_LIMIT,
              'sample_evidence_reserve_bytes': SAMPLE_EVIDENCE_RESERVE, 'disk_reserve_bytes': DISK_RESERVE,
              'storage_profile': storage_profile, 'database_tmpfs_bytes': limits[0].memory_bytes,
              'database_memory_bytes': limits[0].memory_bytes, 'database_stop_margin_bytes': 64 * 1024**2})
    preflight(executor, output)
    executor.execute(['cargo', 'fmt', '--all', '--', '--check'])
    executor.execute([*NATIVE_TEST_COMMAND, '--no-run'])
    executor.finish_builds()
    started = time.monotonic()
    baseline = None
    for sample in selected:
        require(time.monotonic() - started < 7200, 'Storage campaign measurement allowance expired')
        name = f'sample-{sample.index:02d}'
        write_new(output / (name + '-preflight.json'), preflight(executor, output))
        before = time.monotonic()
        receipt = check(executor, storage_sample=sample.index,
                        evidence_directory=output / name, reset_assets=True, storage_profile=storage_profile)
        write_new(output / (name + '-receipt.json'), receipt)
        binding = read_verified_sample(output / name, receipt, sample)
        require(sum(path.stat().st_size for path in output.rglob('*') if path.is_file()) <= EVIDENCE_LIMIT,
                'Storage exported evidence exceeds campaign budget')
        if baseline is None:
            baseline = binding
        require(binding == baseline, 'Storage batch source or execution profile changed')
        write_new(output / (name + '-controller.json'),
                  {'schema': 1, 'sample': sample.index, 'seconds_through_export_and_reset': time.monotonic()-before})
        executor.verify_source()
    require(time.monotonic() - started <= 7200, 'Storage campaign measurement allowance expired')
    write_new(output / 'batch-complete.json', {'schema': 1, 'samples': [sample.index for sample in selected],
              'binding': baseline, 'source_unchanged': True, 'measurement_seconds': time.monotonic()-started})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sample', type=int, action='append', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--timeout', type=float)
    parser.add_argument('--storage-profile', choices=('default', 'extended'), default='default')
    parser.add_argument('--image', default=os.environ.get('KEHILA_VERIFICATION_IMAGE'))
    parser.add_argument('--podman-machine', default=os.environ.get('KEHILA_TEST_PODMAN_MACHINE') or None)
    exclusive = os.environ.get('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE', '0')
    if exclusive not in {'0', '1'}:
        parser.error('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE must be 0 or 1')
    parser.add_argument('--exclusive-podman-machine', action='store_true', default=exclusive == '1')
    args = parser.parse_args()
    selected = select(args.sample)
    for sample in selected:
        storage_limits(sample.index, args.storage_profile)
    output = admit_output(args.output)
    if (ROOT / '.git').exists() or (ROOT / 'AGENTS.md').exists():
        raise ValueError('Storage campaign requires an exported public source snapshot')
    validate_runtime('podman', args.podman_machine, args.exclusive_podman_machine)
    validate_daemon_environment(os.environ, args.podman_machine)
    settings = load()
    timeout = settings['gate_seconds'] if args.timeout is None else args.timeout
    validate_seconds(timeout, maximum=settings['gate_seconds'])
    image = image_reference(args.image or settings['runner_image'])
    output.mkdir(exist_ok=False)
    with work_budget(timeout, maximum=settings['gate_seconds']):
        with managed_runtime('podman', args.podman_machine, environment=os.environ,
                             exclusive=args.exclusive_podman_machine) as runtime:
            with ContainerExecutor(runtime, settings, image, ROOT, dependencies=True).owned() as executor:
                executor.execute(['python', 'scripts/container_dependencies.py', 'check', executor.cache.key])
                batch(executor, selected, output, storage_profile=args.storage_profile)
    write_new(output / 'batch-cleaned.json', {'schema': 1, 'owned_cleanup_completed': True})
    print('STORAGE BATCH: finite paired observations and independent state witnesses retained', flush=True)


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_CONTAINER_STORAGE_GROWTH'))
