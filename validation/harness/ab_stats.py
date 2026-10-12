"""Paired arm-vs-A statistics for a claude_ab.py run (tokens.json).

Pairs each arm's session with arm A's session of the same task and
repetition; reports mean total-token and turn differences with a 95 %
bootstrap CI (10,000 draws, seed 11), per task and pooled over tasks.
Usage: ab_stats.py <results-dir>  -> ab_stats.json
"""
from __future__ import annotations

import json
import random
import sys
from pathlib import Path

from common import write_json

TOKENS = ("input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")


def total(session: dict) -> int:
    return sum(session.get(k) or 0 for k in TOKENS)


def ci(diffs: list[float], rng: random.Random) -> dict:
    means = sorted(sum(rng.choice(diffs) for _ in diffs) / len(diffs) for _ in range(10_000))
    return {"n": len(diffs), "mean": sum(diffs) / len(diffs), "ci95": [means[250], means[9749]]}


def main() -> int:
    out = Path(sys.argv[1])
    data = json.loads((out / "tokens.json").read_text(encoding="utf-8"))
    cell = {(s["task"], s["arm"], str(s["rep"])): s for s in data["sessions"] if s["status"] == "EXECUTED"}
    rng = random.Random(11)
    arms = sorted({a for _, a, _ in cell} - {"A"})
    tasks = sorted({t for t, _, _ in cell})
    result = {"pairing": "same task and repetition", "draws": 10_000, "seed": 11, "arms": {}}
    for arm in arms:
        pairs = [(cell[(t, arm, r)], cell[(t, "A", r)]) for (t, a, r) in cell if a == arm and (t, "A", r) in cell]
        per_task = {}
        for task in tasks:
            sub = [(x, a) for x, a in pairs if x["task"] == task]
            if sub:
                per_task[task] = {"tokens": ci([total(x) - total(a) for x, a in sub], rng),
                                  "turns": ci([(x["num_turns"] or 0) - (a["num_turns"] or 0) for x, a in sub], rng),
                                  "success": [sum(bool(x["correct"]) for x, _ in sub), sum(bool(a["correct"]) for _, a in sub), len(sub)],
                                  "findex_used": sum(1 for x, _ in sub if x.get("mcp_call_count"))}
        result["arms"][arm] = {
            "pooled_tokens": ci([total(x) - total(a) for x, a in pairs], rng),
            "pooled_relative_tokens": sum(total(x) for x, _ in pairs) / sum(total(a) for _, a in pairs) - 1,
            "pooled_turns": ci([(x["num_turns"] or 0) - (a["num_turns"] or 0) for x, a in pairs], rng),
            "success": [sum(bool(x["correct"]) for x, _ in pairs), sum(bool(a["correct"]) for _, a in pairs), len(pairs)],
            "per_task": per_task}
    write_json(out / "ab_stats.json", result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
