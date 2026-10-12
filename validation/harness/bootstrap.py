"""Paired bootstrap confidence intervals for localization.json differences.

For each split, the per-task difference findex-context minus another system
is resampled with replacement (10,000 draws, fixed seed) and the 2.5/97.5
percentiles are reported. Reads and writes next to the given localization.json.
Usage: bootstrap.py <results-dir>
"""
from __future__ import annotations

import json
import random
import sys
from pathlib import Path

from common import write_json

DRAWS, SEED = 10_000, 11


def reciprocal_rank(row: dict) -> float:
    return 1 / row["first_gold_rank"] if row["first_gold_rank"] else 0.0


def hit(k: int):
    return lambda row: 1.0 if row["first_gold_rank"] and row["first_gold_rank"] <= k else 0.0


METRICS = {"mrr": reciprocal_rank, "hit@1": hit(1), "hit@5": hit(5), "hit@10": hit(10)}


def interval(diffs: list[float], rng: random.Random) -> dict:
    means = sorted(sum(rng.choice(diffs) for _ in diffs) / len(diffs) for _ in range(DRAWS))
    return {"mean_difference": sum(diffs) / len(diffs), "ci95": [means[int(DRAWS * 0.025)], means[int(DRAWS * 0.975) - 1]]}


assert interval([1.0, 1.0], random.Random(0))["ci95"] == [1.0, 1.0]


def main() -> int:
    out = Path(sys.argv[1])
    data = json.loads((out / "localization.json").read_text(encoding="utf-8"))
    rng = random.Random(SEED)
    result = {"draws": DRAWS, "seed": SEED, "splits": {}}
    for split in ("dev", "test"):
        rows = [r for r in data["per_task"] if r["split"] == split]
        result["splits"][split] = {"n": len(rows), **{
            f"findex-context_minus_{other}": {name: interval([f(r["findex-context"]) - f(r[other]) for r in rows], rng)
                                              for name, f in METRICS.items()}
            for other in ("findex-main", "gitgrep-words")}}
    write_json(out / "localization_bootstrap.json", result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
