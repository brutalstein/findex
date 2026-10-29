"""Frozen offline MCP payload comparison; no model, API or network calls.

Prepare before running. Historical 60 queries are regression controls; the
other 55 held-out queries are the primary set. Development queries are excluded.
"""
from __future__ import annotations

import argparse
from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import statistics

from common import REPO, percentiles, write_json
from mcp_client import McpProcess
from retrieval import score
from tool_comparison import CORPORA, GOLDEN, cbm_hits, corpus_identity, graphify_hits, superiority, tool_identity

MODES = ("findex-baseline-json", "findex-json", "findex-compact",
         "codebase-json", "codebase-tree", "graphify-mcp")


def same_hits(baseline: list[dict], current: list[dict]) -> bool:
    """Equal hit lists; bm25_score is compared at the three decimals the server now emits."""
    rounded = lambda hits: [dict(h, bm25_score=round(h["bm25_score"], 3)) if "bm25_score" in h else h for h in hits]
    return rounded(baseline) == rounded(current)


def expand_hits(payload: dict) -> dict:
    if "columns" not in payload:
        return payload
    result = dict(payload)
    if type(result.get("sync")) is int and result["sync"] == 0:
        result["sync"] = {key: 0 for key in ("examined", "indexed", "unchanged", "removed", "skipped", "cloud_only", "symbols_parsed", "symbols_reused")}
    columns = result.pop("columns")
    if len(set(columns)) != len(columns) or any(len(row) != len(columns) for row in result["hits"]):
        raise ValueError("Invalid compact columns/row width")
    result["hits"] = [dict(zip(columns, row)) for row in result["hits"]]
    return result


def cbm_tree_hits(text: str) -> list[dict]:
    hits, grouped_path = [], None
    flat = False
    for line in text.splitlines():
        if line.startswith("results:"):
            flat = "cols: qn label file lines rank" in line
        header = re.match(r"^\S.* \((.+)\):$", line)
        if header:
            grouped_path = header[1]
        if not line.startswith("  "):
            continue
        fields = shlex.split(line)
        if len(fields) != 5:
            continue
        if flat:
            path, lines = fields[2:4]
        else:
            path = grouped_path
            lines = fields[2]
        row = re.fullmatch(r"(\d+)-\d+", lines)
        if row and path:
            hits.append({"path": path.replace("\\", "/"), "line": int(row[1])})
    return hits


def identity(args) -> dict:
    return {"corpora": {c: corpus_identity(args.corpora / c) for c in CORPORA},
            "tools": {name: tool_identity(path) for name, path in (
                ("findex", args.findex), ("baseline", args.baseline),
                ("codebase-memory", args.cbm), ("graphify", args.graphify))},
            "graphs": {c: hashlib.sha256((args.work / "graphify" / c / "graphify-out/graph.json").read_bytes()).hexdigest()
                       for c in CORPORA}}


def prepare(args) -> None:
    manifest_path = args.out / "manifest.json"
    if manifest_path.exists():
        raise ValueError("Never overwrite a frozen manifest")
    historical = json.loads((args.historical / "manifest.json").read_text(encoding="utf-8"))
    old_ids = {q["id"] for q in historical["queries"]}
    queries = [json.loads(line) for line in GOLDEN.read_text(encoding="utf-8").splitlines()]
    queries = [q for q in queries if q["split"] == "heldout" and q["corpus"] in CORPORA]
    for q in queries:
        q["evaluation_set"] = "historical-60" if q["id"] in old_ids else "new-55"
    frozen = identity(args)
    for c in CORPORA:
        if {q["commit"] for q in queries if q["corpus"] == c} != {frozen["corpora"][c]["commit"]}:
            raise ValueError(f"Corpus pin mismatch: {c}")
    setups = json.loads((args.historical / "local-results.json").read_text(encoding="utf-8"))["setups"]
    write_json(manifest_path, {"queries": queries, "identity": frozen,
        "queries_sha256": hashlib.sha256(json.dumps(queries, sort_keys=True).encode()).hexdigest(),
        "golden_sha256": hashlib.sha256(GOLDEN.read_bytes()).hexdigest(),
        "projects": {s["corpus"]: s["codebase_project"] for s in setups},
        "modes": MODES, "budget_tokens": 2000, "limit": 10, "tokenizer": "o200k_base",
        "policy": "Exact symbols: normalized Findex lookup, anchored codebase regex; other queries: lexical/BM25. Graphify code-only native MCP depth=2. No tuning on these results."})
    print(f"Frozen {len(queries)} queries; {sum(q['evaluation_set'] == 'new-55' for q in queries)} new", flush=True)


def run(args) -> None:
    manifest = json.loads((args.out / "manifest.json").read_text(encoding="utf-8"))
    if any((args.out / p).exists() for p in ("raw", "results.json", "run-identity.json")):
        raise ValueError("Measurement already attempted; preserve it")
    if identity(args) != manifest["identity"]:
        raise ValueError("Source, binary or graph drift since preparation")
    if hashlib.sha256(GOLDEN.read_bytes()).hexdigest() != manifest["golden_sha256"] or hashlib.sha256(
            json.dumps(manifest["queries"], sort_keys=True).encode()).hexdigest() != manifest["queries_sha256"]:
        raise ValueError("Golden/query drift")
    import tiktoken
    encoding = tiktoken.get_encoding(manifest["tokenizer"])
    write_json(args.out / "run-identity.json", manifest["identity"])
    records = []
    for corpus in CORPORA:
        root = (args.corpora / corpus).resolve()
        raw_dir = args.out / "raw" / corpus
        raw_dir.mkdir(parents=True)
        with ExitStack() as stack:
            clients = {}
            for name, binary in (("baseline", args.baseline), ("findex", args.findex), ("cbm", args.cbm), ("graphify", args.python)):
                env = dict(os.environ)
                server_args = []
                if name in ("baseline", "findex"):
                    env["LOCALAPPDATA"] = str((args.work / "payload-cost" / name / corpus).resolve())
                    Path(env["LOCALAPPDATA"]).mkdir(parents=True, exist_ok=True)
                    server_args = ["mcp"]
                if name == "graphify":
                    server_args = ["-m", "graphify.serve", str((args.work / "graphify" / corpus / "graphify-out/graph.json").resolve())]
                clients[name] = McpProcess(binary, root, env, server_args=server_args)
                stack.callback(clients[name].close)
                if name in ("baseline", "findex"):
                    if clients[name].call_tool("findex_search", {"query": "warmup_marker", "limit": 1})[3]:
                        raise RuntimeError(f"{name} index warmup failed")
            for query in (q for q in manifest["queries"] if q["corpus"] == corpus):
                evidence = {}
                offset = int(hashlib.sha256(query["id"].encode()).hexdigest(), 16) % len(MODES)
                for mode in MODES[offset:] + MODES[:offset]:
                    exact = query["type"] == "symbol_exact"
                    if mode.startswith("findex"):
                        client = clients["baseline" if "baseline" in mode else "findex"]
                        arguments = {"name" if exact else "query": query["query"], "limit": manifest["limit"]}
                        if mode == "findex-compact":
                            arguments["format"] = "compact"
                        payload, text, seconds, error = client.call_tool("findex_find_symbol" if exact else "findex_search", arguments)
                        if mode == "findex-compact" and "columns" not in (payload or {}):
                            raise AssertionError("Production compact dispatch missing")
                        expanded = expand_hits(payload or {})
                        evidence[mode] = expanded
                        hits = [{"path": h["path"], "line": h.get("line", h.get("start_line"))} for h in expanded.get("hits", [])]
                    elif mode.startswith("codebase"):
                        arguments = {"project": manifest["projects"][corpus], "limit": manifest["limit"],
                                     "format": "tree" if mode.endswith("tree") else "json"}
                        arguments["name_pattern" if exact else "query"] = "^" + re.escape(query["query"]) + "$" if exact else query["query"]
                        payload, text, seconds, error = clients["cbm"].call_tool("search_graph", arguments)
                        hits = cbm_tree_hits(text) if mode.endswith("tree") else cbm_hits(payload or {})
                        evidence[mode] = hits
                    else:
                        _, text, seconds, error = clients["graphify"].call_tool("query_graph", {
                            "question": query["query"], "depth": 2, "token_budget": manifest["budget_tokens"]})
                        hits = graphify_hits(text)
                    (raw_dir / f"{hashlib.sha256(query['id'].encode()).hexdigest()[:16]}-{mode}.txt").write_text(text, encoding="utf-8")
                    tokens = encoding.encode(text, disallowed_special=())
                    over = len(tokens) > manifest["budget_tokens"]
                    visible = hits
                    if over:
                        visible = graphify_hits(encoding.decode(tokens[:manifest["budget_tokens"]])) if mode == "graphify-mcp" else []
                    records.append({"id": query["id"], "corpus": corpus, "type": query["type"],
                        "evaluation_set": query["evaluation_set"], "mode": mode, "raw_tokens": len(tokens),
                        "visible_tokens": min(len(tokens), manifest["budget_tokens"]), "seconds": seconds,
                        "error": error, "over_budget": over, "hits": hits[:manifest["limit"]],
                        "scores": score(query, [] if error else visible[:manifest["limit"]])})
                if evidence["findex-json"] != evidence["findex-compact"]:
                    raise AssertionError(f"Lossless roundtrip failed: {query['id']}")
                if not same_hits(evidence["findex-baseline-json"]["hits"], evidence["findex-json"]["hits"]):
                    raise AssertionError(f"Ranking/source changed: {query['id']}")
                if evidence["codebase-json"] != evidence["codebase-tree"]:
                    raise AssertionError(f"Competitor parser mismatch: {query['id']}")
                write_json(args.out / "results.json", {"records": records})
        print(f"{corpus}: {len(records)} results, lossless and competitor parser assertions passed", flush=True)
    save_summary(args.out, manifest, records)


def save_summary(out: Path, manifest: dict, records: list[dict]) -> None:
    summary = {}
    for subset in ("all-115", "new-55", "historical-60", *CORPORA):
        summary[subset] = {}
        for mode in MODES:
            rows = [r for r in records if r["mode"] == mode and (subset == "all-115" or r["evaluation_set"] == subset or r["corpus"] == subset)]
            summary[subset][mode] = {"queries": len(rows), "mean_raw_tokens": statistics.mean(r["raw_tokens"] for r in rows),
                "mean_visible_tokens": statistics.mean(r["visible_tokens"] for r in rows),
                "hit_at_1": statistics.mean(r["scores"]["file_recall@1"] for r in rows),
                "span_at_1": statistics.mean(r["scores"]["span_recall@1"] for r in rows),
                "mrr": statistics.mean(r["scores"]["file_mrr"] for r in rows),
                "latency_s": percentiles([r["seconds"] for r in rows]),
                "errors": sum(r["error"] for r in rows), "over_budget": sum(r["over_budget"] for r in rows)}
    gates = {scope: superiority({tool: dict(values[mode], mean_tokens=values[mode]["mean_raw_tokens"])
                              for tool, mode in (("findex", "findex-compact"), ("codebase-memory", "codebase-tree"), ("graphify", "graphify-mcp"))})
             for scope, values in summary.items()}
    write_json(out / "summary.json", {"summary": summary, "gate": gates["new-55"], "gates_by_scope": gates, "lossless_roundtrips": len(manifest["queries"]),
        "baseline_hit_equivalence": len(manifest["queries"]), "competitor_parser_equivalence": len(manifest["queries"]),
        "limits": ["Author-created navigation golden, four languages, no code-edit tasks", "Graphify code-only extraction", "Output tokens are not whole-session usage or billed money", "Historical 60 queries are not independent confirmation"]})
    print(json.dumps(summary["new-55"], indent=2), flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["prepare", "run"])
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--findex", type=Path, default=REPO / "target/release/findex.exe")
    parser.add_argument("--baseline", type=Path, default=REPO / "target/token-baseline/findex.exe")
    parser.add_argument("--corpora", type=Path, default=REPO / "target/tool-comparison/corpora")
    parser.add_argument("--work", type=Path, default=REPO / "target/tool-comparison/state")
    parser.add_argument("--historical", type=Path, default=REPO / "validation/results/20261026-codex-comparison")
    parser.add_argument("--cbm", type=Path, required=True)
    parser.add_argument("--graphify", type=Path, required=True)
    parser.add_argument("--python", type=Path, required=True)
    args = parser.parse_args()
    for key, value in vars(args).items():
        if isinstance(value, Path):
            setattr(args, key, value.resolve())
    args.out.mkdir(parents=True, exist_ok=True)
    {"prepare": prepare, "run": run}[args.mode](args)


if __name__ == "__main__":
    main()
