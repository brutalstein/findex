"""Evaluate file localization on the frozen issue dataset (issues.py).

For each task the repository is checked out at the fix's parent commit and
each system receives only the issue title. Systems:
  gitgrep-words   files ranked by distinct title words found (git grep -i -w)
  findex-main     `findex search` from the frozen main build (caf33fa)
  findex-context  `findex context` from this checkout's release build
Metrics: hit@k (any gold file in the top k), MRR of the first gold file,
gold-file recall@10, empty-result rate, output bytes.
Usage: localization.py [--out DIR] [--limit N] [--split dev|test]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

from common import VALIDATION, WORK, CommandLog, findex_binary, isolated_env, percentiles, results_dir, write_json
from issues import HISTORY

STOP = set("""a an and any are as at be but by can cannot do does doesn't for from has have how if in into is it its
not of on or should so than that the then there this to too was were when where which while why will with without
""".split())
WORDS = re.compile(r"[A-Za-z_][A-Za-z0-9_]{2,}")


def gitgrep_rank(repo: Path, query: str) -> list[str]:
    words = sorted({w.lower() for w in WORDS.findall(query)} - STOP)
    score: dict[str, int] = {}
    for word in words:
        out = subprocess.run(["git", "-C", str(repo), "grep", "-i", "-l", "-w", "-I", "-F", word],
                             capture_output=True, text=True, encoding="utf-8", errors="replace").stdout
        for path in out.splitlines():
            score[path] = score.get(path, 0) + 1
    return [p for p, _ in sorted(score.items(), key=lambda kv: (-kv[1], kv[0]))]


def findex_rank(log: CommandLog, exe: Path, repo: Path, state: Path, argv: list[str]) -> tuple[list[str], int, float]:
    started = time.perf_counter()
    proc = log.run([str(exe), *argv], cwd=repo, env=isolated_env(state), timeout=600)
    elapsed = time.perf_counter() - started
    if proc.returncode != 0:
        raise RuntimeError(f"Retrieval command failed with exit {proc.returncode}; no score recorded")
    paths: list[str] = []
    if argv[0] == "context":
        try:
            report = json.loads(proc.stdout)
        except ValueError as exc:
            raise RuntimeError("Context response is not valid JSON; no score recorded") from exc
        if not isinstance(report, dict) or not isinstance(report.get("evidence"), list):
            raise RuntimeError("Context response has no evidence array; no score recorded")
        ranked = report.get("ranked", [])
        if not isinstance(ranked, list) or any(not isinstance(p, str) or not p for p in ranked):
            raise RuntimeError("Context response has invalid ranked paths; no score recorded")
        candidates = ranked + [e["path"] for e in report["evidence"]]
    else:
        candidates = [line.split(":", 1)[0] for line in proc.stdout.splitlines() if re.match(r"^\S+:\d+\s", line)]
    for path in candidates:
        if path not in paths:
            paths.append(path)
    return paths, len(proc.stdout.encode("utf-8")), elapsed


def split(task: dict) -> str:
    """Fixed before any result was seen: odd issue numbers may be inspected
    while developing (dev); even ones are reported only in aggregate (test)."""
    return "dev" if task["issue"] % 2 else "test"


def metrics(rows: list[dict]) -> dict:
    out = {"n": len(rows)}
    for k in (1, 3, 5, 10):
        out[f"hit@{k}"] = sum(1 for r in rows if r["first_gold_rank"] and r["first_gold_rank"] <= k) / len(rows)
    out["mrr"] = sum(1 / r["first_gold_rank"] for r in rows if r["first_gold_rank"]) / len(rows)
    out["gold_recall@10"] = sum(r["gold_found@10"] for r in rows) / len(rows)
    out["empty_rate"] = sum(1 for r in rows if not r["returned"]) / len(rows)
    out["output_bytes"] = percentiles([float(r["bytes"]) for r in rows])
    out["latency_s"] = percentiles([float(r["seconds"]) for r in rows])
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=None)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--split", choices=("dev", "test"), default=None,
                        help="evaluate one split only (development work must use dev)")
    parser.add_argument("--baseline-context", type=Path,
                        help="pair context with a frozen context binary instead of historical main lexical search")
    parser.add_argument("--budget-tokens", type=int, default=8000,
                        help="same rendered-context budget for both arms (200-8000)")
    args = parser.parse_args()
    if not 200 <= args.budget_tokens <= 8000:
        parser.error("budget-tokens must be between 200 and 8000")
    out = results_dir(args.out)
    if (out / "manifest.json").exists() or (out / "localization.json").exists():
        raise ValueError("Preserve attempted measurements; select a new output directory")
    log = CommandLog(out)
    tasks = [json.loads(line) for line in (VALIDATION / "golden" / "issue_localization.jsonl").open(encoding="utf-8")]
    tasks = [t for t in tasks if args.split in (None, split(t))]
    tasks = tasks[: args.limit] if args.limit else tasks
    systems = {"findex-baseline-context" if args.baseline_context else "findex-main":
               args.baseline_context or findex_binary("main"), "findex-context": findex_binary("current")}
    identities = {}
    for name, source in list(systems.items()):
        source = source.resolve(strict=True)
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        frozen = WORK / "bin" / "localization" / digest / "findex.exe"
        frozen.parent.mkdir(parents=True, exist_ok=True)
        if not frozen.exists():
            shutil.copy2(source, frozen)
        if hashlib.sha256(frozen.read_bytes()).hexdigest() != digest:
            raise ValueError("Frozen runtime mismatch")
        systems[name] = frozen
        identities[name] = digest
    write_json(out / "manifest.json", {"binary_sha256": identities,
        "golden_sha256": hashlib.sha256((VALIDATION / "golden/issue_localization.jsonl").read_bytes()).hexdigest(),
        "task_ids": [t["id"] for t in tasks], "split": args.split,
        "budget_tokens": args.budget_tokens, "policy": "Historical frozen issue-title tasks; dev inspected, test aggregate only; no model/API calls"})
    scratch = (WORK / "localization" / out.name).resolve()
    if not scratch.is_relative_to(WORK.resolve()):
        raise ValueError("Scratch path escaped validation workspace")
    per_task = []
    for task in tasks:
        history = HISTORY / task["corpus"]
        tree = scratch / "tree"
        if tree.exists():
            subprocess.run(["git", "-C", str(history), "worktree", "remove", "--force", str(tree)], capture_output=True)
            shutil.rmtree(tree, ignore_errors=True)
        subprocess.run(["git", "-C", str(history), "worktree", "prune"], capture_output=True)
        subprocess.run(["git", "-C", str(history), "worktree", "add", "-q", "--detach", str(tree), task["base_commit"]],
                       check=True, capture_output=True)
        results = {}
        started = time.perf_counter()
        ranked = gitgrep_rank(tree, task["query"])
        results["gitgrep-words"] = (ranked, sum(len(p) + 1 for p in ranked[:10]), time.perf_counter() - started)
        for name, exe in systems.items():
            state = scratch / f"state-{name}"
            shutil.rmtree(state, ignore_errors=True)
            indexed = log.run([str(exe), "index"], cwd=tree, env=isolated_env(state), timeout=900)
            if indexed.returncode:
                raise RuntimeError(f"Index initialization failed for {task['id']}; no score recorded")
            argv = ["context", "--json", "--budget", str(args.budget_tokens), task["query"]] if name.endswith("context") else ["search", task["query"]]
            results[name] = findex_rank(log, exe, tree, state, argv)
        gold = set(task["gold_files"])
        row = {"id": task["id"], "corpus": task["corpus"], "split": split(task), "query": task["query"],
               "gold_files": task["gold_files"]}
        for name, (paths, size, seconds) in results.items():
            ranks = [i + 1 for i, p in enumerate(paths) if p in gold]
            row[name] = {"first_gold_rank": ranks[0] if ranks else None, "returned": len(paths),
                         "gold_found@10": len({p for p in paths[:10] if p in gold}) / len(gold),
                         "bytes": size, "seconds": round(seconds, 4), "top5": paths[:5]}
        per_task.append(row)
        print(f"[loc] {task['id']}: " + " ".join(f"{n}={row[n]['first_gold_rank']}" for n in results), file=sys.stderr)
    systems_all = ["gitgrep-words", *systems]
    summary = {name: {"all": metrics([r[name] for r in per_task]),
                      "by_split": {s: metrics([r[name] for r in per_task if r["split"] == s])
                                   for s in ("dev", "test") if any(r["split"] == s for r in per_task)},
                      "by_corpus": {c: metrics([r[name] for r in per_task if r["corpus"] == c])
                                    for c in sorted({r["corpus"] for r in per_task})}}
               for name in systems_all}
    write_json(out / "localization.json", {"status": "EXECUTED", "dataset": "validation/golden/issue_localization.jsonl",
                                           "tasks": len(per_task), "systems": summary, "per_task": per_task,
                                           "note": "query = GitHub issue title; gold = non-test source files changed by the closing commit; evaluated at its parent"})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
