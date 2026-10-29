"""Multi-model agent evaluation harness for Findex.

Simulates different agent configurations (query style, budget, output format)
against the frozen golden datasets and reports hit@k, MRR, latency, and token
usage. No live model/API calls required — the harness drives the findex MCP
server directly with parameterized agent profiles.

Usage:
    python eval_agents.py [--corpora findex,ripgrep,requests,zod,fmt,pybind11,tokio]
                          [--split all|dev|heldout]
                          [--limit N]
                          [--out DIR]

Agent profiles:
  minimal    -- short query, budget=1200, format=text   (fast, low token)
  balanced   -- medium query, budget=2000, format=text   (default recommendation)
  thorough   -- long query, budget=4000, format=json     (structured evidence)
  strict     -- short query, budget=800,  format=text    (aggressive budget)

Writes eval_agents.json into the shared results directory.
"""
from __future__ import annotations

import argparse
import json
import shutil
import sys
import time
from collections import defaultdict
from pathlib import Path

from common import CORPORA, REPO, VALIDATION, WORK, CommandLog, findex_binary, isolated_env, percentiles, results_dir, write_json
from mcp_client import McpProcess

# Agent profiles: (name, query_template, budget_tokens, format, tool)
PROFILES = [
    ("minimal",  "{query}", 1200, "text", "findex_context"),
    ("balanced", "{query}", 2000, "text", "findex_context"),
    ("thorough", "{query}", 4000, "json", "findex_context"),
    ("strict",   "{query}",  800, "text", "findex_context"),
]

GOLDEN = VALIDATION / "golden" / "symbol_navigation.jsonl"


def load_queries(split: str, corpora: list[str] | None, limit: int | None) -> list[dict]:
    rows = [json.loads(line) for line in GOLDEN.open(encoding="utf-8")]
    rows = [r for r in rows if (split == "all" or r["split"] == split)]
    if corpora:
        rows = [r for r in rows if r["corpus"] in corpora]
    if limit:
        rows = rows[:limit]
    return rows


def score(query: dict, hits: list[dict]) -> dict:
    """Same scoring as retrieval.py: file/span recall@k, MRR, nDCG."""
    import math
    expected = [tuple(e) for e in query["expected"]]
    paths = {p for p, _ in expected}
    file_rel = [h["path"] in paths for h in hits]
    span_rel = [any(h["path"] == p and abs(h["line"] - line) <= 1 for p, line in expected)
                for h in hits]
    result: dict[str, float] = {}
    for k in (1, 3, 5, 10):
        result[f"file_recall@{k}"] = float(any(file_rel[:k]))
        result[f"span_recall@{k}"] = float(any(span_rel[:k]))
        distinct = list(dict.fromkeys(h["path"] for h in hits[:k]))
        result[f"file_precision@{k}"] = sum(p in paths for p in distinct) / k
    result["file_mrr"] = next((1 / (i + 1) for i, r in enumerate(file_rel) if r), 0.0)
    result["span_mrr"] = next((1 / (i + 1) for i, r in enumerate(span_rel) if r), 0.0)
    first_ranks = {}
    for i, hit in enumerate(hits[:10]):
        first_ranks.setdefault(hit["path"], i)
    dcg = sum(1 / math.log2(i + 2) for p, i in first_ranks.items() if p in paths)
    ideal = sum(1 / math.log2(i + 2) for i in range(min(len(paths), 10)))
    result["file_ndcg@10"] = dcg / ideal if ideal else 0.0
    result["empty"] = float(not hits)
    result["top1_false_positive"] = float(bool(hits) and not file_rel[0])
    return result


def run_profile(client: McpProcess, profile: tuple, query: dict, limit: int) -> dict:
    name, template, budget, fmt, tool = profile
    qtext = template.format(query=query["query"])
    started = time.perf_counter()
    payload, text, seconds, is_error = client.call_tool(
        tool,
        {"task": qtext, "budget_tokens": budget, "format": fmt},
        timeout=600.0,
    )
    elapsed = time.perf_counter() - started
    hits: list[dict] = []
    if payload and isinstance(payload, dict) and "evidence" in payload:
        # JSON structured output (ContextReport.render_json)
        for e in payload["evidence"]:
            hits.append({
                "path": e.get("path", ""),
                "line": e.get("line", 0),
                "kind": e.get("kind", ""),
                "relevance_score": e.get("relevance_score", 0.0),
            })
    elif payload and isinstance(payload, dict) and "hits" in payload:
        # Symbol/search payload
        for h in payload.get("hits", []):
            hits.append({
                "path": h.get("path", ""),
                "line": h.get("start_line", h.get("line", 0)),
                "kind": h.get("match_kind", "match"),
                "relevance_score": h.get("score", 0.0),
            })
    elif fmt == "text" and text and not is_error:
        # Plain text output: parse "kind path:line[-end]" lines
        for line in text.splitlines():
            line = line.strip()
            if not line or line.startswith(("intent:", "kinds:", "shown:", "missing:")):
                continue
            parts = line.split(None, 2)
            if len(parts) >= 2:
                kind = parts[0].rstrip(" test")
                path_colon = parts[1]
                if ":" in path_colon:
                    path, line_part = path_colon.rsplit(":", 1)
                    if not line_part:
                        continue
                    # line_part may be "27-31" or "27"
                    try:
                        line_num = int(line_part.split("-")[0])
                    except ValueError:
                        continue
                    hits.append({
                        "path": path,
                        "line": line_num,
                        "kind": kind,
                        "relevance_score": 0.5,
                    })
    text_len = len(text.encode("utf-8"))
    return {
        "profile": name,
        "format": fmt,
        "budget": budget,
        "hits": hits[:limit],
        "bytes": text_len,
        "seconds": round(elapsed, 4),
        "is_error": is_error,
        "est_tokens": round(text_len / 4, 1),
    }


def summarize(results: list[dict]) -> dict:
    groups: dict[str, list[dict]] = defaultdict(list)
    for r in results:
        groups[r["profile"]].append(r)
    summary = {}
    for profile, items in sorted(groups.items()):
        metrics = defaultdict(float)
        n = len(items)
        for item in items:
            for name, value in score(item["query"], item["hits"]).items():
                metrics[name] += value
        profile_summary = {name: round(value / n, 4) for name, value in metrics.items()}
        profile_summary["queries"] = n
        profile_summary["errors"] = sum(1 for i in items if i.get("is_error"))
        profile_summary["bytes_mean"] = round(sum(i["bytes"] for i in items) / n, 1)
        profile_summary["tokens_mean"] = round(sum(i["est_tokens"] for i in items) / n, 1)
        profile_summary["latency_ms"] = percentiles([i["seconds"] for i in items])
        # Carry format/budget from the first item
        if items:
            profile_summary["format"] = items[0]["format"]
            profile_summary["budget"] = items[0]["budget"]
        summary[profile] = profile_summary
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description="Multi-model agent evaluation for Findex")
    parser.add_argument("--corpora", default="",
                        help="Comma-separated corpus filter (default: all)")
    parser.add_argument("--split", default="all",
                        help="all|dev|heldout (default: all)")
    parser.add_argument("--limit", type=int, default=None,
                        help="Max queries (default: all)")
    parser.add_argument("--out", default=None, help="Output directory")
    args = parser.parse_args()

    corpora = [c for c in args.corpora.split(",") if c] or None
    queries = load_queries(args.split, corpora, args.limit)
    if not queries:
        print("No queries matched", file=sys.stderr)
        return 1

    out = results_dir(args.out)
    log = CommandLog(out)
    by_corpus: dict[str, list[dict]] = defaultdict(list)
    for q in queries:
        by_corpus[q["corpus"]].append(q)
    roots = {c: CORPORA / c for c in by_corpus}

    exe = findex_binary("current")
    if not exe.exists():
        print(f"Findex binary not found: {exe}", file=sys.stderr)
        print("Build with: cargo build --release -p findex", file=sys.stderr)
        return 1

    report = {
        "dataset": str(GOLDEN),
        "split": args.split,
        "profiles": [p[0] for p in PROFILES],
        "systems": {},
        "note": "No model/API calls. Agent profiles drive the MCP server directly.",
    }

    for system_name in ["findex-current"]:
        exe_path = exe
        state = WORK / "state" / "eval_agents" / system_name
        if state.exists():
            shutil.rmtree(state)
        env = isolated_env(state)
        started = time.perf_counter()

        # Warm-up: index findex repo itself, then run one context call
        client = McpProcess(exe_path, REPO, env)
        _, _, _, _ = client.call_tool("findex_context",
                                      {"task": "warmup", "budget_tokens": 200, "format": "text"})
        client.close()

        all_results: list[dict] = []
        per_corpus: dict[str, dict] = {}
        for corpus, items in by_corpus.items():
            root = roots[corpus]
            cstate = WORK / "state" / "eval_agents" / system_name / corpus
            if cstate.exists():
                shutil.rmtree(cstate)
            cenv = isolated_env(cstate)
            cclient = McpProcess(exe_path, root, cenv)
            # Index the corpus
            idx = log.run([str(exe_path), "index"], cwd=root, env=cenv, timeout=900)
            if idx.returncode:
                raise RuntimeError(f"Index failed for {corpus}: {idx.stderr[-500:]}")
            for query in items:
                for profile in PROFILES:
                    result = run_profile(cclient, profile, query, limit=10)
                    result["query"] = query
                    result["corpus"] = corpus
                    all_results.append(result)
            cpeak = cclient.memory()
            ccode = cclient.close()
            log.record([str(exe_path), "mcp", corpus], root, ccode, 0.0,
                       note=f"{system_name} {corpus} peak_memory={cpeak}")

        ccode = client.close()
        elapsed = time.perf_counter() - started

        summary = summarize(all_results)
        report["systems"][system_name] = {
            "status": "EXECUTED",
            "elapsed_seconds": round(elapsed, 2),
            "overall": summary,
            "per_corpus": {},
        }
        for corpus in by_corpus:
            corpus_items = [r for r in all_results if r["corpus"] == corpus]
            report["systems"][system_name]["per_corpus"][corpus] = summarize(corpus_items)

        # Per-query detail
        detail = out / "raw" / "eval_agents.jsonl"
        detail.parent.mkdir(exist_ok=True)
        with detail.open("w", encoding="utf-8") as handle:
            for item in all_results:
                handle.write(json.dumps({
                    "id": item["query"]["id"],
                    "profile": item["profile"],
                    "corpus": item["corpus"],
                    "hits": item["hits"][:5],
                    "ms": item["seconds"] * 1000,
                    "bytes": item["bytes"],
                    "tokens": item["est_tokens"],
                    "error": item["is_error"],
                }) + "\n")

    write_json(out / "eval_agents.json", report)

    # Print summary
    print("\n=== Multi-Model Agent Evaluation Summary ===\n")
    for system, data in report["systems"].items():
        if data["status"] != "EXECUTED":
            continue
        print(f"System: {system}  (elapsed {data['elapsed_seconds']}s)")
        for profile, m in data["overall"].items():
            print(f"  {profile:12} fmt={m.get('format','?')} budget={m.get('budget','?')} "
                  f"n={m['queries']} R@1={m['file_recall@1']:.3f} R@5={m['file_recall@5']:.3f} "
                  f"R@10={m['file_recall@10']:.3f} MRR={m['file_mrr']:.3f} "
                  f"spanR@1={m['span_recall@1']:.3f} empty={m['empty']:.3f} "
                  f"tokens={m['tokens_mean']:.0f} p50ms={m['latency_ms'].get('p50',0):.0f}")
    print(f"\nResults written to: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
