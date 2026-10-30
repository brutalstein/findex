"""Offline checks for the live localization lane's task choice and answer scoring."""
import unittest

from localization_live import answer_files, score, select_tasks


class LocalizationLiveTests(unittest.TestCase):
    def test_answer_needs_exactly_one_object_with_a_files_list(self):
        self.assertEqual(answer_files('Here: {"files":["a/b.py","./c.rs"]}'), ["a/b.py", "c.rs"])
        self.assertEqual(answer_files('```json\n{"files":["src\\\\a.py"]}\n```'), ["src/a.py"])
        for text in ('{"files":["a"]} {"files":["b"]}', '{"path":"a"}', '{"files":"a"}', "no json"):
            with self.subTest(text=text):
                self.assertEqual(answer_files(text), [])

    def test_hit1_needs_the_first_file_hit3_any_of_three(self):
        gold = ["x.py"]
        self.assertEqual(score(["x.py", "y.py"], gold), {"hit1": True, "hit3": True})
        self.assertEqual(score(["y.py", "z.py", "x.py"], gold), {"hit1": False, "hit3": True})
        self.assertEqual(score(["a", "b", "c", "x.py"], gold), {"hit1": False, "hit3": False})
        self.assertEqual(score([], gold), {"hit1": False, "hit3": False})

    def test_tasks_are_test_split_only_and_reproducible(self):
        rows = [{"id": f"{c}-{n}", "corpus": c, "issue": n} for c in ("a", "b") for n in range(1, 13)]
        chosen = select_tasks(rows, 2)
        self.assertEqual(chosen, select_tasks(list(reversed(rows)), 2))
        self.assertEqual(len(chosen), 4)
        self.assertTrue(all(r["issue"] % 2 == 0 for r in chosen))


if __name__ == "__main__":
    unittest.main()
