"""Frozen, offline native-tool comparison. No model sessions or API calls.

Prepare once, review manifest.json, then run against the same manifest.
Graphify uses its documented code-only extraction and query CLI; Findex and
codebase-memory use persistent MCP processes. Latencies include those surfaces.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

from common import REPO, VALIDATION, percentiles, write_json
from golden import admitted_files
from mcp_client import McpProcess
from retrieval import score

TOOLS = ("findex", "graphify", "codebase-memory")
CORPORA = ("requests", "zod", "fmt", "ripgrep")
GOLDEN = VALIDATION / "golden/symbol_navigation.jsonl"
SEED = "findex-three-tools-v1"


def select_queries(rows: list[dict], corpora: list[str], per_corpus: int) -> list[dict]:
    selected = []
    for corpus in corpora:
        groups = defaultdict(list)
        for row in rows:
            if row["corpus"] == corpus and row["split"] == "heldout":
                groups[row["type"]].append(row)
        for group in groups.values():
            group.sort(key=lambda r: hashlib.sha256((SEED + r["id"]).encode()).hexdigest())
        chosen = []
        while len(chosen) < per_corpus:
            available = [groups[k] for k in sorted(groups) if groups[k]]
            if not available:
                raise ValueError(f"Not enough held-out queries for {corpus}")
            for group in available:
                if len(chosen) < per_corpus:
                    chosen.append(group.pop(0))
        selected.extend(chosen)
    return selected


def cbm_hits(payload: dict) -> list[dict]:
    columns = payload.get("cols", [])
    hits = []
    groups = payload.get("groups", [{"rows": payload.get("rows", [])}])
    for group in groups:
        for values in group.get("rows", []):
            row = dict(zip(columns, values))
            path = row.get("file", group.get("file"))
            lines = str(row.get("lines", ""))
            if path and re.match(r"\d+", lines):
                hits.append({"path": path.replace("\\", "/"), "line": int(re.match(r"\d+", lines)[0])})
    return hits


def graphify_hits(text: str) -> list[dict]:
    return [{"path": m[0].replace("\\", "/"), "line": int(m[1])}
            for m in re.findall(r"^NODE .*?\[src=(.*?) loc=L(\d+)(?=\s)", text, re.M)]


def superiority(summary: dict) -> dict:
    findex = summary["findex"]
    checks = {other: {
        "lower_context_tokens": findex["mean_tokens"] < summary[other]["mean_tokens"],
        "higher_file_hit_at_1": findex["hit_at_1"] > summary[other]["hit_at_1"],
        "higher_span_hit_at_1": findex["span_at_1"] > summary[other]["span_at_1"],
    } for other in TOOLS if other != "findex"}
    return {"vs": checks, "local_pass": all(all(v.values()) for v in checks.values()),
            "overall": "NOT_ESTABLISHED", "reason": "Live task success and actual session usage are separate; a small pilot cannot establish general superiority."}


def invoke(argv: list[str], cwd: Path, timeout: float = 600) -> tuple[str, float]:
    started = time.perf_counter()
    result = subprocess.run(argv, cwd=cwd, capture_output=True, text=True,
                            encoding="utf-8", errors="replace", timeout=timeout)
    if result.returncode:
        raise RuntimeError(f"Command failed ({result.returncode}): {result.stderr[-1200:]}")
    return result.stdout, time.perf_counter() - started


def disk_bytes(path: Path) -> int | None:
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file()) if path.exists() else None


def corpus_identity(root: Path) -> dict:
    head, _ = invoke(["git", "rev-parse", "HEAD"], root)
    dirty, _ = invoke(["git", "status", "--porcelain", "--untracked-files=all"], root)
    if dirty.strip():
        raise ValueError(f"Corpus has modified or untracked files: {root.name}")
    tracked, _ = invoke(["git", "ls-files", "-z"], root)
    for relative in filter(None, tracked.split("\0")):
        path = root / relative
        if not path.resolve().is_relative_to(root.resolve()) or path.is_symlink():
            raise ValueError(f"Corpus contains a source link: {relative}")
    hashes = {}
    for relative in sorted(admitted_files(root)):
        path = root / relative
        hashes[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    return {"commit": head.strip(), "admitted_sha256": hashes}


def tool_identity(path: Path) -> dict:
    path = path.resolve()
    version, _ = invoke([str(path), "--version"], REPO, timeout=30)
    if not version.strip():
        raise ValueError(f"Tool did not report its version: {path.name}")
    return {"version": version.strip(), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def comparison_identity(args) -> dict:
    return {"corpora": {c: corpus_identity(args.corpora / c) for c in CORPORA},
            "tools": {tool: tool_identity(path) for tool, path in (
                ("findex", args.findex), ("graphify", args.graphify), ("codebase-memory", args.cbm))}}


def require_frozen_identity(manifest: dict, identity: dict, out: Path) -> None:
    if "identity" not in manifest:
        raise ValueError("Historical manifest has no measured source/tool identity; preserve it and prepare a new output directory with --findex, --cbm and --graphify")
    if manifest["identity"] != identity:
        raise ValueError("Corpus content or tool version/hash changed since preparation")
    if any((out / name).exists() for name in ("local-results.json", "local-summary.json", "run-identity.json", "raw")):
        raise ValueError("Output directory already contains an attempted measurement; use a new prepared directory")


def prepare(args) -> None:
    if (args.out / "manifest.json").exists():
        raise ValueError("Manifest already exists; use a new output directory rather than replacing frozen tasks")
    rows = [json.loads(line) for line in GOLDEN.read_text(encoding="utf-8").splitlines()]
    queries = select_queries(rows, list(CORPORA), 15)
    identity = comparison_identity(args)
    commits = {}
    for corpus in CORPORA:
        head = identity["corpora"][corpus]["commit"]
        expected = {r["commit"] for r in queries if r["corpus"] == corpus}
        if expected != {head}:
            raise ValueError(f"Corpus pin mismatch: {corpus}")
        commits[corpus] = head
    manifest = {"selection": SEED, "golden_sha256": hashlib.sha256(GOLDEN.read_bytes()).hexdigest(),
                "queries": queries, "corpus_commits": commits, "identity": identity, "tokenizer": "o200k_base",
                "budget_tokens": 2000, "candidate_limit": 10,
                "queries_sha256": hashlib.sha256(json.dumps(queries, sort_keys=True).encode()).hexdigest()}
    write_json(args.out / "manifest.json", manifest)
    print(f"Frozen {len(queries)} queries: {manifest['queries_sha256']}", flush=True)


def run(args) -> None:
    manifest = json.loads((args.out / "manifest.json").read_text(encoding="utf-8"))
    if "identity" not in manifest:
        require_frozen_identity(manifest, {}, args.out)
    identity = comparison_identity(args)
    require_frozen_identity(manifest, identity, args.out)
    if hashlib.sha256(GOLDEN.read_bytes()).hexdigest() != manifest["golden_sha256"]:
        raise ValueError("Golden answers changed since preparation")
    if hashlib.sha256(json.dumps(manifest["queries"], sort_keys=True).encode()).hexdigest() != manifest["queries_sha256"]:
        raise ValueError("Frozen queries changed")
    import tiktoken
    write_json(args.out / "run-identity.json", identity)
    encoding = tiktoken.get_encoding(manifest["tokenizer"])
    budget, limit = manifest["budget_tokens"], manifest["candidate_limit"]
    records, setups = [], []
    run_name = args.out.name
    for corpus in CORPORA:
        root = (args.corpora / corpus).resolve()
        head, _ = invoke(["git", "rev-parse", "HEAD"], root)
        if head.strip() != manifest["corpus_commits"][corpus]:
            raise ValueError(f"Corpus revision changed: {corpus}")
        graph_out = args.work / "graphify" / corpus
        graph_out.mkdir(parents=True, exist_ok=True)
        if (root / "graphify-out").exists():
            raise ValueError("Generated graph inside corpus would contaminate the comparison")
        graph_text, graph_s = invoke([str(args.graphify), "extract", str(root), "--code-only",
                                     "--max-workers", "2", "--out", str(graph_out)], REPO)
        (args.out / "raw" / corpus).mkdir(parents=True, exist_ok=True)
        (args.out / "raw" / corpus / "graphify-build.txt").write_text(graph_text, encoding="utf-8")
        graph = graph_out / "graphify-out/graph.json"
        env = dict(os.environ, LOCALAPPDATA=str((args.work / "findex" / corpus).resolve()))
        Path(env["LOCALAPPDATA"]).mkdir(parents=True, exist_ok=True)
        cbm_name = re.sub(r"[^a-zA-Z0-9_-]", "-", f"findex-bench-{run_name}-{corpus}")
        findex = McpProcess(args.findex, root, env)
        cbm = McpProcess(args.cbm, root, dict(os.environ), server_args=[])
        try:
            _, _, findex_s, findex_error = findex.call_tool("findex_search", {"query": "warmup_marker", "limit": 1})
            cbm_index, _, cbm_s, cbm_error = cbm.call_tool("index_repository", {
                "repo_path": str(root), "name": cbm_name, "mode": "full", "persistence": False})
            if findex_error or cbm_error:
                raise RuntimeError(f"Index setup failed: findex={findex_error}, codebase={cbm_index}")
            setups.append({"corpus": corpus, "findex_s": findex_s, "graphify_s": graph_s,
                           "codebase_memory_s": cbm_s, "codebase_project": cbm_name,
                           "codebase_index": cbm_index,
                           "findex_disk_bytes": disk_bytes(Path(env["LOCALAPPDATA"])),
                           "graphify_disk_bytes": disk_bytes(graph_out), "codebase_disk_bytes": None})
            for query in [q for q in manifest["queries"] if q["corpus"] == corpus]:
                # Rotate order deterministically; never select outputs using gold answers.
                offset = int(hashlib.sha256(query["id"].encode()).hexdigest(), 16) % len(TOOLS)
                for tool in TOOLS[offset:] + TOOLS[:offset]:
                    error, hits, text, seconds = False, [], "", 0.0
                    try:
                        if tool == "findex":
                            exact = query["type"] == "symbol_exact"
                            payload, text, seconds, error = findex.call_tool(
                                "findex_find_symbol" if exact else "findex_search",
                                {("name" if exact else "query"): query["query"], "limit": limit})
                            hits = [{"path": h["path"], "line": h.get("line", h.get("start_line"))}
                                    for h in (payload or {}).get("hits", [])]
                        elif tool == "codebase-memory":
                            payload, text, seconds, error = cbm.call_tool("search_graph", {
                                "project": cbm_name, "query": query["query"], "limit": limit, "format": "json"})
                            hits = cbm_hits(payload or {})
                        else:
                            text, seconds = invoke([str(args.graphify), "query", query["query"],
                                                    "--graph", str(graph), "--budget", str(budget)], root)
                            hits = graphify_hits(text)
                    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as exc:
                        error, text = True, str(exc)
                    encoded = encoding.encode(text, disallowed_special=())
                    tokens = len(encoded)
                    over_budget = tokens > budget
                    unbounded = score(query, hits[:limit])
                    if over_budget and tool == "graphify":
                        hits = graphify_hits(encoding.decode(encoded[:budget]))
                    elif over_budget:
                        hits = []  # An incomplete JSON response is not parseable.
                    if error:
                        hits = []
                    filename = hashlib.sha256(query["id"].encode()).hexdigest()[:16]
                    (args.out / "raw" / corpus / f"{filename}-{tool}.txt").write_text(text, encoding="utf-8")
                    records.append({"id": query["id"], "corpus": corpus, "type": query["type"],
                                    "tool": tool, "seconds": seconds, "tokens": min(tokens, budget),
                                    "raw_tokens": tokens, "error": error, "over_budget": over_budget,
                                    "hits": hits[:limit], "scores": score(query, hits[:limit]),
                                    "scores_unbounded": unbounded})
                write_json(args.out / "local-results.json", {"setups": setups, "records": records})
            print(f"{corpus}: {len(records)} tool/query results recorded", flush=True)
        finally:
            findex.close()
            cbm.close()
    save_summary(args.out, records)


def save_summary(out: Path, records: list[dict]) -> None:
    summary = {}
    for tool in TOOLS:
        rows = [r for r in records if r["tool"] == tool]
        summary[tool] = {"queries": len(rows), "hit_at_1": statistics.mean(r["scores"]["file_recall@1"] for r in rows),
                         "span_at_1": statistics.mean(r["scores"]["span_recall@1"] for r in rows),
                         "uncapped_hit_at_1": statistics.mean(r["scores_unbounded"]["file_recall@1"] for r in rows),
                         "hit_at_10": statistics.mean(r["scores"]["file_recall@10"] for r in rows),
                         "mrr": statistics.mean(r["scores"]["file_mrr"] for r in rows),
                         "mean_tokens": statistics.mean(r["tokens"] for r in rows),
                         "mean_raw_tokens": statistics.mean(r["raw_tokens"] for r in rows),
                         "latency_s": percentiles([r["seconds"] for r in rows]),
                         "errors": sum(r["error"] for r in rows), "over_budget": sum(r["over_budget"] for r in rows)}
    write_json(out / "local-summary.json", {"summary": summary, "gate": superiority(summary),
               "limits": ["Graphify code-only, not semantic extraction", "Native surfaces differ; Graphify CLI includes process startup",
                          "mean_tokens is clipped visible output; mean_raw_tokens is actual returned output",
                          "Context tokens are a proxy, not session usage or billed money", "No codebase disk-size measurement"]})
    print(json.dumps(summary, indent=2), flush=True)


def rescore(args) -> None:
    import tiktoken
    manifest = json.loads((args.out / "manifest.json").read_text(encoding="utf-8"))
    data = json.loads((args.out / "local-results.json").read_text(encoding="utf-8"))
    queries = {q["id"]: q for q in manifest["queries"]}
    encoding = tiktoken.get_encoding(manifest["tokenizer"])
    for row in data["records"]:
        name = hashlib.sha256(row["id"].encode()).hexdigest()[:16]
        text = (args.out / "raw" / row["corpus"] / f"{name}-{row['tool']}.txt").read_text(encoding="utf-8")
        if row["tool"] == "graphify":
            raw_hits = graphify_hits(text)
            visible = encoding.decode(encoding.encode(text, disallowed_special=())[:manifest["budget_tokens"]])
            row["hits"] = [] if row["error"] else graphify_hits(visible)[:manifest["candidate_limit"]]
        else:
            raw_hits = row["hits"]
        row["scores_unbounded"] = score(queries[row["id"]], raw_hits[:manifest["candidate_limit"]])
        row["scores"] = score(queries[row["id"]], row["hits"])
    write_json(args.out / "local-results.json", data)
    save_summary(args.out, data["records"])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["prepare", "run", "rescore"])
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--corpora", type=Path, default=REPO / "target/tool-comparison/corpora")
    parser.add_argument("--work", type=Path, default=REPO / "target/tool-comparison/state")
    parser.add_argument("--findex", type=Path, default=REPO / "target/release/findex.exe")
    parser.add_argument("--cbm", type=Path)
    parser.add_argument("--graphify", type=Path)
    args = parser.parse_args()
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    args.work = args.work.resolve()
    for name in ("findex", "cbm", "graphify"):
        if getattr(args, name) is not None:
            setattr(args, name, getattr(args, name).resolve())
    if args.mode in ("prepare", "run") and (args.cbm is None or args.graphify is None):
        parser.error("prepare/run require --cbm and --graphify native executable paths to freeze and verify identity")
    {"prepare": prepare, "run": run, "rescore": rescore}[args.mode](args)


if __name__ == "__main__":
    main()
