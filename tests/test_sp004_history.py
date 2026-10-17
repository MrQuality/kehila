"""Evidence framing for the opt-in native history experiment."""
import importlib.util
from pathlib import Path
import unittest

SOURCE = Path(__file__).resolve().parents[1] / 'experiments/SP-004/history.py'
spec = importlib.util.spec_from_file_location('sp004_history', SOURCE)
history = importlib.util.module_from_spec(spec)
spec.loader.exec_module(history)


class EvidenceTests(unittest.TestCase):
    def test_one_json_record_among_psql_command_tags(self):
        self.assertEqual(history.record('SET\n {"events":10}\n(1 row)\n'), {'events': 10})

    def test_missing_duplicate_malformed_and_oversized_evidence_fail(self):
        for output in ('SET\n', '{}\n{}\n', '{bad}\n', '{"value":[{"actor":1},\n {"actor":2}]}',
                       '{"x":"' + 'x' * 65536 + '"}'):
            with self.subTest(output=output[:20]), self.assertRaises(ValueError):
                history.record(output)


if __name__ == '__main__':
    unittest.main()
