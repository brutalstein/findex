"""Retrieval benchmark: git-grep baseline vs. frozen Findex vs. current Findex.

Usage: retrieval.py [--systems a,b] [--split all|dev|heldout] [--corpora x,y] [--limit 10]
Writes retrieval.json into the shared results directory.
"""
from __future__ import annotations

import argparse
import json
import math
import re
import shutil
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

from common import CORPORA, VALIDATION, WORK, CommandLog, findex_binary, isolated_env, percentiles, results_dir, write_json
from golden import admitted_files
from mcp_client import McpProcess

SYSTEMS = ["gitgrep", "findex-baseline", "findex-current", "findex-current-symbol"]


def load_queries(split: str, corpora: list[str] | None) -> list[dict]:
    rows = [json.loads(line) for line in (VALIDATION / "golden" / "symbol_navigation.jsonl").open(encoding="utf-8")]
    return [r for r in rows if (split == "all" or r["split"] == split) and (not corpora or r["corpus"] in corpora)]


def terms_for(query: dict) -> list[str]:
    return [t for t in re.split(r"[^\w$]+", query["query"]) if t][:12]


def gitgrep(root: Path, query: dict, admitted: set[str], limit: int):
    argv = ["git", "-C", str(root), "grep", "-n", "-I", "-i", "-F", "--no-color"]
    for term in terms_for(query):
        argv += ["-e", term]
    started = time.perf_counter()
    out = subprocess.run(argv, capture_output=True, text=True, encoding="utf-8", errors="replace")
    seconds = time.perf_counter() - started
    per_file: dict[str, list] = defaultdict(list)
    for raw in out.stdout.splitlines():
        parts = raw.split(":", 2)
        if len(parts) < 3 or parts[0] not in admitted or not parts[1].isdigit():
            continue
        lowered = parts[2].lower()
        covered = sum(1 for t in terms_for(query) if t.lower() in lowered)
        per_file[parts[0]].append((covered, int(parts[1])))
    ranked = sorted(per_file.items(), key=lambda item: (-max(c for c, _ in item[1]), -len(item[1]), item[0]))
    hits = []
    for path, lines in ranked[:limit]:
        best = max(c for c, _ in lines)
        hits.append({"path": path, "line": min(n for c, n in lines if c == best)})
    return hits, len(out.stdout.encode("utf-8")), seconds


def score(query: dict, hits: list[dict], k_values=(1, 5, 10)) -> dict:
    expected = [tuple(e) for e in query["expected"]]
    paths = {p for p, _ in expected}
    file_rel = [h["path"] in paths for h in hits]
    span_rel = [any(h["path"] == p and abs(h["line"] - line) <= 1 for p, line in expected) for h in hits]
    result = {}
    for k in k_values:
        result[f"file_recall@{k}"] = float(any(file_rel[:k]))
        result[f"span_recall@{k}"] = float(any(span_rel[:k]))
        distinct = list(dict.fromkeys(h["path"] for h in hits[:k]))
        result[f"file_precision@{k}"] = sum(p in paths for p in distinct) / k
    result["file_mrr"] = next((1 / (i + 1) for i, r in enumerate(file_rel) if r), 0.0)
    result["span_mrr"] = next((1 / (i + 1) for i, r in enumerate(span_rel) if r), 0.0)
    # File-level nDCG: each distinct file counts once, at its first rank.
    first_ranks = {}
    for i, hit in enumerate(hits[:10]):
        first_ranks.setdefault(hit["path"], i)
    dcg = sum(1 / math.log2(i + 2) for p, i in first_ranks.items() if p in paths)
    ideal = sum(1 / math.log2(i + 2) for i in range(min(len(paths), 10)))
    result["file_ndcg@10"] = dcg / ideal if ideal else 0.0
    result["missing_evidence"] = float(not hits)
    result["top1_false_positive"] = float(bool(hits) and not file_rel[0])
    return result


def evidence_valid(root: Path, hit: dict, cache: dict) -> bool | None:
    """An excerpt must equal the cited line in the current file (trimmed, 240 chars)."""
    if "excerpt" not in hit:
        return None
    if hit["path"] not in cache:
        try:
            cache[hit["path"]] = (root / hit["path"]).read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeDecodeError):
            cache[hit["path"]] = []
    lines = cache[hit["path"]]
    if not 1 <= hit["line"] <= len(lines):
        return False
    actual = lines[hit["line"] - 1].strip()[:240]
    return hit["excerpt"] == actual or "[REDACTED" in hit["excerpt"]


def run_findex(system: str, corpus: str, queries: list[dict], limit: int, log: CommandLog, run_tag: str):
    exe = findex_binary("baseline" if "baseline" in system else "current")
    root = CORPORA / corpus
    state = WORK / "state" / run_tag / system.replace("-symbol", "") / corpus
    if state.exists():
        shutil.rmtree(state)
    env = isolated_env(state)
    started = time.perf_counter()
    client = McpProcess(exe, root, env)
    tool = "findex_find_symbol" if system.endswith("symbol") else "findex_search"
    warm_args = {"name": "warmup_marker", "limit": 1} if tool == "findex_find_symbol" else {"query": "warmup_marker", "limit": 1}
    _, _, cold_seconds, cold_error = client.call_tool(tool, warm_args)
    outputs = []
    for query in queries:
        if tool == "findex_find_symbol":
            if query["type"] != "symbol_exact":
                continue
            payload, text, seconds, is_error = client.call_tool(tool, {"name": query["query"], "limit": limit})
            hits = [{"path": h["path"], "line": h["start_line"]} for h in (payload or {}).get("hits", [])]
        else:
            payload, text, seconds, is_error = client.call_tool(tool, {"query": query["query"], "limit": limit})
            hits = (payload or {}).get("hits", [])
        outputs.append({"query": query, "hits": hits, "bytes": len(text.encode("utf-8")), "seconds": seconds,
                        "is_error": is_error, "truncated": (payload or {}).get("truncated")})
    peak = client.memory()
    code = client.close()
    log.record([str(exe), "mcp", f"<{len(queries)} {tool} calls>"], root, code, time.perf_counter() - started,
               note=f"{system} {corpus}")
    return outputs, {"cold_first_query_s": cold_seconds, "cold_error": cold_error, "memory_at_end": peak}


def summarize(outputs: list[dict], corpus_roots: dict) -> dict:
    groups = defaultdict(list)
    for item in outputs:
        q = item["query"]
        for key in ("all", f"{q['type']}", f"{q['type']}/{q['split']}", f"split/{q['split']}"):
            groups[key].append(item)
    summary = {}
    for key, items in sorted(groups.items()):
        metrics = defaultdict(float)
        for item in items:
            for name, value in score(item["query"], item["hits"]).items():
                metrics[name] += value
        result = {name: round(value / len(items), 4) for name, value in metrics.items()}
        caches: dict = defaultdict(dict)
        checks = [evidence_valid(corpus_roots[i["query"]["corpus"]], h, caches[i["query"]["corpus"]])
                  for i in items for h in i["hits"]]
        checks = [c for c in checks if c is not None]
        result["evidence_validity"] = round(sum(checks) / len(checks), 4) if checks else None
        result["queries"] = len(items)
        result["errors"] = sum(1 for i in items if i.get("is_error"))
        result["payload_bytes"] = percentiles([i["bytes"] for i in items])
        result["est_tokens_mean"] = round(result["payload_bytes"]["mean"] / 4, 1)
        result["latency_ms"] = {k: (v * 1000 if isinstance(v, float) and k not in ("n",) and k != "variance" else v)
                                for k, v in percentiles([i["seconds"] for i in items]).items()}
        summary[key] = result
    return summary


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--systems", default=",".join(SYSTEMS))
    parser.add_argument("--split", default="all")
    parser.add_argument("--corpora", default="")
    parser.add_argument("--limit", type=int, default=10)
    parser.add_argument("--out", default=None)
    args = parser.parse_args()
    out = results_dir(args.out)
    log = CommandLog(out)
    corpora = [c for c in args.corpora.split(",") if c] or None
    queries = load_queries(args.split, corpora)
    by_corpus = defaultdict(list)
    for q in queries:
        by_corpus[q["corpus"]].append(q)
    roots = {c: CORPORA / c for c in by_corpus}
    report = {"dataset": "validation/golden/symbol_navigation.jsonl", "split": args.split, "limit": args.limit,
              "token_estimate": "payload_bytes/4 (heuristic, not a model tokenizer)",
              "line_tolerance": 1, "systems": {}}
    run_tag = out.name
    for system in args.systems.split(","):
        missing = system.startswith("findex") and not findex_binary("baseline" if "baseline" in system else "current").exists()
        if missing:
            report["systems"][system] = {"status": "BLOCKED", "reason": "binary missing"}
            continue
        all_outputs, per_corpus = [], {}
        for corpus, items in by_corpus.items():
            print(f"[retrieval] {system} {corpus} ({len(items)} queries)", file=sys.stderr)
            if system == "gitgrep":
                admitted = set(admitted_files(roots[corpus]))
                outputs = []
                for q in items:
                    hits, size, seconds = gitgrep(roots[corpus], q, admitted, args.limit)
                    outputs.append({"query": q, "hits": hits, "bytes": size, "seconds": seconds, "is_error": False})
                info = {}
            else:
                outputs, info = run_findex(system, corpus, items, args.limit, log, run_tag)
            per_corpus[corpus] = {**info, **summarize(outputs, roots)["all"]}
            all_outputs.extend(outputs)
        report["systems"][system] = {"status": "EXECUTED", "overall": summarize(all_outputs, roots),
                                     "per_corpus": per_corpus}
        # Per-query evidence for audit (local only; may contain corpus paths, no secrets).
        detail = out / "raw" / f"retrieval-{system}.jsonl"
        detail.parent.mkdir(exist_ok=True)
        with detail.open("w", encoding="utf-8") as handle:
            for item in all_outputs:
                handle.write(json.dumps({"id": item["query"]["id"], "hits": item["hits"][:10], "bytes": item["bytes"],
                                         "ms": round(item["seconds"] * 1000, 3), "error": item["is_error"]}) + "\n")
    write_json(out / "retrieval.json", report)
    for system, data in report["systems"].items():
        if data.get("status") != "EXECUTED":
            print(f"{system}: {data}")
            continue
        for key in ("symbol_exact", "identifier_words", "split/heldout"):
            m = data["overall"].get(key)
            if m:
                print(f"{system:24} {key:18} n={m['queries']:3} R@1={m['file_recall@1']:.3f} R@10={m['file_recall@10']:.3f} "
                      f"MRR={m['file_mrr']:.3f} spanR@1={m['span_recall@1']:.3f} spanMRR={m['span_mrr']:.3f} "
                      f"bytes={m['payload_bytes']['mean']:.0f} p50={m['latency_ms']['p50']:.1f}ms p95={m['latency_ms']['p95']:.1f}ms")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
