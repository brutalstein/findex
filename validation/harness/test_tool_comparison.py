"""Deterministic checks for the comparison's selection and native result adapters."""
import unittest

from tool_comparison import cbm_hits, graphify_hits, select_queries, superiority
from tool_comparison_live import answer_correct, codex_env, codex_subscription_error


class ComparisonTests(unittest.TestCase):
    def test_subscription_gate_and_env_never_accept_an_api_key(self):
        self.assertIsNone(codex_subscription_error(0, "Logged in using ChatGPT"))
        self.assertIsNotNone(codex_subscription_error(0, "Logged in using an API key"))
        self.assertIsNotNone(codex_subscription_error(1, "Logged in using ChatGPT"))
        self.assertEqual(codex_env({"OPENAI_API_KEY": "fake", "ANTHROPIC_API_KEY": "fake", "PATH": "safe"}), {"PATH": "safe"})

    def test_task_success_requires_a_correct_source_anchor(self):
        q = {"expected": [["src/a.py", 7]]}
        self.assertTrue(answer_correct('{"path":"src/a.py","line":7}', q))
        self.assertFalse(answer_correct('{"path":"wrong.py","line":7}', q))
        self.assertFalse(answer_correct('{"path":"src/a.py","line":true}', q))

    def test_selection_is_balanced_reproducible_and_heldout_only(self):
        rows = [{"id": f"{c}:{i}", "corpus": c, "type": str(i % 3), "split": "heldout"}
                for c in ("a", "b") for i in range(12)]
        rows.append({"id": "dev", "corpus": "a", "type": "0", "split": "dev"})
        chosen = select_queries(rows, ["a", "b"], 5)
        self.assertEqual(chosen, select_queries(list(reversed(rows)), ["a", "b"], 5))
        self.assertEqual([sum(r["corpus"] == c for r in chosen) for c in ("a", "b")], [5, 5])
        self.assertTrue(all(r["split"] == "heldout" for r in chosen))
        with self.assertRaises(ValueError):
            select_queries(rows, ["missing"], 5)

    def test_adapters_preserve_native_order_and_line_anchors(self):
        self.assertEqual(cbm_hits({"cols": ["qn", "file", "lines"], "rows": [
            ["b", "src/b.rs", "9-12"], ["a", "src/a.rs", "2-4"]]}),
            [{"path": "src/b.rs", "line": 9}, {"path": "src/a.rs", "line": 2}])
        self.assertEqual(cbm_hits({"cols": ["name", "lines"], "groups": [
            {"file": "src/a.py", "rows": [["foo", "7-8"]]}]}),
            [{"path": "src/a.py", "line": 7}])
        self.assertEqual(graphify_hits("NODE foo() [src=src/a.py loc=L7 community=1]\n"
                                       "NODE External [src= loc= community=1]"),
                         [{"path": "src/a.py", "line": 7}])
        self.assertEqual(graphify_hits("NODE foo() [src=src/a.py loc=L7 community=1]\n"
                                       "NODE bar() [src=src/b.py loc=L2"),
                         [{"path": "src/a.py", "line": 7}])

    def test_gate_never_turns_a_tie_or_missing_live_evidence_into_a_win(self):
        base = {"hit_at_1": .5, "span_at_1": .4, "mean_tokens": 200}
        self.assertFalse(superiority({t: dict(base) for t in ("findex", "graphify", "codebase-memory")})["local_pass"])
        better = {"hit_at_1": .6, "span_at_1": .5, "mean_tokens": 100}
        result = superiority({"findex": better, "graphify": base, "codebase-memory": base})
        self.assertTrue(result["local_pass"])
        self.assertEqual(result["overall"], "NOT_ESTABLISHED")


if __name__ == "__main__":
    unittest.main()
