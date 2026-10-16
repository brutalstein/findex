"""Does the rendered `findex context` report fit its token budget?

For the first 40 golden names of a corpus, asks "callers of `name`" and
"where is `name` defined" at several budgets and compares the rendered text
length / 4 (the engine's own token estimate) with the budget.
Usage: budget_check.py [--corpus ripgrep] [--out DIR]  -> budget_check.json
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys

from common import CORPORA, VALIDATION, WORK, CommandLog, findex_binary, isolated_env, results_dir, write_json


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", default="ripgrep")
    parser.add_argument("--out", default=None)
    args = parser.parse_args()
    out = results_dir(args.out)
    log = CommandLog(out)
    exe, root = findex_binary("current"), CORPORA / args.corpus
    state = WORK / "budget-state" / args.corpus
    shutil.rmtree(state, ignore_errors=True)
    env = isolated_env(state)
    log.run([str(exe), "index"], cwd=root, env=env, timeout=1800)
    rows = [json.loads(line) for line in (VALIDATION / "golden" / "symbol_navigation.jsonl").open(encoding="utf-8")]
    names = sorted({r["name"] for r in rows if r["corpus"] == args.corpus and r["name"]})[:40]
    result = {"status": "EXECUTED", "corpus": args.corpus, "names": len(names), "budgets": {}}
    for budget in (200, 500, 2000):
        ratios = sorted(
            len(log.run([str(exe), "context", "--budget", str(budget), task], cwd=root, env=env, timeout=300).stdout)
            / 4 / budget
            for name in names for task in (f"callers of `{name}`", f"where is `{name}` defined"))
        result["budgets"][budget] = {"p50": ratios[len(ratios) // 2], "p95": ratios[int(len(ratios) * 0.95)],
                                     "max": ratios[-1], "over_budget": sum(r > 1 for r in ratios), "reports": len(ratios)}
        print(f"[budget] {budget}: {result['budgets'][budget]}", file=sys.stderr)
    write_json(out / "budget_check.json", result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
