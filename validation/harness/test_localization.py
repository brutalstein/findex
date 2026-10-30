"""Failed/invalid retrieval must not silently become a scored empty answer."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock

from localization import findex_rank


class LocalizationTests(unittest.TestCase):
    def query(self, code, stdout):
        log = Mock()
        log.run.return_value = subprocess.CompletedProcess([], code, stdout, "failure")
        with tempfile.TemporaryDirectory() as folder:
            return findex_rank(log, Path("findex.exe"), Path(folder), Path(folder) / "state",
                               ["context", "--json", "example issue"])

    def test_failed_command_is_not_scored_as_empty_retrieval(self):
        with self.assertRaises(RuntimeError):
            self.query(1, '{"evidence": []}')

    def test_invalid_report_is_not_scored_as_empty_retrieval(self):
        for invalid in ("not JSON", "[]", "{}", '{"evidence": "invalid"}'):
            with self.subTest(report=invalid), self.assertRaises(RuntimeError):
                self.query(0, invalid)

    def test_valid_empty_and_ordered_distinct_paths(self):
        self.assertEqual(self.query(0, '{"evidence": []}')[0], [])
        self.assertEqual(self.query(0, '{"evidence": [{"path":"b.rs"},{"path":"a.rs"},{"path":"b.rs"}]}')[0], ["b.rs", "a.rs"])

    def test_explicit_ranking_precedes_packed_evidence_order(self):
        self.assertEqual(self.query(0, '{"ranked":["a.rs","b.rs"],"evidence":[{"path":"b.rs"},{"path":"c.rs"}]}')[0],
                         ["a.rs", "b.rs", "c.rs"])

    def test_invalid_explicit_ranking_is_not_scored(self):
        for ranked in ('"a.rs"', '[1]', '[""]'):
            with self.subTest(ranked=ranked), self.assertRaises(RuntimeError):
                self.query(0, '{"ranked":' + ranked + ',"evidence":[]}')
