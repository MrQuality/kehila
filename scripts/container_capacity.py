#!/usr/bin/env python3
"""Run an explicit finite capacity batch using the canonical owned native fixture."""
import argparse
import json
import os
from pathlib import Path
import sys
import time

from capacity_plan import select
from cleanup_errors import run_main
from container_executor import ContainerExecutor, image_reference
from container_native import check
from native_authority_fixture import NATIVE_TEST_COMMAND
from test_services import managed_runtime, validate_daemon_environment, validate_runtime
from verification_budget import validate_seconds, work_budget
from verification_settings import load

ROOT = Path(__file__).resolve().parents[1]


def write_new(path, value):
    with path.open('x', encoding='utf-8') as stream:
        json.dump(value, stream, sort_keys=True, indent=2)


def admit_output(path, source=ROOT):
    if (not path.is_absolute() or path.resolve() != path or path.exists()
            or not path.parent.is_dir() or source == path or source in path.parents
            or path in source.parents
            or any((parent / '.git').exists() for parent in (path, *path.parents))):
        raise ValueError('Capacity batch requires a new absolute output directory outside source and Git')
    return path


def selection(indices, observer, calibration):
    selected = select(indices)
    if (type(calibration) is not bool or observer not in {'off','stages','sampled'}
            or (calibration and (len(selected) != 1 or selected[0].index != 15 or observer != 'sampled'))):
        raise ValueError('Known-blocker calibration requires only sampled serial-edit cell 15')
    return selected


def observation_jobs(selected, observer, compare):
    if type(compare) is not bool:
        raise ValueError('Observer comparison selector must be boolean')
    modes = ('off','stages','sampled')
    jobs = []
    for entry in selected:
        offset = entry.repetition - 1
        order = modes[offset:] + modes[:offset] if compare else (observer,)
        jobs.extend((entry, mode) for mode in order)
    return tuple(jobs)


def batch(executor, selected, observer, output, *, oracle_control=False, calibration=False, compare=False):
    """Compile once; retain each complete receipt before starting another cell."""
    selected = selection([entry.index for entry in selected], observer, calibration)
    if compare and calibration:
        raise ValueError('Known-blocker calibration is separate from observer comparisons')
    jobs = observation_jobs(selected, observer, compare)
    job_plan = [{'cell_index': entry.index, 'observer': mode} for entry, mode in jobs]
    write_new(output / 'batch-plan.json', {'schema': 1, 'observer': observer, 'calibration': calibration,
              'compare_observers': compare, 'jobs': job_plan,
              'cells': [entry.document() for entry in selected],
              'oracle_control_first_cell': oracle_control})
    executor.execute([*NATIVE_TEST_COMMAND, '--no-run'])
    executor.finish_builds()
    for offset, (entry, mode) in enumerate(jobs):
        label = ('calibration-' if calibration else '') + f'cell-{entry.index:03d}' + ('-' + mode if compare else '')
        started = time.monotonic_ns()
        receipt = check(executor, capacity_cell=entry.index, capacity_observer=mode,
                        capacity_oracle_control=oracle_control and offset == 0,
                        capacity_calibration=calibration,
                        evidence_directory=output / label, reset_assets=True)
        write_new(output / (label + '-receipt.json'), receipt)
        write_new(output / (label + '-controller.json'),
                  {'schema': 1, 'cell_index': entry.index,
                   'seconds_through_export_and_reset': (time.monotonic_ns() - started) / 1e9})
        executor.verify_source()
    write_new(output / 'batch-complete.json', {'schema': 1, 'cells': [entry.index for entry in selected],
              'observer': observer, 'calibration': calibration, 'compare_observers': compare,
              'jobs': job_plan, 'source_unchanged': True})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cell', type=int, action='append', required=True,
                        help='Explicit index 0..179; repeat for a finite batch')
    parser.add_argument('--observer', choices=['off', 'stages', 'sampled'], default='sampled')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--oracle-control', action='store_true',
                        help='Also reject an altered final-state expectation in the first cell')
    special = parser.add_mutually_exclusive_group()
    special.add_argument('--calibration', action='store_true',
                        help='Separate known actor/project blocker control; requires only cell 15, sampled')
    special.add_argument('--compare-observers', action='store_true',
                         help='Run off/stages/sampled for each selected cell, rotating order by repetition')
    parser.add_argument('--timeout', type=float)
    parser.add_argument('--image', default=os.environ.get('KEHILA_VERIFICATION_IMAGE'))
    parser.add_argument('--podman-machine', default=os.environ.get('KEHILA_TEST_PODMAN_MACHINE') or None)
    exclusive = os.environ.get('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE', '0')
    if exclusive not in {'0', '1'}:
        parser.error('KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE must be 0 or 1')
    parser.add_argument('--exclusive-podman-machine', action='store_true', default=exclusive == '1')
    args = parser.parse_args()
    selected = selection(args.cell, args.observer, args.calibration)
    output = admit_output(args.output)
    if (ROOT / '.git').exists() or (ROOT / 'AGENTS.md').exists():
        raise ValueError('Container capacity requires an exported public source snapshot')
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
                batch(executor, selected, args.observer, output, oracle_control=args.oracle_control,
                      calibration=args.calibration, compare=args.compare_observers)
    write_new(output / 'batch-cleaned.json', {'schema': 1, 'owned_cleanup_completed': True})
    print('CAPACITY BATCH: finite observations and independent state witnesses retained', flush=True)


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_CONTAINER_CAPACITY'))
