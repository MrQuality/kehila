"""Resource profiles must keep memory, tmpfs, admission and stop guards aligned."""
from dataclasses import FrozenInstanceError
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from native_database_limits import DatabaseLimits, storage_limits
from test_container_native import database, DATABASE, RUNNER, OWNER, NAME
from container_native import admit_database


class DatabaseLimitTests(unittest.TestCase):
    def test_default_stays_256_and_extended_is_explicit_and_only_for_last_four_samples(self):
        ordinary = storage_limits(20)
        self.assertEqual((ordinary.mib, ordinary.memory_bytes, ordinary.stop_bytes),
                         (256, 256*1024**2, 192*1024**2))
        extended = storage_limits(20, 'extended')
        self.assertEqual((extended.mib, extended.memory_bytes, extended.stop_bytes),
                         (512, 512*1024**2, 448*1024**2))
        for index, profile in ((19, 'extended'), (True, 'extended'), (24, 'extended'), (20, 'typo')):
            with self.subTest(index=index, profile=profile), self.assertRaises(ValueError):
                storage_limits(index, profile)
        for value in (True, 1024, 512.0, '512'):
            with self.subTest(value=value), self.assertRaises(ValueError):
                DatabaseLimits(value)
        with self.assertRaises(FrozenInstanceError):
            extended.mib = 1024

    def test_default_admission_cannot_silently_adopt_extended_or_mismatched_limits(self):
        info = database()
        info['HostConfig']['Memory'] = 512*1024**2
        info['HostConfig']['Tmpfs']['/var/lib/postgresql/data'] = 'rw,size=512m'
        with self.assertRaises(ValueError):
            admit_database(info, DATABASE, RUNNER, OWNER, NAME)
        admit_database(info, DATABASE, RUNNER, OWNER, NAME, limits=DatabaseLimits(512))
        info['HostConfig']['Tmpfs']['/var/lib/postgresql/data'] = 'rw,size=256m'
        with self.assertRaises(ValueError):
            admit_database(info, DATABASE, RUNNER, OWNER, NAME, limits=DatabaseLimits(512))


if __name__ == '__main__':
    unittest.main()
