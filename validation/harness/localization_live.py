"""Subscription-only live bug-localization study; at most 32 sequential Claude Haiku sessions.

Each session gets one issue title in a checkout of the fix's parent commit and must name the
source files to change. Arms: Findex, Graphify, codebase-memory, and no tool (Read/Grep/Glob).
Test-split tasks only, two per corpus. Scored against the gold files of the closing commit.
Off by default; an invalid session counts as a failure and the study continues; only a plan
or sign-in limit stops it. No API keys, no fallback model. Raw transcripts stay in raw/.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib

from claude_session import APPROVAL, APPROVAL_ENV, LIVE_FLAG, live_model_gate, run_claude
from common import REPO, VALIDATION, WORK, write_json
from issues import HISTORY
from localization import split
from mcp_client import McpProcess
from tool_comparison import invoke
from tool_comparison_live import claude_record, codex_env, load_results, trial_attempt

ARMS = ("findex", "graphify", "codebase-memory", "none")
CAP = 32
TOOL_NAMES = {"findex": ["findex_find_symbol", "findex_context", "findex_search"],
              "graphify": ["query_graph", "get_node"],
              "codebase-memory": ["search_graph", "get_code_snippet", "check_index_coverage"]}


def select_tasks(rows: list[dict], per_corpus: int) -> list[dict]:
    """Test split only, a fixed hash order per corpus (never chosen by outcome)."""
    chosen = []
    for corpus in sorted({r["corpus"] for r in rows}):
        pool = sorted((r for r in rows if r["corpus"] == corpus and split(r) == "test"),
                      key=lambda r: hashlib.sha256(("loc-live-v1" + r["id"]).encode()).hexdigest())
        chosen.extend(pool[:per_corpus])
    return chosen


def answer_files(text: str) -> list[str]:
    """The files of the single JSON object in the reply; anything else is no answer."""
    objects = re.findall(r"\{[^{}]*\}", text)
    if len(objects) != 1:
        return []
    try:
        files = json.loads(objects[0]).get("files")
    except (ValueError, AttributeError):
        return []
    if not isinstance(files, list):
        return []
    return [f.replace("\\", "/").removeprefix("./") for f in files if isinstance(f, str)]


def score(files: list[str], gold: list[str]) -> dict:
    return {"hit1": bool(files) and files[0] in gold, "hit3": any(f in gold for f in files[:3])}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(LIVE_FLAG, action="store_true")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--cbm", type=Path, required=True)
    parser.add_argument("--graphify", type=Path, required=True)
    parser.add_argument("--graphify-python", type=Path, required=True)
    parser.add_argument("--per-corpus", type=int, default=2)
    parser.add_argument("--max-sessions", type=int, default=CAP)
    parser.add_argument("--arms", nargs="+", choices=ARMS, default=list(ARMS), help="run a subset, e.g. a Findex-only re-run")
    args = parser.parse_args()
    arms = tuple(args.arms)
    args.out = args.out.resolve()  # the client runs with the task checkout as cwd
    if not args.enable_live_model_tests or os.environ.get(APPROVAL_ENV) != APPROVAL:
        raise SystemExit("NOT_RUN: explicit live flag and subscription approval required")
    if os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS"):
        raise SystemExit("BLOCKED: live tests never run in CI")
    if not 1 <= args.max_sessions <= CAP:
        raise SystemExit(f"BLOCKED: maximum {CAP} sessions")
    args.out.mkdir(parents=True, exist_ok=True)
    rows = [json.loads(line) for line in (VALIDATION / "golden/issue_localization.jsonl").open(encoding="utf-8")]
    tasks = select_tasks(rows, args.per_corpus)
    if len(tasks) * len(arms) > args.max_sessions:
        raise SystemExit("BLOCKED: planned sessions exceed the cap")
    saved = args.out / "live-results.json"
    results = load_results(saved)
    clean = codex_env(dict(os.environ))
    try:
        status = json.loads(subprocess.run(["claude", "auth", "status"], env=clean, capture_output=True, text=True,
                                           encoding="utf-8", errors="replace", timeout=30).stdout)
    except (ValueError, OSError):
        status = {}
    blocked = live_model_gate(True, env=dict(clean, **{APPROVAL_ENV: APPROVAL}), status=status)
    if blocked:
        raise SystemExit(blocked)
    home = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex")))
    registered = tomllib.loads((home / "config.toml").read_text(encoding="utf-8")).get(
        "mcp_servers", {}).get("codebase-memory", {})
    cbm_env = {k: v for k, v in registered.get("env", {}).items() if k in ("CBM_RUNTIME_DIR", "CBM_CACHE_DIR")}
    findex_bin = REPO / "target/release/findex.exe"
    findex_state = Path(os.environ.get("FINDEX_LIVE_FINDEX_STATE") or REPO / "target/live-state")
    write_json(args.out / "live-plan.json", {"tasks": [t["id"] for t in tasks], "arms": list(arms),
                                             "max_sessions": args.max_sessions, "claude_model": "haiku", "max_turns": 20})
    scratch = (WORK / "localization-live" / args.out.name).resolve()
    raw = args.out / "raw"
    idle_failures = 0
    for task in tasks:
        if all(any(s["key"] == f"{task['id']}:{arm}" for s in results["sessions"]) for arm in arms):
            continue
        history, tree, graph_out = HISTORY / task["corpus"], scratch / task["id"] / "tree", scratch / task["id"] / "graphify"
        if tree.exists():
            subprocess.run(["git", "-C", str(history), "worktree", "remove", "--force", str(tree)], capture_output=True)
            shutil.rmtree(tree, ignore_errors=True)
        shutil.rmtree(graph_out, ignore_errors=True)
        tree.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "-C", str(history), "worktree", "prune"], capture_output=True)
        subprocess.run(["git", "-C", str(history), "worktree", "add", "-q", "--detach", str(tree), task["base_commit"]],
                       check=True, capture_output=True)
        root = findex_state / task["id"]
        (root / "tmp").mkdir(parents=True, exist_ok=True)
        findex_env = {"LOCALAPPDATA": str(root), "TEMP": str(root / "tmp"), "TMP": str(root / "tmp")}
        indexed = subprocess.run([str(findex_bin), "index"], cwd=tree, env=dict(clean, **findex_env),
                                 capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=300)
        if indexed.returncode:
            raise SystemExit(f"BLOCKED: findex index failed for {task['id']}: {indexed.stderr.strip()[:200]}")
        if "graphify" in arms:
            invoke([str(args.graphify), "extract", str(tree), "--code-only", "--max-workers", "2",
                    "--out", str(graph_out)], REPO)
        project = re.sub(r"[^a-zA-Z0-9_-]", "-", f"loc-live-{args.out.name}-{task['id']}")
        if "codebase-memory" in arms:
            cbm = McpProcess(args.cbm, tree, dict(os.environ, **cbm_env), server_args=[])
            try:
                _, text, _, error = cbm.call_tool("index_repository", {
                    "repo_path": str(tree), "name": project, "mode": "full", "persistence": False})
            finally:
                cbm.close()
            if error:
                raise SystemExit(f"BLOCKED: codebase-memory index failed for {task['id']}: {text[:200]}")
        servers = {
            "findex": {"command": str(findex_bin), "args": ["mcp"], "env": findex_env},
            "graphify": {"command": str(args.graphify_python),
                         "args": ["-m", "graphify.serve", str(graph_out / "graphify-out/graph.json")]},
            "codebase-memory": {"command": str(args.cbm), "args": [], "env": cbm_env}}
        for arm in arms:
            key = f"{task['id']}:{arm}"
            if any(s["key"] == key for s in results["sessions"]):
                continue
            hint = {"findex": "findex_context is loaded at session start: call it with the issue title as a plain sentence. ",
                    "graphify": "Start with the supplied graphify MCP server, then read source only if needed. ",
                    "codebase-memory": f"Start with the supplied codebase-memory MCP server (indexed project `{project}`; "
                                       'do not reindex it; use format="tree" for search_graph), then read source only if needed. ',
                    "none": "Use only Read, Grep and Glob. "}[arm]
            prompt = (f'An issue titled "{task["query"]}" is open in this repository. Name the source files that '
                      f"must change to fix it. {hint}"
                      "Do not edit files, install anything, index another project, or use the network. "
                      "Do not launch agents, subagents or other model sessions. "
                      'Return exactly one JSON object, no prose: {"files":["path/one","path/two"]} with at most '
                      "3 repository-relative source files (not tests), most likely first.")
            dest = raw / f"{task['id']}-{arm}"
            dest.mkdir(parents=True, exist_ok=True)
            mcp = dest / "mcp.json"
            mcp.write_text(json.dumps({"mcpServers": {arm: servers[arm]} if arm != "none" else {}}), encoding="utf-8")
            record = {"key": key, "client": "claude", "tool": arm, "task": task["id"]}
            with trial_attempt(saved, results, record, args.max_sessions, halt_on_invalid=False, hard_cap=CAP):
                session = run_claude(tree, prompt, model="haiku", max_turns=20, budget_usd=1,
                                     allowed_tools=["Read", "Grep", "Glob"] + (
                                         ["ToolSearch"] + [f"mcp__{arm}__{t}" for t in TOOL_NAMES[arm]] if arm != "none" else []),
                                     disallowed_tools=["Bash", "Edit", "Write", "Agent", "Skill", "WebSearch", "WebFetch"],
                                     mcp_config_path=mcp, env=clean, timeout=300)
                (dest / "events.jsonl").write_text("\n".join(json.dumps(e) for e in session["events"]), encoding="utf-8")
                scored = claude_record(session, arm, {"id": task["id"], "expected": []})
                if arm == "none":  # no supplied tool to call: a clean, usage-reporting session is valid
                    summary = scored["summary"]
                    scored["valid"] = (summary["exit_code"] == 0 and scored["usage"]["input_tokens"] > 0
                                       and not summary["limit_hit"] and summary["is_error"] is False
                                       and not summary["timed_out"])
                files = answer_files(scored["summary"]["final_text"])
                scored.update(score(files, task["gold_files"]), files=files, gold=task["gold_files"])
                scored["hit1"], scored["hit3"] = scored["valid"] and scored["hit1"], scored["valid"] and scored["hit3"]
                scored["correct"] = scored["hit1"]
                if not scored["valid"]:
                    scored["stderr_tail"] = session["stderr_tail"][-300:]
                record.update(scored)
            print(f"{key}: hit1={record['hit1']} hit3={record['hit3']} valid={record['valid']}", flush=True)
            # A client that fails before any model call (bad config, sign-in) would burn the whole cap.
            unused = not record.get("valid") and not record.get("usage", {}).get("input_tokens")
            idle_failures = idle_failures + 1 if unused else 0
            if idle_failures >= 3 and not results.get("stopped"):
                results["stopped"] = "three consecutive sessions failed before any model call; fix the setup"
                write_json(saved, results)
            if results.get("stopped"):
                break
        if results.get("stopped"):
            break
    summary = {}
    for arm in ARMS:
        rows_ = [s for s in results["sessions"] if s["tool"] == arm]
        summary[arm] = {"sessions": len(rows_), "valid": sum(s["valid"] for s in rows_),
                        "correct": sum(bool(s.get("correct")) for s in rows_),
                        "hit1": sum(bool(s.get("hit1")) for s in rows_), "hit3": sum(bool(s.get("hit3")) for s in rows_),
                        "total_tokens": sum(s.get("usage", {}).get("input_tokens", 0) + s.get("usage", {}).get("output_tokens", 0)
                                            for s in rows_)}
    write_json(args.out / "live-localization-summary.json", {
        "summary": summary, "stopped": results.get("stopped"),
        "limits": f"{len(tasks)} test-split tasks, one repetition per arm; invalid sessions count as failures; "
                  "cache differences included. Not evidence of general superiority or monetary savings."})


if __name__ == "__main__":
    main()
