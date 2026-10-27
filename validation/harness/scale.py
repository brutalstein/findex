"""Offline native scaling check: paired builds, resources, warm queries and cap rollback.

Cold means a fresh index, not flushed OS/antivirus caches. Synthetic Rust files
isolate file-count scaling; use the public corpora comparison for language quality.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import tempfile
import time

from common import REPO, CommandLog, environment, percentiles, write_json
from mcp_client import McpProcess
from payload_cost import expand_hits
from perf import measured_run


def contents(number: int, changed: bool = False) -> str:
    return f"pub fn {'changed' if changed else 'scale'}_marker_{number:06}() -> u32 {{ {number} }}\n"


def warm_queries(binary: Path, project: Path, env: dict, size: int) -> dict:
    client = McpProcess(binary, project, env)
    records = []
    try:
        _, _, first_s, error = client.call_tool("findex_search", {"query": "warmup_marker", "limit": 1})
        assert not error, "initial MCP reconciliation failed"
        for rep in range(5):
            for number in (0, size // 2, size - 1):
                for tool, field in (("findex_find_symbol", "name"), ("findex_search", "query")):
                    payload, text, seconds, error = client.call_tool(tool, {field: f"scale_marker_{number:06}", "limit": 1, "format": "compact"})
                    expanded = expand_hits(payload or {})
                    assert not error and len(expanded.get("hits", [])) == 1, (tool, number)
                    hit = expanded["hits"][0]
                    assert hit["path"] == f"pkg/{number:06}.rs" and hit.get("line", hit.get("start_line")) == 1, hit
                    assert expanded["sync"]["examined"] == 0, "warm query unexpectedly traversed the project"
                    records.append({"kind": tool, "number": number, "rep": rep, "seconds": seconds, "chars": len(text), "hits": expanded["hits"]})
            payload, text, seconds, error = client.call_tool("findex_search", {"query": "pub", "limit": 8, "format": "compact"})
            expanded = expand_hits(payload or {})
            assert not error and len(expanded.get("hits", [])) == 8, "common term lost evidence"
            assert expanded["sync"]["examined"] == 0
            assert expanded["truncated"] is True, "common-term candidate cap must remain explicit"
            records.append({"kind": "common-term", "rep": rep, "seconds": seconds, "chars": len(text), "hits": expanded["hits"]})
        cpu_before = client.cpu_seconds()
        time.sleep(5)
        return {"first_query_incl_watch_sync_s": first_s, "records": records,
                "latency_s": {kind: percentiles([r["seconds"] for r in records if r["kind"] == kind])
                              for kind in ("findex_find_symbol", "findex_search", "common-term")},
                "idle_cpu_seconds_per_5s": client.cpu_seconds() - cpu_before,
                "memory_after_queries": client.memory(), "threads_after_queries": client.ps.num_threads()}
    finally:
        client.close()


def cap_rollback(binary: Path, project: Path, state: Path, env: dict, log: CommandLog) -> dict:
    database, = state.rglob("index.sqlite3")
    paths = [f"pkg/{i:06}.rs" for i in range(9)]
    with closing(sqlite3.connect(database)) as db:
        before = dict(db.execute("SELECT path, hash FROM files WHERE path IN (" + ",".join("?" for _ in paths) + ")", paths))
    assert len(before) == 9
    # At most eight entries can be pending at the stop. Nine edits ensure at
    # least one partial write is attempted regardless of directory enumeration.
    for number in range(9):
        (project / paths[number]).write_text(contents(number, changed=True), encoding="utf-8")
    extra = project / "pkg/100000.rs"
    extra.write_text(contents(100000), encoding="utf-8")
    rejected = log.run([str(binary), "index"], cwd=project, env=env, timeout=600)
    assert rejected.returncode != 0 and "100000-file safety budget" in rejected.stderr, rejected.stderr
    with closing(sqlite3.connect(database)) as db:
        after = dict(db.execute("SELECT path, hash FROM files WHERE path IN (" + ",".join("?" for _ in paths) + ")", paths))
        assert db.execute("SELECT count(*) FROM files").fetchone()[0] == 100000
    assert before == after, "over-budget traversal committed a partial snapshot"
    integrity = log.run([str(binary), "index-check"], cwd=project, env=env, timeout=120)
    assert integrity.returncode == 0, integrity.stderr
    extra.unlink()
    recovered = log.run([str(binary), "index"], cwd=project, env=env, timeout=600)
    assert recovered.returncode == 0 and "Indexed 9 changed" in recovered.stdout, recovered.stdout + recovered.stderr
    return {"status": "PASS", "rejected_exit": rejected.returncode, "old_hashes_preserved": len(before),
            "index_integrity": "PASS", "recovery": "9 changes admitted after removing excess file"}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, default=REPO / "target/release/findex.exe")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out = args.out.resolve()
    if args.out.exists():
        raise ValueError("Preserve existing scaling attempts; select a new directory")
    args.out.mkdir(parents=True)
    binaries = {"baseline": args.baseline.resolve(strict=True), "candidate": args.candidate.resolve(strict=True)}
    hashes = {role: hashlib.sha256(path.read_bytes()).hexdigest() for role, path in binaries.items()}
    sizes = (200, 2000, 10000, 100000)
    write_json(args.out / "manifest.json", {"binary_sha256": hashes,
        "sizes": sizes, "paired_repetitions": 3, "100k": "candidate only, one repetition; no baseline speedup claim",
        "recipe": "pkg/{i:06}.rs: pub fn scale_marker_{i:06}() -> u32 { i } newline", "environment": environment(),
        "model_sessions": 0, "api_calls": 0, "conditions": "Separate identical source directories and fresh databases per build/repetition; OS and antivirus caches not flushed; order alternates"})
    log = CommandLog(args.out)
    report = {"sizes": {}}
    with tempfile.TemporaryDirectory(prefix="findex-scale-") as folder:
        root = Path(folder)
        for role, source in list(binaries.items()):
            frozen = root / "bin" / role / "findex.exe"
            frozen.parent.mkdir(parents=True)
            shutil.copy2(source, frozen)
            assert hashlib.sha256(frozen.read_bytes()).hexdigest() == hashes[role], "binary changed during freezing"
            binaries[role] = frozen
        for size in sizes:
            projects = {}
            entry = {role: {"cold": []} for role in (binaries if size < 100000 else ("candidate",))}
            repetitions = 3 if size < 100000 else 1
            for rep in range(repetitions):
                order = list(entry) if rep % 2 == 0 else list(reversed(entry))
                for role in order:
                    project = root / str(size) / f"source-{role}-{rep}"
                    (project / "pkg").mkdir(parents=True)
                    (project / ".git").mkdir()
                    for number in range(size):
                        (project / "pkg" / f"{number:06}.rs").write_text(contents(number), encoding="utf-8")
                    projects[(role, rep)] = project
                    state = root / str(size) / f"state-{role}-{rep}"
                    state.mkdir()
                    env = dict(os.environ, LOCALAPPDATA=str(state))
                    result = measured_run([str(binaries[role]), "index"], project, env, log, timeout=1200)
                    assert result["exit_code"] == 0 and f"Indexed {size} changed" in result["stdout_tail"], result
                    entry[role]["cold"].append(result)
                    report["sizes"][str(size)] = entry
                    write_json(args.out / "results.json", report)
                    print(f"{size} {role} rep={rep}: {result['wall_s']:.3f}s, peak={result.get('peak_working_set', 0)/2**20:.2f} MiB", flush=True)
            for role in entry:
                project = projects[(role, 0)]
                state = root / str(size) / f"state-{role}-0"
                env = dict(os.environ, LOCALAPPDATA=str(state))
                entry[role]["cold_s"] = percentiles([r["wall_s"] for r in entry[role]["cold"]])
                entry[role]["warm"] = warm_queries(binaries[role], project, env, size)
                print(f"{size} {role}: warm queries and idle check passed", flush=True)
                write_json(args.out / "results.json", report)
            if "baseline" in entry:
                a, b = entry["baseline"]["warm"]["records"], entry["candidate"]["warm"]["records"]
                assert [(r["kind"], r.get("number"), r["hits"]) for r in a] == [(r["kind"], r.get("number"), r["hits"]) for r in b], "retrieval changed between builds"
            else:
                project = projects[("candidate", 0)]
                state = root / str(size) / "state-candidate-0"
                entry["cap_rollback"] = cap_rollback(binaries["candidate"], project, state, dict(os.environ, LOCALAPPDATA=str(state)), log)
                write_json(args.out / "results.json", report)
            print(f"{size}: source checks passed", flush=True)
    write_json(args.out / "completion.json", {"status": "PASS", "sizes": sizes, "scope": "Synthetic Rust file-count scaling; no general all-project claim"})


if __name__ == "__main__":
    main()
