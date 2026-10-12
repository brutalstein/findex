"""Finalize one results directory: environment, manifest, regressions,
failures, summary.md and checksums. Components that were not executed are
recorded as NOT_RUN, never as passing.
Usage: report.py --out validation/results/<UTC>-<sha>
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from common import environment, git, results_dir, write_checksums, write_json

COMPONENTS = ["tests", "retrieval", "relations", "performance", "memory", "cpu", "tokens", "comparisons", "security", "compatibility"]
HIGHER_BETTER = ["file_recall@1", "file_recall@5", "file_recall@10", "span_recall@1", "file_mrr", "span_mrr",
                 "file_ndcg@10", "file_precision@1", "evidence_validity"]
LOWER_BETTER = ["top1_false_positive", "missing_evidence"]


def load(out: Path, name: str):
    path = out / f"{name}.json"
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else None


def delta(metric, base, cur, higher_better, tolerance):
    if base is None or cur is None:
        return None
    worse = cur < base * (1 - tolerance) if higher_better else cur > base * (1 + tolerance) + 1e-12
    return {"metric": metric, "baseline": base, "current": cur, "regression": bool(worse),
            "change_pct": round((cur - base) / base * 100, 2) if base else None}


def regressions(data: dict) -> list[dict]:
    rows = []
    systems = (data.get("retrieval") or {}).get("systems", {})
    base, cur = systems.get("findex-baseline", {}), systems.get("findex-current", {})
    for group in sorted(set(base.get("overall", {})) & set(cur.get("overall", {}))):
        b, c = base["overall"][group], cur["overall"][group]
        for metric in HIGHER_BETTER + LOWER_BETTER:
            row = delta(f"retrieval/{group}/{metric}", b.get(metric), c.get(metric), metric in HIGHER_BETTER, 0.0)
            if row:
                rows.append(row)
        for metric, path in (("latency_p95_ms", ("latency_ms", "p95")), ("payload_bytes_mean", ("payload_bytes", "mean"))):
            row = delta(f"retrieval/{group}/{metric}", b.get(path[0], {}).get(path[1]), c.get(path[0], {}).get(path[1]),
                        False, 0.10)
            if row:
                rows.append(row)
    systems = (data.get("performance") or {}).get("systems", {})
    base, cur = systems.get("baseline", {}).get("corpora", {}), systems.get("current", {}).get("corpora", {})
    for corpus in sorted(set(base) & set(cur)):
        b, c = base[corpus], cur[corpus]
        pairs = [("cold_index_s_p50", b["cold_index_s"].get("p50"), c["cold_index_s"].get("p50"), False),
                 ("noop_reconcile_s_p50", b["noop_reconcile_s"].get("p50"), c["noop_reconcile_s"].get("p50"), False),
                 ("cold_index_peak_working_set_bytes", b.get("cold_index_peak_working_set_bytes"),
                  c.get("cold_index_peak_working_set_bytes"), False),
                 ("db_bytes", b.get("db_bytes"), c.get("db_bytes"), False),
                 ("search_p95_ms", b["warm_mcp"]["search_ms"].get("p95"), c["warm_mcp"]["search_ms"].get("p95"), False),
                 ("find_symbol_p95_ms", b["warm_mcp"]["find_symbol_ms"].get("p95"), c["warm_mcp"]["find_symbol_ms"].get("p95"), False),
                 ("concurrency_rps", b["concurrency_4_clients_shared_db"].get("throughput_rps"),
                  c["concurrency_4_clients_shared_db"].get("throughput_rps"), True)]
        for metric, bv, cv, higher in pairs:
            row = delta(f"performance/{corpus}/{metric}", bv, cv, higher, 0.10)
            if row:
                rows.append(row)
    return rows


def failures(data: dict) -> dict:
    found = {"FAIL": [], "BLOCKED": [], "SKIPPED": [], "NOT_RUN": []}
    for name in COMPONENTS:
        value = data.get(name)
        if value is None or value.get("status") == "NOT_RUN":
            found["NOT_RUN"].append(name)
        elif value.get("status") == "BLOCKED":
            found["BLOCKED"].append({"component": name, "reason": value.get("reason")})
    for step, info in ((data.get("tests") or {}).get("steps") or {}).items():
        if info["status"] in ("FAIL", "SKIPPED"):
            found[info["status"]].append({"component": "tests", "step": step,
                                          "detail": info.get("failed_tests") or info.get("reason") or info.get("exit_code")})
    for name in ("security", "compatibility"):
        for check in (data.get(name) or {}).get("checks", []):
            if check["status"] != "PASS":
                found.setdefault(check["status"], []).append({"component": name, "check": check["name"],
                                                              "detail": check.get("detail")})
    for system, info in ((data.get("retrieval") or {}).get("systems") or {}).items():
        if info.get("status") == "BLOCKED":
            found["BLOCKED"].append({"component": "retrieval", "system": system, "reason": info.get("reason")})
    for session in (data.get("tokens") or {}).get("sessions", []):
        if session.get("status") == "INVALID":
            found["FAIL"].append({"component": "tokens", "session": f"{session['task']}/{session['arm']}/{session['rep']}",
                                  "detail": "Findex MCP server not connected; session excluded from comparisons"})
        elif session.get("status") == "BLOCKED":
            found["BLOCKED"].append({"component": "tokens", "session": f"{session['task']}/{session['arm']}/{session['rep']}",
                                     "reason": session.get("reason")})
        elif session.get("status") == "EXECUTED" and not session.get("correct"):
            found["FAIL"].append({"component": "tokens", "session": f"{session['task']}/{session['arm']}/{session['rep']}",
                                  "detail": "task answer judged incorrect: " + str(session.get("judge_detail"))[:200]})
    return found


def summary_md(out: Path, data: dict, env: dict, regress: list[dict], fails: dict) -> str:
    lines = [f"# Validation run {out.name}", "",
             f"- commit `{env['git_sha']}` on `{env['git_branch']}` (dirty: {env['git_dirty']})",
             f"- {env.get('os')}, {env.get('cpu_model')}, {env.get('logical_processors')} logical CPUs, "
             f"{round(env.get('ram_bytes', 0) / 2**30, 1)} GiB RAM", f"- rustc: {env.get('rustc')}",
             f"- Claude Code: {env.get('claude_code')}", "", "## Components", ""]
    for name in COMPONENTS:
        value = data.get(name)
        status = "NOT_RUN" if value is None or value.get("status") == "NOT_RUN" else value.get("status", "EXECUTED")
        lines.append(f"- {name}: {status}")
    tests = (data.get("tests") or {}).get("steps", {})
    if tests:
        lines += ["", "## Static checks and tests", ""]
        for step, info in tests.items():
            extra = f" ({info['passed']} passed, {info['failed']} failed, {info['ignored']} ignored)" if step == "test" and "passed" in info else ""
            lines.append(f"- {step}: {info['status']}{extra}")
    retrieval = (data.get("retrieval") or {}).get("systems", {})
    if retrieval:
        lines += ["", "## Retrieval (all queries)", "", "| system | R@1 | R@10 | MRR | spanR@1 | top1 FP | payload B | p95 ms |",
                  "|---|---|---|---|---|---|---|---|"]
        for system, info in retrieval.items():
            a = info.get("overall", {}).get("all")
            if a:
                lines.append(f"| {system} | {a['file_recall@1']:.3f} | {a['file_recall@10']:.3f} | {a['file_mrr']:.3f} | "
                             f"{a['span_recall@1']:.3f} | {a['top1_false_positive']:.3f} | {a['payload_bytes']['mean']:.0f} | "
                             f"{a['latency_ms']['p95']:.1f} |")
            else:
                lines.append(f"| {system} | {info.get('status')} {info.get('reason', '')} |")
    overall = ((data.get("comparisons") or {}).get("overall") or {}).get("__all__")
    if overall:
        lines += ["", "## Claude Code A/B (all tasks)", ""]
        for arm in ("A", "B", "C"):
            if arm in overall:
                o = overall[arm]
                lines.append(f"- arm {arm}: success {o['success_rate']}, total tokens mean {o['total_tokens'].get('mean')}, "
                             f"est. cost mean {o['total_cost_usd_estimate'].get('mean')}, n={o['total_tokens'].get('n')}")
        pct = lambda key: "n/a" if overall.get(key) is None else f"{overall[key]:+.1f}%"  # noqa: E731
        if "B_vs_A_total_tokens_change_pct" in overall:
            lines.append(f"- B vs A total tokens: {pct('B_vs_A_total_tokens_change_pct')}; "
                         f"est. cost: {pct('B_vs_A_est_cost_change_pct')}")
    lines += ["", "## Regressions (current vs baseline)", ""]
    flagged = [r for r in regress if r["regression"]]
    lines += [f"- {r['metric']}: {r['baseline']} -> {r['current']} ({r['change_pct']}%)" for r in flagged] or ["- none flagged"]
    lines += ["", "## Not passing", ""]
    for status in ("FAIL", "BLOCKED", "SKIPPED", "NOT_RUN"):
        lines.append(f"- {status}: {len(fails.get(status, []))}")
    lines += ["", "Details: failures.json, regressions.json. Checksums: checksums.sha256.", ""]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=None)
    parser.add_argument("--environment-only", action="store_true", help="record the environment at run start")
    args = parser.parse_args()
    out = results_dir(args.out)
    # The environment (commit, dirty state, tools) is the one the run started with.
    env = load(out, "environment") or environment()
    write_json(out / "environment.json", env)
    if args.environment_only:
        return 0
    data = {name: load(out, name) for name in COMPONENTS}
    for name, value in data.items():
        if value is None:
            write_json(out / f"{name}.json", {"status": "NOT_RUN", "reason": "component not executed in this run"})
    regress = regressions(data)
    fails = failures(data)
    write_json(out / "regressions.json", {"tolerance": "retrieval quality 0%, latency/payload/perf 10%", "rows": regress})
    write_json(out / "failures.json", fails)
    (out / "commands.jsonl").touch()
    (out / "summary.md").write_text(summary_md(out, data, env, regress, fails), encoding="utf-8")
    files = sorted(p for p in out.iterdir() if p.is_file() and p.name not in ("manifest.json", "checksums.sha256"))
    write_json(out / "manifest.json", {"run": out.name, "git_sha": env["git_sha"], "git_branch": env["git_branch"],
                                       "git_dirty": env["git_dirty"], "harness_tree": git("rev-parse", "HEAD:validation/harness"),
                                       "generated_by": "validation/harness/report.py", "argv": sys.argv[1:],
                                       "files": [{"name": p.name, "bytes": p.stat().st_size} for p in files]})
    write_checksums(out)
    print((out / "summary.md").read_text(encoding="utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
