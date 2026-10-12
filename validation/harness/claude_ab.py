"""Controlled A/B study: official Claude Code without vs. with Findex.

Same model, effort, prompt, corpus commit, turn/budget caps and built-in
tool permissions in both arms; only the Findex MCP server differs. Every
session runs in a fresh git-initialised copy with its own Findex state.
Order is counterbalanced (ABBA per task and repetition). Correctness is
judged by scripts and predefined acceptance criteria, never by the model.

Usage: claude_ab.py [--model haiku] [--reps 3] [--tasks t1,t2] [--session-budget 0.40] [--total-budget 6]
Writes tokens.json and comparisons.json.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
from pathlib import Path

from claude_session import fresh_workspace, mcp_config, run_claude, summarize_session
from common import CORPORA, VALIDATION, WORK, results_dir, write_json

BASE_TOOLS = ["Read", "Grep", "Glob", "Edit"]
FINDEX_TOOLS = ["mcp__findex__findex_search", "mcp__findex__findex_find_symbol",
                "mcp__findex__findex_symbols", "mcp__findex__findex_impact"]
DENY = ["Bash", "WebFetch", "WebSearch", "Agent", "Task", "NotebookEdit", "Write"]


def golden_symbol(corpus: str) -> dict:
    rows = [json.loads(line) for line in (VALIDATION / "golden" / "symbol_navigation.jsonl").open(encoding="utf-8")]
    return next(r for r in rows if r["corpus"] == corpus and r["split"] == "heldout" and r["type"] == "symbol_exact")


def mentioned_paths(text: str) -> set[str]:
    return {m.replace("\\", "/").strip("`'\"(),.:") for m in re.findall(r"[\w./\\-]+\.(?:rs|py|ts|cc|h)", text)}


def callers_truth() -> set[str]:
    out = subprocess.run(["git", "-C", str(CORPORA / "tokio"), "grep", "-n", "-E", r"\bpoll_proceed\("],
                         capture_output=True, text=True).stdout
    return {line.split(":")[0] for line in out.splitlines() if "fn poll_proceed" not in line}


def cargo_check(repo: Path, test_filter: str, target: Path) -> tuple[bool, str]:
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    proc = subprocess.run(["cargo", "test", "--locked", "-q", "-p", "findex-core", "--lib", test_filter], cwd=repo,
                          capture_output=True, text=True, env=env, timeout=900)
    return proc.returncode == 0 and "test result: ok" in proc.stdout and " 0 passed" not in proc.stdout, proc.stdout[-300:] + proc.stderr[-300:]


def make_tasks() -> dict:
    symbol = golden_symbol("ripgrep")
    callers = callers_truth()

    def judge_find(text, repo, target):
        expected = symbol["expected"]
        for path, line in expected:
            for match in re.finditer(re.escape(path) + r"\D{0,3}(\d+)", text.replace("\\", "/")):
                if abs(int(match.group(1)) - line) <= 1:
                    return True, f"found {path}:{match.group(1)}"
        return False, f"expected {expected}"

    def judge_callers(text, repo, target):
        mentioned = {p for p in mentioned_paths(text) if p.startswith(("tokio", "tests", "src"))}
        mentioned = {next((c for c in callers if c.endswith(p) or p.endswith(c)), p) for p in mentioned}
        recall = len(callers & mentioned) / len(callers)
        precision = len(callers & mentioned) / len(mentioned) if mentioned else 0.0
        return recall >= 0.8 and precision >= 0.8, f"recall={recall:.2f} precision={precision:.2f} expected={len(callers)}"

    def judge_concept(text, repo, target):
        ok = "should_strip_auth" in text and "sessions.py" in text
        return ok, "expects should_strip_auth in src/requests/sessions.py"

    def judge_trace(text, repo, target):
        ok = "search_in_db" in text and ("WatchedIndex" in text or "SearchBackend" in text)
        return ok, "expects SearchBackend/WatchedIndex::search -> findex_index::search_in_db"

    def prepare_bug(repo: Path):
        path = repo / "crates" / "findex-core" / "src" / "lib.rs"
        text = path.read_text(encoding="utf-8")
        path.write_text(text.replace('        || name.ends_with(".pem")\n', ""), encoding="utf-8", newline="\n")

    def judge_bug(text, repo, target):
        return cargo_check(repo, "sensitive_filename_rules", target)

    def judge_feature(text, repo, target):
        path = repo / "crates" / "findex-core" / "src" / "lib.rs"
        source = path.read_text(encoding="utf-8")
        if "kdbx" not in source.split("#[cfg(test)]")[-1]:
            return False, "agent did not add a test assertion for .kdbx"
        hidden = ("\n#[cfg(test)]\nmod hidden_acceptance {\n    #[test]\n    fn hidden_kdbx_acceptance() {\n"
                  "        assert!(super::is_sensitive_filename(std::path::Path::new(\"Vault.KDBX\")));\n"
                  "        assert!(!super::is_sensitive_filename(std::path::Path::new(\"main.rs\")));\n    }\n}\n")
        path.write_text(source + hidden, encoding="utf-8", newline="\n")
        ok_hidden, out_hidden = cargo_check(repo, "hidden_kdbx_acceptance", target)
        ok_all, out_all = cargo_check(repo, "", target)
        return ok_hidden and ok_all, f"hidden={ok_hidden} suite={ok_all} {out_hidden[-120:]}"

    return {
        "t1_find_function": {"corpus": "ripgrep", "judge": judge_find, "max_turns": 12,
                             "prompt": f"In this repository, where is `{symbol['name']}` defined? "
                                       "Answer with the repository-relative path and 1-based line number as path:line."},
        "t2_identify_callers": {"corpus": "tokio", "judge": judge_callers, "max_turns": 20,
                                "prompt": "List every source file in this repository that calls the function "
                                          "`poll_proceed` (excluding the file where it is only defined, unless it also calls it). "
                                          "Give repository-relative paths, one per line."},
        "t3_explain_concept": {"corpus": "requests", "judge": judge_concept, "max_turns": 12,
                               "prompt": "When requests follows a redirect, which method decides whether the Authorization "
                                         "header must be removed? Give the method name and its file, and explain the rule in two sentences."},
        "t4_cross_file_trace": {"corpus": "findex", "judge": judge_trace, "max_turns": 15,
                                "prompt": "Trace what happens when the MCP server receives a findex_search tool call: name the "
                                          "function in the findex-watch crate that handles it and the findex-index function that finally "
                                          "runs the SQLite FTS query. Be concise."},
        "t5_fix_bug": {"corpus": "findex", "judge": judge_bug, "max_turns": 20, "prepare": prepare_bug,
                       "prompt": "Bug report: Findex now indexes TLS private key files such as server.pem even though they "
                                 "must be excluded as sensitive. Find the root cause and fix it with a minimal edit. "
                                 "You cannot run commands."},
        "t6_feature_with_test": {"corpus": "findex", "judge": judge_feature, "max_turns": 20,
                                 "prompt": "Treat KeePass databases (*.kdbx, case-insensitive) as sensitive files that are never "
                                           "indexed, and add an assertion for it to the existing unit test of that rule. "
                                           "Make a minimal edit. You cannot run commands."},
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default="haiku")
    parser.add_argument("--effort", default=None)
    parser.add_argument("--reps", type=int, default=3)
    parser.add_argument("--tasks", default="")
    parser.add_argument("--arms", default="A,B")
    parser.add_argument("--session-budget", type=float, default=0.40)
    parser.add_argument("--total-budget", type=float, default=6.0, help="stop when estimated spend reaches this")
    parser.add_argument("--out", default=None)
    args = parser.parse_args()
    out = results_dir(args.out)
    tasks = make_tasks()
    selected = [t for t in tasks if not args.tasks or t in args.tasks.split(",")]
    arms = args.arms.split(",")
    spent, sessions, blocked = 0.0, [], ""
    base = WORK / "claude" / f"ab-{out.name}"
    target = WORK / "claude" / "cargo-target"  # shared build cache for acceptance tests only
    for rep in range(args.reps):
        for index, name in enumerate(selected):
            order = arms if (rep + index) % 2 == 0 else list(reversed(arms))  # counterbalanced
            for arm in order:
                if spent >= args.total_budget or blocked:
                    sessions.append({"task": name, "arm": arm, "rep": rep, "status": "BLOCKED",
                                     "reason": blocked or f"total budget {args.total_budget} reached"})
                    continue
                task = tasks[name]
                dest = base / f"{name}-{arm}-{rep}"
                repo = fresh_workspace(task["corpus"], dest)
                if "prepare" in task:
                    task["prepare"](repo)
                    subprocess.run(["git", "-c", "user.email=v@l", "-c", "user.name=v", "commit", "-qam", "task setup"],
                                   cwd=repo, capture_output=True)
                tools = BASE_TOOLS + (FINDEX_TOOLS if arm in ("B", "C") else [])
                config = mcp_config(dest, "current" if arm == "B" else "baseline", via="direct") if arm in ("B", "C") else None
                print(f"[ab] rep={rep} {name} arm={arm}", file=sys.stderr)
                session = run_claude(repo, task["prompt"], model=args.model, effort=args.effort,
                                     max_turns=task["max_turns"], budget_usd=args.session_budget,
                                     allowed_tools=tools, disallowed_tools=DENY, mcp_config_path=config)
                summary = summarize_session(session)
                correct, reason = task["judge"](summary["final_text"], repo, target)
                spent += summary["total_cost_usd_estimate"] or 0.0
                connected = any(s.get("name") == "findex" and s.get("status") == "connected"
                                for s in summary["init_mcp_servers"] or [])
                # A Findex arm whose server failed to start silently measures A twice.
                status = "EXECUTED" if connected or not config else "INVALID"
                if summary["limit_hit"]:
                    # Never retry around a plan limit: record it and stop scheduling sessions.
                    status, blocked = "BLOCKED", "Claude plan usage limit reached: " + summary["final_text"][:120]
                record = {"task": name, "arm": arm, "rep": rep, "status": status, "findex_connected": connected,
                          "correct": correct,
                          "judge_detail": reason, **{k: v for k, v in summary.items() if k != "final_text"}}
                record["answer_excerpt"] = summary["final_text"][:600]
                if status == "BLOCKED":
                    record["reason"], record["correct"] = blocked, None
                sessions.append(record)
                print(f"     correct={correct} in={summary['input_tokens']} out={summary['output_tokens']} "
                      f"cw={summary['cache_creation_input_tokens']} cr={summary['cache_read_input_tokens']} "
                      f"cost~{summary['total_cost_usd_estimate']} tools={summary['tool_call_count']} "
                      f"wall={summary['wall_s']}s {reason[:80]}", file=sys.stderr)
                shutil.rmtree(dest / "state", ignore_errors=True)
    write_json(out / "tokens.json", {"model": args.model, "effort": args.effort, "reps": args.reps,
                                      "cost_note": "total_cost_usd is Claude Code's client-side estimate, not an invoice",
                                      "sessions": sessions, "estimated_spend_usd": spent})
    write_json(out / "comparisons.json", compare(sessions, arms))
    return 0


def describe(values: list[float]) -> dict:
    if not values:
        return {"n": 0}
    mean = statistics.fmean(values)
    sd = statistics.stdev(values) if len(values) > 1 else 0.0
    return {"n": len(values), "mean": mean, "stdev": sd, "min": min(values), "max": max(values),
            "ci95_halfwidth_normal_approx": 1.96 * sd / len(values) ** 0.5 if len(values) > 1 else None}


def compare(sessions: list[dict], arms: list[str]) -> dict:
    metrics = ["input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens",
               "total_tokens", "total_cost_usd_estimate", "tool_call_count", "mcp_call_count", "files_read",
               "tool_result_bytes", "wall_s"]
    executed = [s for s in sessions if s.get("status") == "EXECUTED"]
    for s in executed:
        s["total_tokens"] = sum((s.get(k) or 0) for k in ("input_tokens", "output_tokens",
                                                           "cache_creation_input_tokens", "cache_read_input_tokens"))
    result = {"per_task": {}, "overall": {}}
    tasks = sorted({s["task"] for s in executed})
    for task in tasks + ["__all__"]:
        entry = {}
        for arm in arms:
            rows = [s for s in executed if s["arm"] == arm and (task == "__all__" or s["task"] == task)]
            entry[arm] = {"success_rate": (sum(r["correct"] for r in rows) / len(rows)) if rows else None,
                          **{m: describe([float(r.get(m) or 0) for r in rows]) for m in metrics}}
        if "A" in entry and "B" in entry and entry["A"]["total_tokens"].get("n") and entry["B"]["total_tokens"].get("n"):
            a, b = entry["A"]["total_tokens"]["mean"], entry["B"]["total_tokens"]["mean"]
            entry["B_vs_A_total_tokens_change_pct"] = (b - a) / a * 100 if a else None
            a, b = entry["A"]["total_cost_usd_estimate"]["mean"], entry["B"]["total_cost_usd_estimate"]["mean"]
            entry["B_vs_A_est_cost_change_pct"] = (b - a) / a * 100 if a else None
            # Paired by (task, rep): per-pair differences give a variance estimate.
            pairs = []
            for s in executed:
                if s["arm"] == "A" and (task == "__all__" or s["task"] == task):
                    other = next((o for o in executed if o["arm"] == "B" and o["task"] == s["task"] and o["rep"] == s["rep"]), None)
                    if other:
                        pairs.append(other["total_tokens"] - s["total_tokens"])
            entry["paired_total_token_difference_B_minus_A"] = describe([float(p) for p in pairs])
        result["per_task" if task != "__all__" else "overall"][task] = entry
    return result


if __name__ == "__main__":
    raise SystemExit(main())
