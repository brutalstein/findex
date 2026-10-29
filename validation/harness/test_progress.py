"""The progress page: verdict rules, change direction and freshness of the committed charts."""
import unittest

import progress


class ProgressTests(unittest.TestCase):
    def test_verdict_separates_lead_tie_and_loss(self):
        self.assertEqual(progress.verdict(0.9, {"cbm": 0.8}, 1), "leads codebase-memory")
        self.assertEqual(progress.verdict(0.8, {"cbm": 0.9}, 1), "TRAILS codebase-memory")
        self.assertEqual(progress.verdict(264, {"cbm": 266}, -1), "ties codebase-memory")
        self.assertEqual(progress.verdict(200, {"cbm": 266}, -1), "leads codebase-memory")
        self.assertEqual(progress.verdict(300, {"cbm": 266}, -1), "TRAILS codebase-memory")
        self.assertEqual(progress.verdict(1, {}, 1), "no rival measured")

    def test_change_arrow_follows_metric_polarity(self):
        rounds = lambda a, b: [{"data": {"live": {"findex": {"tokens": v}}}} for v in (a, b)]
        self.assertTrue(progress.delta(rounds(100, 80), "live", "tokens", ",.0f", -1).startswith("▲"))
        self.assertTrue(progress.delta(rounds(80, 100), "live", "tokens", ",.0f", -1).startswith("▼"))
        self.assertEqual(progress.delta(rounds(5, 5), "live", "tokens", ",.0f", -1), "no change")
        self.assertEqual(progress.delta(rounds(5, 5)[:1], "live", "tokens", ",.0f", -1), "first measurement")

    def test_committed_page_and_charts_are_current(self):
        stale = [p.name for p, text in progress.render().items()
                 if not p.exists() or p.read_text(encoding="utf-8") != text]
        self.assertEqual(stale, [], "run: python validation/harness/progress.py")


if __name__ == "__main__":
    unittest.main()
