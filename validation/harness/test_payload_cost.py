"""Offline adapters for the lossless payload comparison."""
import unittest
from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import tempfile

from payload_cost import MODES, cbm_tree_hits, expand_hits, same_hits, save_summary


class PayloadTests(unittest.TestCase):
    def test_same_hits_ignores_score_precision_but_nothing_else(self):
        old = [{"path": "a.py", "line": 1, "bm25_score": -7.556564023538062}]
        self.assertTrue(same_hits(old, [{"path": "a.py", "line": 1, "bm25_score": -7.557}]))
        self.assertFalse(same_hits(old, [{"path": "a.py", "line": 2, "bm25_score": -7.557}]))
        self.assertFalse(same_hits(old, [{"path": "a.py", "line": 1, "bm25_score": -7.6}]))
        self.assertFalse(same_hits(old, old + old))

    def test_idle_sync_expands_all_counters_without_hiding_flags(self):
        expected = {key: 0 for key in ("examined", "indexed", "unchanged", "removed", "skipped", "cloud_only", "symbols_parsed", "symbols_reused")}
        self.assertEqual(expand_hits({"columns": [], "hits": [], "sync": 0, "truncated": True}),
                         {"hits": [], "sync": expected, "truncated": True})

    def test_expansion_preserves_flags_and_source_strings(self):
        self.assertEqual(expand_hits({"columns": ["path", "line"], "hits": [["a\n.py", 7]], "truncated": True}),
                         {"hits": [{"path": "a\n.py", "line": 7}], "truncated": True})
        with self.assertRaises(ValueError):
            expand_hits({"columns": ["path", "line"], "hits": [["a.py"]]})

    def test_tree_parser_handles_native_flat_and_grouped_modes(self):
        flat = "total: 1\nresults: 1 (cols: qn label file lines rank)\n  project.foo Method src/a.py 7-9 -1.25\nhas_more: false"
        grouped = "total: 1\nproject.src (src/a.py):\n  foo Method 7-9 2 3\nhas_more: false"
        for text in (flat, grouped):
            self.assertEqual(cbm_tree_hits(text), [{"path": "src/a.py", "line": 7}])

    def test_tree_parser_preserves_quoted_cpp_names_and_paths(self):
        text = 'results: 1 (cols: qn label file lines rank)\n  "project.Matcher < std :: string >.Matcher" Method "src/a b.h" 7-9 -1.25'
        self.assertEqual(cbm_tree_hits(text), [{"path": "src/a b.h", "line": 7}])

    def test_summary_keeps_losing_controls_beside_a_winning_primary_subset(self):
        records = []
        for corpus in ("requests", "zod", "fmt", "ripgrep"):
            for subset in ("new-55", "historical-60"):
                for mode in MODES:
                    ours = mode == "findex-compact"
                    tokens = (10 if subset == "new-55" else 30) if ours else 20
                    records.append({"corpus": corpus, "evaluation_set": subset, "mode": mode,
                                    "raw_tokens": tokens, "visible_tokens": tokens, "seconds": .01,
                                    "error": False, "over_budget": False, "scores": {
                                        "file_recall@1": 1 if ours else .5,
                                        "span_recall@1": 1 if ours else .5, "file_mrr": 1}})
        with tempfile.TemporaryDirectory() as folder, redirect_stdout(io.StringIO()):
            out = Path(folder)
            save_summary(out, {"queries": list(range(8))}, records)
            result = json.loads((out / "summary.json").read_text())
        self.assertTrue(result["gate"]["local_pass"])
        self.assertFalse(result["gates_by_scope"]["historical-60"]["local_pass"])
        self.assertFalse(result["gates_by_scope"]["all-115"]["local_pass"])


if __name__ == "__main__":
    unittest.main()
