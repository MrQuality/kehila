"""Dependency reuse cannot substitute foreign state or skip source verification."""
from pathlib import Path
import json
import sys
import tempfile
import unittest
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from container_cache import DependencyCache, LABEL
from container_dependencies import readiness
from container_verify import prepare_dependencies


class CacheTests(unittest.TestCase):
    def test_manifest_or_image_changes_select_a_different_cache_but_source_does_not(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'src').mkdir()
            (root / 'go').mkdir()
            for name in ('Cargo.lock', 'Cargo.toml', 'go.work'):
                (root / name).write_text(name)
            initial = DependencyCache.for_source(root, 'image-a')
            (root / 'src' / 'lib.rs').write_text('changed source')
            self.assertEqual(initial, DependencyCache.for_source(root, 'image-a'))
            self.assertNotEqual(initial, DependencyCache.for_source(root, 'image-b'))
            (root / 'Cargo.lock').write_text('changed dependency')
            self.assertNotEqual(initial, DependencyCache.for_source(root, 'image-a'))

    def test_gate_never_creates_an_unprepared_volume(self):
        engine = Mock(return_value=Mock(stdout=''))
        with self.assertRaisesRegex(ValueError, 'prepare-dependencies'):
            DependencyCache('a' * 64).admit(engine, prepare=False)
        self.assertEqual(engine.call_count, 1)

    def test_foreign_or_host_redirected_volume_is_not_adopted(self):
        cache = DependencyCache('a' * 64)
        valid = {'Name': cache.name, 'Driver': 'local', 'Labels': {LABEL: cache.key},
                 'Options': {}, 'Mountpoint': '/engine/volume'}
        for change in ({'Labels': {}}, {'Options': {'device': '/host'}}, {'Driver': 'plugin'}):
            with self.subTest(change=change):
                engine = Mock(side_effect=[Mock(stdout=cache.name),
                    Mock(stdout=json.dumps([{**valid, **change}]))])
                with self.assertRaises(ValueError):
                    cache.admit(engine, prepare=False)
                self.assertFalse(any(call.args[:2] == ('volume', 'create')
                                     for call in engine.call_args_list))

    def test_preparation_uses_explicit_volume_owner_and_checks_created_identity(self):
        cache = DependencyCache('a' * 64)
        state = {'Name': cache.name, 'Driver': 'local', 'Labels': {LABEL: cache.key},
                 'Options': None, 'Mountpoint': '/engine/volume'}
        engine = Mock(side_effect=[Mock(stdout=''), Mock(), Mock(stdout=json.dumps([state]))])
        self.assertEqual(cache.admit(engine, prepare=True), '/engine/volume')
        create = engine.call_args_list[1].args
        self.assertIn('--uid=10001', create)
        self.assertIn('--gid=10001', create)

    def test_ready_witness_requires_success_and_exact_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            key = 'a' * 64
            with self.assertRaises(OSError):
                readiness('check', key, root)
            readiness('mark', key, root)
            readiness('check', key, root)
            with self.assertRaises(ValueError):
                readiness('check', 'b' * 64, root)
            readiness('clear', key, root)
            with self.assertRaises(OSError):
                readiness('check', key, root)
            self.assertEqual(list(root.iterdir()), [])

    def test_failed_fetch_cannot_publish_ready_or_skip_copied_source_check(self):
        executor = Mock()
        executor.cache.key = 'a' * 64
        primary = RuntimeError('fetch failed')
        executor.execute.side_effect = [None, primary]
        with self.assertRaises(RuntimeError) as caught:
            prepare_dependencies(executor, 1200)
        self.assertIs(caught.exception, primary)
        executor.verify_source.assert_not_called()
        self.assertEqual(executor.execute.call_count, 2)

    def test_preparation_downloads_without_compiling_and_verifies_source_before_marking(self):
        events = []
        executor = Mock()
        executor.cache.key = 'a' * 64
        executor.execute.side_effect = lambda command, **kw: events.append(command)
        executor.verify_source.side_effect = lambda: events.append(['verify-source'])
        prepare_dependencies(executor, 1200)
        self.assertEqual(events[1], ['cargo', 'fetch', '--locked', '--target',
                                     'x86_64-unknown-linux-gnu'])
        self.assertEqual(events[2], ['go', 'mod', 'download', 'all'])
        self.assertEqual(events[-2], ['verify-source'])
        self.assertEqual(events[-1][-2], 'mark')
