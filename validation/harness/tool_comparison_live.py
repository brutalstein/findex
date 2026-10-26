"""Subscription-only, sequential two-task pilot; maximum 12 sessions, no API keys.

Off by default. Reuses saved sessions on rerun. Never falls back to a provider
or another model when a client is blocked. Raw transcripts stay in raw/.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib

from claude_session import (APPROVAL_ENV, APPROVAL, BILLING_ENV, LIVE_FLAG, live_model_gate,
                            run_claude, session_env, subscription_status, summarize_session)
from common import REPO, write_json
from tool_comparison import TOOLS

CODEX_BILLING = ("OPENAI_API_KEY", "CODEX_API_KEY", "OPENAI_BASE_URL", "OPENAI_API_BASE",
                "AZURE_OPENAI_API_KEY", "AZURE_OPENAI_ENDPOINT", "CODEX_CONFIG_OVERRIDES")


def codex_env(env: dict) -> dict:
    return {k: v for k, v in session_env(env).items() if k not in CODEX_BILLING}


def codex_subscription_error(code: int, text: str) -> str | None:
    if code or "Logged in using ChatGPT" not in text or re.search(r"API key", text, re.I):
        return "BLOCKED: Codex must report a ChatGPT subscription login"
    return None


def answer_correct(text: str, query: dict) -> bool:
    try:
        answer = json.loads(text.strip().removeprefix("```json").removesuffix("```").strip())
        return any(answer.get("path", "").replace("\\", "/") == path
                   and type(answer.get("line")) is int and abs(answer["line"] - line) <= 1
                   for path, line in query["expected"])
    except (ValueError, AttributeError, TypeError):
        return False


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(LIVE_FLAG, action="store_true")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--codex", type=Path, required=True)
    parser.add_argument("--cbm", type=Path, required=True)
    parser.add_argument("--graphify-python", type=Path, required=True)
    parser.add_argument("--max-sessions", type=int, default=12)
    args = parser.parse_args()
    args.out = args.out.resolve()
    if not args.enable_live_model_tests or os.environ.get(APPROVAL_ENV) != APPROVAL:
        raise SystemExit("NOT_RUN: explicit live flag and subscription approval required")
    if os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS"):
        raise SystemExit("BLOCKED: live tests never run in CI")
    if not 1 <= args.max_sessions <= 12:
        raise SystemExit("BLOCKED: maximum 12 sessions; cannot widen the authorized cap")
    manifest = json.loads((args.out / "manifest.json").read_text(encoding="utf-8"))
    tasks = [next(q for q in manifest["queries"] if q["corpus"] == corpus and q["type"] == "symbol_exact")
             for corpus in ("requests", "zod")]
    setups = json.loads((args.out / "local-results.json").read_text(encoding="utf-8"))["setups"]
    projects = {s["corpus"]: s["codebase_project"] for s in setups}
    state = args.out / "raw/live"
    state.mkdir(parents=True, exist_ok=True)
    schema = state / "answer-schema.json"
    schema.write_text(json.dumps({"type": "object", "properties": {"path": {"type": "string"},
                            "line": {"type": "integer"}}, "required": ["path", "line"],
                            "additionalProperties": False}), encoding="utf-8")
    clean = codex_env(dict(os.environ))
    login = subprocess.run([str(args.codex), "login", "status"], env=clean, capture_output=True,
                           text=True, encoding="utf-8", errors="replace", timeout=30)
    codex_block = codex_subscription_error(login.returncode, login.stdout + login.stderr)
    claude_status = {}
    if shutil.which("claude"):
        status = subprocess.run(["claude", "auth", "status"], env=clean, capture_output=True,
                                text=True, encoding="utf-8", errors="replace", timeout=30)
        try:
            claude_status = json.loads(status.stdout)
        except ValueError:
            pass
    claude_block = live_model_gate(True, env=dict(clean, **{APPROVAL_ENV: APPROVAL}), status=claude_status)
    home = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex")))
    config = tomllib.loads((home / "config.toml").read_text(encoding="utf-8"))
    profile = {k: config[k] for k in ("model", "model_reasoning_effort", "service_tier") if k in config}
    registered_cbm = config.get("mcp_servers", {}).get("codebase-memory", {})
    cbm_environment = {k: v for k, v in registered_cbm.get("env", {}).items()
                       if k in ("CBM_RUNTIME_DIR", "CBM_CACHE_DIR")}
    plan = {"tasks": [q["id"] for q in tasks], "tools": list(TOOLS), "max_sessions": args.max_sessions,
            "codex_profile": profile, "claude_model": "haiku",
            "blocked": {"codex": codex_block, "claude": claude_block}}
    write_json(args.out / "live-plan.json", plan)
    saved = args.out / "live-results.json"
    results = json.loads(saved.read_text(encoding="utf-8")) if saved.exists() else {"sessions": [], "started": 0}
    if results.get("stopped"):
        raise SystemExit(f"BLOCKED: {results['stopped']}")
    pending = sum(6 for reason in (codex_block, claude_block) if reason is None)
    if results["started"] + pending > args.max_sessions and not results["sessions"]:
        raise SystemExit("BLOCKED: planned sessions exceed the cap")
    for client, blocked in (("codex", codex_block), ("claude", claude_block)):
        if blocked:
            print(f"{client}: {blocked}", flush=True)
            continue
        for query in tasks:
            corpus = query["corpus"]
            repo = REPO / "target/tool-comparison/corpora" / corpus
            graph = REPO / "target/tool-comparison/state/graphify" / corpus / "graphify-out/graph.json"
            servers = {
                "findex": {"command": str(REPO / "target/release/findex.exe"), "args": ["mcp"],
                           "env": {"LOCALAPPDATA": str(REPO / "target/tool-comparison/state/findex" / corpus)}},
                "graphify": {"command": str(args.graphify_python), "args": ["-m", "graphify.serve", str(graph)]},
                "codebase-memory": {"command": str(args.cbm), "args": [], "env": cbm_environment},
            }
            for tool in TOOLS:
                key = f"{client}:{query['id']}:{tool}"
                if any(s["key"] == key for s in results["sessions"]):
                    continue
                if results["started"] >= args.max_sessions:
                    raise SystemExit("Session cap reached; no additional calls authorized")
                dest = state / f"trial-{results['started'] + 1}-{client}-{corpus}-{tool}"
                dest.mkdir(exist_ok=True)
                server = servers[tool]
                prompt = (f"Locate the definition of `{query['query']}` in this repository. "
                          f"Start with the supplied {tool} MCP server, then read source only if needed. "
                          "Do not edit files, install anything, index another project, or use the network. "
                          "Return only JSON with path (repository-relative) and line (1-based definition). ")
                if tool == "codebase-memory":
                    prompt += f"Use existing indexed project `{projects[corpus]}`; do not reindex it."
                mcp = dest / "mcp.json"
                mcp.write_text(json.dumps({"mcpServers": {tool: server}}), encoding="utf-8")
                results["started"] += 1
                write_json(saved, results)  # Count before spawning, even if the client fails.
                if client == "codex":
                    argv = [str(args.codex), "exec", "--ignore-user-config", "--ephemeral", "--json",
                            "--sandbox", "read-only", "--output-schema", str(schema), "-C", str(repo)]
                    overrides = dict(profile, forced_login_method="chatgpt", model_provider="openai",
                                     **{"features.plugins": False, "project_doc_max_bytes": 0,
                                        "approval_policy": "never"})
                    for k, v in overrides.items():
                        argv += ["-c", f"{k}={json.dumps(v)}"]
                    for k, v in server.items():
                        if k == "env":
                            for env_key, env_value in v.items():
                                argv += ["-c", f"mcp_servers.{tool}.env.{env_key}={json.dumps(env_value)}"]
                        else:
                            argv += ["-c", f"mcp_servers.{tool}.{k}={json.dumps(v)}"]
                    read_tools = {
                        "findex": ["findex_find_symbol", "findex_context", "findex_search", "findex_symbols"],
                        "graphify": ["query_graph", "get_node", "get_neighbors"],
                        "codebase-memory": ["search_graph", "get_code_snippet", "check_index_coverage", "trace_path"],
                    }[tool]
                    argv += ["-c", f"mcp_servers.{tool}.enabled_tools={json.dumps(read_tools)}",
                             "-c", f'mcp_servers.{tool}.default_tools_approval_mode="approve"',
                             "-c", f"mcp_servers.{tool}.startup_timeout_sec=60"]
                    argv.append(prompt)
                    proc = subprocess.run(argv, env=clean, capture_output=True, text=True,
                                          encoding="utf-8", errors="replace", timeout=240)
                    (dest / "events.jsonl").write_text(proc.stdout, encoding="utf-8")
                    (dest / "stderr.txt").write_text(proc.stderr, encoding="utf-8")
                    events = []
                    for line in proc.stdout.splitlines():
                        try:
                            events.append(json.loads(line))
                        except ValueError:
                            pass
                    items = [e["item"] for e in events if "item" in e and e.get("type") == "item.completed"]
                    final = next((i.get("text", "") for i in reversed(items) if i.get("type") == "agent_message"), "")
                    usage = next((e["usage"] for e in reversed(events) if e.get("type") == "turn.completed"), {})
                    used = [i for i in items if i.get("type") == "mcp_tool_call" and i.get("server") == tool]
                    record = {"key": key, "client": client, "tool": tool, "task": query["id"],
                              "exit_code": proc.returncode, "usage": usage, "mcp_calls": len(used),
                              "correct": answer_correct(final, query) and bool(used), "final": final}
                    permission_failure = any("requires approval" in json.dumps(i) for i in items)
                    record["valid"] = not permission_failure and bool(used) and all(i.get("status") == "completed" for i in used)
                    stop = proc.returncode != 0 or not usage or not record["valid"]
                else:
                    tool_names = {"findex": ["findex_find_symbol", "findex_context", "findex_search"],
                                  "graphify": ["query_graph", "get_node"],
                                  "codebase-memory": ["search_graph", "get_code_snippet", "check_index_coverage"]}[tool]
                    session = run_claude(repo, prompt, model="haiku", max_turns=8, budget_usd=1,
                                         allowed_tools=["Read", "Grep", "Glob", "ToolSearch"] +
                                             [f"mcp__{tool}__{t}" for t in tool_names],
                                         disallowed_tools=["Bash", "Edit", "Write", "Agent", "Skill", "WebSearch", "WebFetch"],
                                         mcp_config_path=mcp, env=clean, timeout=240)
                    (dest / "events.jsonl").write_text("\n".join(json.dumps(e) for e in session["events"]), encoding="utf-8")
                    summary = summarize_session(session)
                    record = {"key": key, "client": client, "tool": tool, "task": query["id"],
                              "correct": answer_correct(summary["final_text"], query) and summary["mcp_call_count"] > 0,
                              "summary": summary}
                    stop = summary["limit_hit"] or summary["is_error"] or summary["timed_out"]
                results["sessions"].append(record)
                write_json(saved, results)
                print(f"{key}: correct={record['correct']}", flush=True)
                if stop:
                    results["stopped"] = f"{client}: failed or missing usage; no retries or fallback"
                    write_json(saved, results)
                    return


if __name__ == "__main__":
    main()
