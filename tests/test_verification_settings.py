"""Defaults and explicit machine overrides must remain finite and validated."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import verification_settings as settings


class SettingsTests(unittest.TestCase):
    def test_upstream_defaults(self):
        self.assertEqual(settings.load({}), json.loads(
            (settings.ROOT / 'config/verification-defaults.json').read_text()))

    def test_explicit_external_override_merges(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'settings.json'
            path.write_text(json.dumps({'machine_start_seconds': 300}))
            result = settings.load({'KEHILA_VERIFICATION_CONFIG': str(path)})
        self.assertEqual(result['machine_start_seconds'], 300)
        self.assertEqual(result['machine_stop_seconds'], 120)
        self.assertEqual(result['runner_memory_mib'], 2048)

    def test_invalid_configuration_fails_closed(self):
        for content in ('{"typo": 3}', '{"machine_start_seconds": true}',
                        '{"machine_start_seconds": 0}', '{"machine_start_seconds": -1}',
                        '{"machine_start_seconds": NaN}', '{"runner_image": "python:latest"}',
                        '{"runner_pids": 1.5}', '[]',
                        '{"machine_start_seconds": 2, "machine_start_seconds": 3}'):
            with self.subTest(content=content), tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / 'settings.json'
                path.write_text(content)
                with self.assertRaises(ValueError):
                    settings.load({'KEHILA_VERIFICATION_CONFIG': str(path)})

    def test_local_override_is_rejected_in_ci(self):
        with self.assertRaises(ValueError):
            settings.load({'CI': 'true', 'KEHILA_VERIFICATION_CONFIG': 'unused'})

    def test_missing_override_is_not_silently_ignored(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaises(OSError):
                settings.load({'KEHILA_VERIFICATION_CONFIG': str(Path(folder) / 'missing')})

    def test_relative_and_checkout_local_overrides_are_rejected(self):
        for path in ('relative.json', str(settings.ROOT / 'local-settings.json')):
            with self.subTest(path=path), self.assertRaises(ValueError):
                settings.load({'KEHILA_VERIFICATION_CONFIG': path})

    def test_host_memory_policy_is_machine_local_and_optional_upstream(self):
        defaults = settings.load({})
        self.assertIsNone(defaults['host_available_memory_floor_gib'])
        self.assertIsNone(defaults['host_available_memory_warning_gib'])
        self.assertIsNone(defaults['host_aggregate_memory_budget_gib'])
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'settings.json'
            path.write_text(json.dumps({'host_available_memory_floor_gib': None,
                                        'host_available_memory_warning_gib': 2.5,
                                        'host_aggregate_memory_budget_gib': 4}))
            values = settings.load({'KEHILA_VERIFICATION_CONFIG': str(path)})
        self.assertIsNone(values['host_available_memory_floor_gib'])
        self.assertEqual(values['host_available_memory_warning_gib'], 2.5)
        self.assertEqual(values['host_aggregate_memory_budget_gib'], 4)

    def test_invalid_host_memory_policy_is_rejected(self):
        for value in (True, 0, -1, float('inf'), float('nan'), 'unlimited'):
            with self.subTest(value=value), tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / 'settings.json'
                path.write_text(json.dumps({'host_aggregate_memory_budget_gib': value}))
                with self.assertRaises(ValueError):
                    settings.load({'KEHILA_VERIFICATION_CONFIG': str(path)})
