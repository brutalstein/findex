"""Release-build performance and resource measurements on this machine.

Usage: perf.py [--systems baseline,current] [--corpora ripgrep,tokio,vscode] [--reps 3] [--quick]
Writes performance.json, memory.json and cpu.json into the results directory.
"""
from __future__ import annotations

import argparse
import json
import random
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

import psutil

from common import CORPORA, VALIDATION, WORK, CommandLog, findex_binary, isolated_env, percentiles, results_dir, write_json
from golden import admitted_files, definitions
from mcp_client import McpProcess


def dir_size(path: Path) -> int:
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())


def measured_run(argv: list[str], cwd: Path, env: dict, log: CommandLog, timeout: float = 1800) -> dict:
    """Run to completion while sampling peak working set, CPU and I/O."""
    started = time.perf_counter()
    proc = subprocess.Popen(argv, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    handle = psutil.Process(proc.pid)
    sample: dict = {}
    while proc.poll() is None:
        try:
            with handle.oneshot():
                mem, cpu, io = handle.memory_info(), handle.cpu_times(), handle.io_counters()
            sample = {"peak_working_set": getattr(mem, "peak_wset", mem.rss), "cpu_user_s": cpu.user,
                      "cpu_system_s": cpu.system, "read_bytes": io.read_bytes, "write_bytes": io.write_bytes}
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            break
        if time.perf_counter() - started > timeout:
            proc.kill()
            break
        time.sleep(0.02)
    out, err = proc.communicate()
    seconds = time.perf_counter() - started
    log.record(argv, cwd, proc.returncode, seconds)
    return {"wall_s": seconds, "exit_code": proc.returncode, **sample,
            "stdout_tail": out.decode("utf-8", "replace")[-300:], "stderr_tail": err.decode("utf-8", "replace")[-300:],
            "note": "resource counters sampled every 20 ms; final <20 ms may be unobserved"}


def query_set(corpus: str, count: int) -> list[str]:
    golden = VALIDATION / "golden" / "symbol_navigation.jsonl"
    rows = [json.loads(line) for line in golden.open(encoding="utf-8")]
    queries = [r["query"] for r in rows if r["corpus"] == corpus]
    if not queries:  # corpora without golden labels: sample defined names
        names = sorted(definitions(CORPORA / corpus, admitted_files(CORPORA / corpus)))
        queries = random.Random(7).sample(names, min(count, len(names)))
    return queries[:count]


def warm_latency(exe: Path, root: Path, env: dict, queries: list[str]) -> dict:
    started = time.perf_counter()
    client = McpProcess(exe, root, env)
    init_s = time.perf_counter() - started
    _, _, first_s, first_error = client.call_tool("findex_search", {"query": "warmup_marker", "limit": 8})
    search, symbol, errors = [], [], 0
    for query in queries:
        _, _, seconds, is_error = client.call_tool("findex_search", {"query": query, "limit": 8})
        search.append(seconds * 1000)
        errors += is_error
        if " " not in query:
            _, _, seconds, is_error = client.call_tool("findex_find_symbol", {"name": query, "limit": 8})
            symbol.append(seconds * 1000)
            errors += is_error
    cpu_before = client.cpu_seconds()
    time.sleep(10)
    idle_cpu = client.cpu_seconds() - cpu_before
    memory = client.memory()
    client.close()
    return {"process_start_to_initialize_ms": init_s * 1000, "first_query_incl_watch_and_sync_ms": first_s * 1000,
            "first_query_error": first_error, "search_ms": percentiles(search), "find_symbol_ms": percentiles(symbol),
            "errors": errors, "idle_cpu_seconds_per_10s": idle_cpu, "memory_after_queries": memory}


def edit_workload(exe: Path, source: Path, scratch: Path, env: dict, edits: int) -> dict:
    """Real (not injected) Windows watcher events: edit, create, rename, delete."""
    if scratch.exists():
        shutil.rmtree(scratch)
    shutil.copytree(source, scratch, ignore=shutil.ignore_patterns(".git"))
    (scratch / ".git").mkdir()  # project root marker without history
    client = McpProcess(exe, scratch, env)
    client.call_tool("findex_search", {"query": "warmup_marker", "limit": 1})
    files = sorted(p for p in scratch.rglob("*.rs") if "target" not in p.parts)[:edits]
    results = {"edit": [], "create": [], "rename": [], "delete": []}
    correct = {"edit": 0, "create": 0, "rename": 0, "delete": 0}

    def observe(kind: str, query: str, expect_path: str | None, expect_present: bool):
        # Retry for up to 2 s: OS event delivery is asynchronous.
        deadline = time.perf_counter() + 2.0
        while True:
            payload, _, seconds, _ = client.call_tool("findex_search", {"query": query, "limit": 5})
            hits = (payload or {}).get("hits", [])
            present = any(h["path"] == expect_path for h in hits) if expect_path else bool(hits)
            if present == expect_present or time.perf_counter() > deadline:
                results[kind].append(seconds * 1000)
                correct[kind] += present == expect_present
                return
            time.sleep(0.05)

    for number, path in enumerate(files):
        marker = f"findexeditmarker{number}x"
        with path.open("a", encoding="utf-8") as handle:
            handle.write(f"\n// {marker}\n")
        rel = path.relative_to(scratch).as_posix()
        observe("edit", marker, rel, True)
    for number in range(min(edits, 10)):
        created = scratch / f"created_{number}.rs"
        created.write_text(f"fn findexcreatedmarker{number}() {{}}\n", encoding="utf-8")
        observe("create", f"findexcreatedmarker{number}", created.name, True)
        renamed = scratch / f"renamed_{number}.rs"
        tmp = scratch / f"renamed_{number}.rs.tmp"
        tmp.write_text(f"fn findexatomicmarker{number}() {{}}\n", encoding="utf-8")
        tmp.replace(renamed)  # editor-style atomic save
        observe("rename", f"findexatomicmarker{number}", renamed.name, True)
        created.unlink()
        observe("delete", f"findexcreatedmarker{number}", created.name, False)
    client.close()
    shutil.rmtree(scratch, ignore_errors=True)
    return {kind: {"latency_ms": percentiles(values), "correct": correct[kind], "total": len(values)}
            for kind, values in results.items()}


def concurrency(exe: Path, root: Path, env: dict, queries: list[str], clients: int) -> dict:
    """Several MCP servers sharing one index database, queried in parallel."""
    procs = [McpProcess(exe, root, env) for _ in range(clients)]
    errors, latencies, lock = [], [], threading.Lock()

    def worker(client: McpProcess, offset: int):
        for query in queries[offset::clients] + queries[:20]:
            payload, text, seconds, is_error = client.call_tool("findex_search", {"query": query, "limit": 8})
            with lock:
                latencies.append(seconds * 1000)
                if is_error:
                    errors.append(text[:200])

    started = time.perf_counter()
    threads = [threading.Thread(target=worker, args=(c, i)) for i, c in enumerate(procs)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    wall = time.perf_counter() - started
    for client in procs:
        client.close()
    return {"clients": clients, "requests": len(latencies), "wall_s": wall,
            "throughput_rps": len(latencies) / wall if wall else None, "latency_ms": percentiles(latencies),
            "errors": len(errors), "error_samples": errors[:3]}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--systems", default="baseline,current")
    parser.add_argument("--corpora", default="ripgrep,tokio,vscode")
    parser.add_argument("--reps", type=int, default=3)
    parser.add_argument("--queries", type=int, default=120)
    parser.add_argument("--quick", action="store_true")
    parser.add_argument("--out", default=None)
    args = parser.parse_args()
    if args.quick:
        args.corpora, args.reps, args.queries = "ripgrep", 1, 30
    out = results_dir(args.out)
    log = CommandLog(out)
    report = {"build": "release (cargo build --release -p findex)", "systems": {}}
    for system in args.systems.split(","):
        exe = findex_binary(system)
        if not exe.exists():
            report["systems"][system] = {"status": "BLOCKED", "reason": f"missing {exe.name}"}
            continue
        data = {"status": "EXECUTED", "binary_bytes": exe.stat().st_size, "corpora": {}}
        for corpus in args.corpora.split(","):
            root = CORPORA / corpus
            print(f"[perf] {system} {corpus}", file=sys.stderr)
            state_base = WORK / "state" / out.name / f"perf-{system}" / corpus
            cold = []
            for rep in range(args.reps):
                state = state_base / f"cold-{rep}"
                shutil.rmtree(state, ignore_errors=True)
                env = isolated_env(state)
                run = measured_run([str(exe), "index"], root, env, log)
                run["db_bytes"] = dir_size(state)
                warm = measured_run([str(exe), "index"], root, env, log)
                cold.append({"cold_index": run, "noop_reconcile": warm})
            env = isolated_env(state_base / "cold-0")
            queries = query_set(corpus, args.queries)
            entry = {
                "files_admitted": len(admitted_files(root)),
                "cold_index_s": percentiles([c["cold_index"]["wall_s"] for c in cold]),
                "noop_reconcile_s": percentiles([c["noop_reconcile"]["wall_s"] for c in cold]),
                "cold_index_peak_working_set_bytes": max(c["cold_index"].get("peak_working_set", 0) for c in cold),
                "cold_index_cpu_s": percentiles([c["cold_index"].get("cpu_user_s", 0) + c["cold_index"].get("cpu_system_s", 0) for c in cold]),
                "cold_index_io": {k: cold[0]["cold_index"].get(k) for k in ("read_bytes", "write_bytes")},
                "db_bytes": cold[0]["cold_index"]["db_bytes"],
                "runs": cold,
                "warm_mcp": warm_latency(exe, root, env, queries),
                "concurrency_4_clients_shared_db": concurrency(exe, root, env, queries, 4),
            }
            if corpus == "ripgrep":
                entry["real_watcher_edit_workload"] = edit_workload(
                    exe, root, WORK / "scratch" / f"edit-{system}", isolated_env(state_base / "edit"),
                    10 if args.quick else 25)
            data["corpora"][corpus] = entry
        report["systems"][system] = data
    write_json(out / "performance.json", report)
    memory = {s: {c: {"cold_index_peak_working_set_bytes": v.get("cold_index_peak_working_set_bytes"),
                      "mcp_after_queries": v.get("warm_mcp", {}).get("memory_after_queries"), "db_bytes": v.get("db_bytes")}
                  for c, v in d.get("corpora", {}).items()} for s, d in report["systems"].items()}
    cpu = {s: {c: {"cold_index_cpu_s": v.get("cold_index_cpu_s"),
                   "idle_cpu_seconds_per_10s": v.get("warm_mcp", {}).get("idle_cpu_seconds_per_10s")}
               for c, v in d.get("corpora", {}).items()} for s, d in report["systems"].items()}
    write_json(out / "memory.json", memory)
    write_json(out / "cpu.json", cpu)
    for system, data in report["systems"].items():
        for corpus, entry in data.get("corpora", {}).items():
            w = entry["warm_mcp"]
            print(f"{system:8} {corpus:8} files={entry['files_admitted']:5} cold={entry['cold_index_s']['mean']:.2f}s "
                  f"noop={entry['noop_reconcile_s']['mean']:.2f}s peakWS={entry['cold_index_peak_working_set_bytes']/2**20:.0f}MiB "
                  f"db={entry['db_bytes']/2**20:.1f}MiB first={w['first_query_incl_watch_and_sync_ms']:.0f}ms "
                  f"search p50/p95/p99={w['search_ms']['p50']:.1f}/{w['search_ms']['p95']:.1f}/{w['search_ms']['p99']:.1f}ms "
                  f"sym p50={w['find_symbol_ms'].get('p50', 0):.1f}ms idleCPU={w['idle_cpu_seconds_per_10s']:.3f}s "
                  f"conc={entry['concurrency_4_clients_shared_db']['throughput_rps']:.0f}rps err={entry['concurrency_4_clients_shared_db']['errors']}")
            if "real_watcher_edit_workload" in entry:
                print("   watcher:", {k: f"{v['correct']}/{v['total']} p50={v['latency_ms'].get('p50', 0):.0f}ms"
                                     for k, v in entry["real_watcher_edit_workload"].items()})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
