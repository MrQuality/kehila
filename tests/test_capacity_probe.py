"""Collector oracles; these checks do not execute backend or overlay services."""
import importlib.util
from contextlib import redirect_stdout
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

SPEC = importlib.util.spec_from_file_location('capacity_probe',
    Path(__file__).resolve().parents[1] / 'experiments/SP-007/capacity_probe.py')
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


class CapacityProbeTests(unittest.TestCase):
    def test_workflow_argument_order_keeps_workload_flags_opaque(self):
        outside = Path(tempfile.gettempdir()) / 'capacity-argument-check'
        args, command, output = probe.parse_arguments(['image', '--output', str(outside),
            '--storage', tempfile.gettempdir(), '--', 'podman', 'build', '--memory=2g'])
        self.assertEqual(args.phase, 'image')
        self.assertEqual(command, ['podman', 'build', '--memory=2g'])
        self.assertEqual(output, outside.resolve())

    def test_linux_memory_uses_available_including_reclaimable_memory(self):
        self.assertEqual(probe.memory_bytes(
            'MemTotal: 1000 kB\nMemFree: 10 kB\nMemAvailable: 600 kB\n'), (1024000, 614400))

    def test_missing_available_or_wrong_units_cannot_be_reported_as_zero(self):
        with self.assertRaises(KeyError):
            probe.memory_bytes('MemTotal: 1000 kB\n')
        with self.assertRaises(ValueError):
            probe.memory_bytes('MemTotal: 1000 MB\nMemAvailable: 600 kB\n')

    def test_impossible_memory_values_fail_closed(self):
        for text in ('MemTotal: 10 kB\nMemAvailable: 11 kB\n',
                     'MemTotal: 10 kB\nMemAvailable: -1 kB\n'):
            with self.assertRaises(ValueError):
                probe.memory_bytes(text)

    def test_sampling_failure_cannot_turn_child_success_into_pass(self):
        self.assertEqual(probe.qualified_exit(0, False), 0)
        self.assertEqual(probe.qualified_exit(0, True), 1)

    def test_child_failure_survives_sampler_failure(self):
        self.assertEqual(probe.qualified_exit(23, True), 23)
        self.assertEqual(probe.qualified_exit(23, False), 23)

    def test_raw_child_output_stays_private_when_final_sample_fails(self):
        def sample(samples):
            if samples.count:
                raise OSError('synthetic measurement failure')
            samples.total = 100
            samples.minimum_available = 50
            samples.minimum_disk = 1000
            samples.count = 1

        def child(_command, **kwargs):
            kwargs['stdout'].write(b'synthetic-private-canary')
            return Mock(returncode=23)

        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'result'
            public = io.StringIO()
            with patch.object(probe.Samples, 'sample', sample), \
                    patch.object(probe.subprocess, 'run', side_effect=child), \
                    patch.dict(os.environ, {'GITHUB_STEP_SUMMARY': ''}), redirect_stdout(public):
                result = probe.measure('native', ['synthetic'], output, temporary)
            self.assertEqual(result, 23)
            report = json.loads(public.getvalue())
            self.assertTrue(report['sampling_error'])
            self.assertEqual(report['child_exit'], 23)
            self.assertNotIn('canary', public.getvalue())
            self.assertEqual((output / 'child.log').read_bytes(), b'synthetic-private-canary')
