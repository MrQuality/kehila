"""Versioned verifier defaults with explicit, external machine overrides."""
import json
import math
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
KEYS = {'machine_start_seconds', 'machine_stop_seconds', 'runner_memory_mib',
        'runner_cpus', 'runner_pids', 'runner_stop_seconds', 'runner_image',
        'image_prepare_seconds', 'host_available_memory_floor_gib',
        'host_aggregate_memory_budget_gib', 'gate_seconds', 'command_seconds',
        'dependency_prepare_seconds', 'host_available_memory_warning_gib'}


def immutable_image(value):
    if not isinstance(value, str) or not re.fullmatch(r'(?:[a-zA-Z0-9./:_-]+@)?sha256:[0-9a-f]{64}', value):
        raise ValueError('Container verification requires an explicit immutable toolchain image; '
                         'set runner_image or KEHILA_VERIFICATION_IMAGE. See '
                         'docs/implementation/container-verification.md#image-preparation-and-invocation')
    return value


def _object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('Duplicate verification setting: ' + key)
        result[key] = value
    return result


def _read(path):
    values = json.loads(path.read_text(encoding='utf-8-sig'), object_pairs_hook=_object)
    if not isinstance(values, dict) or not set(values) <= KEYS:
        raise ValueError('Expected known verification settings')
    for key, value in values.items():
        if key == 'runner_image':
            if value is not None:
                immutable_image(value)
            continue
        if key in {'host_available_memory_floor_gib', 'host_available_memory_warning_gib',
                   'host_aggregate_memory_budget_gib'} and value is None:
            continue
        if (type(value) not in (int, float) or not math.isfinite(value) or value <= 0):
            raise ValueError('Verification setting must be finite and positive: ' + key)
    for key in ('runner_memory_mib', 'runner_pids'):
        if key in values and type(values[key]) is not int:
            raise ValueError('Verification resource limit must be an integer: ' + key)
    return values


def load(environment=None):
    environment = os.environ if environment is None else environment
    values = _read(ROOT / 'config/verification-defaults.json')
    if set(values) != KEYS:
        raise ValueError('Incomplete upstream verification defaults')
    override = environment.get('KEHILA_VERIFICATION_CONFIG')
    if override:
        if environment.get('CI', '').lower() not in {'', '0', 'false'}:
            raise ValueError('Machine verification overrides are forbidden in CI')
        path = Path(override)
        if not path.is_absolute():
            raise ValueError('Machine verification overrides require an absolute external path')
        path = path.resolve()
        if path == ROOT or ROOT in path.parents:
            raise ValueError('Machine verification overrides must be outside the checkout')
        values.update(_read(path))
    return values
