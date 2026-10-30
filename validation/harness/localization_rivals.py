"""Offline issue localization for Graphify and codebase-memory on the tasks of localization.py.

Per task the repository is checked out at the fix's parent commit, each rival indexes it
through its documented surface and receives only the issue title. The rank of the first
gold file is taken over the unique paths of its ranked output. No model or API calls.
Graphify: `extract --code-only` then `query --budget`. codebase-memory: `index_repository`
then `search_graph` (a natural-language `query`, JSON rows).
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

from common import REPO, VALIDATION, WORK, results_dir, write_json
from issues import HISTORY
from localization import metrics, split
from mcp_client import McpProcess
from tool_comparison import cbm_hits, graphify_hits, invoke


def unique_paths(hits: list[dict], tree: Path, base: Path) -> list[str]:
    """Repository-relative POSIX paths in first-seen order. `base` is what a relative
    tool path is relative to (Graphify: its output directory; codebase-memory: the tree)."""
    paths: list[str] = []
    for hit in hits:
        path = Path(hit["path"])
        try:
            path = (path if path.is_absolute() else base / path).resolve().relative_to(tree.resolve())
        except ValueError:
            pass
        text = path.as_posix()
        if text not in paths:
            paths.append(text)
    return paths


def row(paths: list[str], gold: set[str], size: int, seconds: float) -> dict:
    ranks = [i + 1 for i, p in enumerate(paths) if p in gold]
    return {"first_gold_rank": ranks[0] if ranks else None, "returned": len(paths),
            "gold_found@10": len({p for p in paths[:10] if p in gold}) / len(gold),
            "bytes": size, "seconds": round(seconds, 4), "top5": paths[:5]}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default=None)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--split", choices=("dev", "test"), default=None)
    parser.add_argument("--dataset", type=Path, default=VALIDATION / "golden" / "issue_localization.jsonl")
    parser.add_argument("--graphify", type=Path, default=Path("C:/Users/cenke/.local/bin/graphify.exe"))
    parser.add_argument("--cbm", type=Path, default=Path("C:/Users/cenke/.cbm-bin/codebase-memory-mcp.exe"))
    parser.add_argument("--budget-tokens", type=int, default=8000, help="Graphify query budget (as for Findex context)")
    args = parser.parse_args()
    out = results_dir(args.out)
    if (out / "localization-rivals.json").exists():
        raise ValueError("Preserve attempted measurements; select a new output directory")
    tasks = [json.loads(line) for line in args.dataset.open(encoding="utf-8")]
    tasks = [t for t in tasks if args.split in (None, split(t))]
    tasks = tasks[: args.limit] if args.limit else tasks
    scratch = (WORK / "localization-rivals" / out.name).resolve()
    if not scratch.is_relative_to(WORK.resolve()):
        raise ValueError("Scratch path escaped validation workspace")
    tree, graph_out = scratch / "tree", scratch / "graphify"
    per_task = []
    for task in tasks:
        history = HISTORY / task["corpus"]
        if tree.exists():
            subprocess.run(["git", "-C", str(history), "worktree", "remove", "--force", str(tree)], capture_output=True)
            shutil.rmtree(tree, ignore_errors=True)
        shutil.rmtree(graph_out, ignore_errors=True)
        subprocess.run(["git", "-C", str(history), "worktree", "prune"], capture_output=True)
        subprocess.run(["git", "-C", str(history), "worktree", "add", "-q", "--detach", str(tree), task["base_commit"]],
                       check=True, capture_output=True)
        gold = set(task["gold_files"])
        result = {"id": task["id"], "corpus": task["corpus"], "split": split(task), "query": task["query"],
                  "gold_files": task["gold_files"]}
        try:
            invoke([str(args.graphify), "extract", str(tree), "--code-only", "--max-workers", "2",
                    "--out", str(graph_out)], REPO)
            text, seconds = invoke([str(args.graphify), "query", task["query"], "--graph",
                                    str(graph_out / "graphify-out/graph.json"), "--budget", str(args.budget_tokens)], tree)
            result["graphify"] = row(unique_paths(graphify_hits(text), tree, graph_out), gold, len(text.encode("utf-8")), seconds)
        except (OSError, RuntimeError, subprocess.TimeoutExpired) as exc:
            result["graphify"] = dict(row([], gold, 0, 0.0), error=str(exc)[-300:])
        name = re.sub(r"[^a-zA-Z0-9_-]", "-", f"loc-{out.name}-{task['id']}")
        cbm = McpProcess(args.cbm, tree, dict(os.environ), server_args=[])
        try:
            indexed, _, _, index_error = cbm.call_tool("index_repository", {
                "repo_path": str(tree), "name": name, "mode": "full", "persistence": False})
            if index_error:
                raise RuntimeError(f"index failed: {indexed}")
            payload, text, seconds, error = cbm.call_tool("search_graph", {
                "project": name, "query": task["query"], "limit": 32, "format": "json"})
            if error:
                raise RuntimeError(f"search failed: {text[:200]}")
            result["codebase-memory"] = row(unique_paths(cbm_hits(payload or {}), tree, tree), gold,
                                            len(text.encode("utf-8")), seconds)
        except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as exc:
            result["codebase-memory"] = dict(row([], gold, 0, 0.0), error=str(exc)[-300:])
        finally:
            cbm.close()
        per_task.append(result)
        print(f"[rivals] {task['id']}: graphify={result['graphify']['first_gold_rank']} "
              f"codebase-memory={result['codebase-memory']['first_gold_rank']}", file=sys.stderr, flush=True)
        write_json(out / "localization-rivals-partial.json", {"per_task": per_task})
    systems = ("graphify", "codebase-memory")
    write_json(out / "localization-rivals.json", {
        "status": "EXECUTED", "tasks": len(per_task), "budget_tokens": args.budget_tokens,
        "systems": {s: {"all": metrics([r[s] for r in per_task]),
                        "by_split": {k: metrics([r[s] for r in per_task if r["split"] == k])
                                     for k in ("dev", "test") if any(r["split"] == k for r in per_task)},
                        "by_corpus": {c: metrics([r[s] for r in per_task if r["corpus"] == c])
                                      for c in sorted({r["corpus"] for r in per_task})}} for s in systems},
        "per_task": per_task,
        "note": "query = GitHub issue title; same gold and checkouts as localization.py; rivals use their documented CLI/MCP surfaces; errors count as misses"})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
