"""Subscription-only, sequential live study; maximum 12 sessions, no API keys.

Off by default. Reuses saved sessions on rerun. Never falls back to a provider
or another model when a client is blocked. An invalid session counts against the
cap and the study continues; only a subscription or usage limit stops it.
Raw transcripts stay in raw/.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
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
    objects = re.findall(r"\{[^{}]*\}", text)
    if len(objects) != 1:  # a hedged list of candidates is not an answer
        return False
    try:
        answer = json.loads(objects[0])
        return any(answer.get("path", "").replace("\\", "/") == path
                   and type(answer.get("line")) is int and abs(answer["line"] - line) <= 1
                   for path, line in query["expected"])
    except (ValueError, AttributeError, TypeError):
        return False


def real_usage(usage: dict) -> bool:
    values = [usage.get(k) for k in ("input_tokens", "output_tokens")]
    return all(type(v) is int and v >= 0 for v in values) and sum(values) > 0


def codex_record(code: int, events: list[dict], tool: str, query: dict) -> dict:
    items = [e["item"] for e in events if "item" in e and e.get("type") == "item.completed"]
    final = next((i.get("text", "") for i in reversed(items) if i.get("type") == "agent_message"), "")
    usage = next((e.get("usage") or {} for e in reversed(events) if e.get("type") == "turn.completed"), {})
    calls = [i for i in items if i.get("type") == "mcp_tool_call" and i.get("server") == tool]
    successful = [i for i in calls if i.get("status") == "completed" and not i.get("error")
                  and isinstance(i.get("result"), dict) and not i["result"].get("isError")
                  and "requires approval" not in json.dumps(i).lower()]
    valid = code == 0 and real_usage(usage) and bool(successful) and len(successful) == len(calls)
    return {"exit_code": code, "usage": usage, "mcp_calls": len(successful), "valid": valid,
            "correct": valid and answer_correct(final, query), "final": final}


def claude_record(session: dict, tool: str, query: dict) -> dict:
    summary = summarize_session(session)
    calls, successful, failed = set(), set(), set()
    for event in session["events"]:
        message = event.get("message")
        blocks = message.get("content") if isinstance(message, dict) else None
        for block in blocks if isinstance(blocks, list) else []:
            if block.get("type") == "tool_use" and str(block.get("name", "")).startswith(f"mcp__{tool}__"):
                calls.add(block.get("id"))
            if block.get("type") == "tool_result" and block.get("tool_use_id") in calls:
                (failed if block.get("is_error") else successful).add(block["tool_use_id"])
    usage = {k: summary[k] for k in ("input_tokens", "output_tokens")}
    # A failed call is reported in mcp_errors; the session stays valid once the model recovers.
    valid = (summary["exit_code"] == 0 and real_usage(usage) and bool(successful)
             and not summary["limit_hit"] and summary["is_error"] is False
             and not summary["timed_out"])
    summary["attempted_mcp_call_count"] = len(calls)
    summary["mcp_call_count"] = len(successful - failed)
    cached = summary.get("cache_read_input_tokens") or 0
    processed = {"input_tokens": (usage["input_tokens"] or 0) + (summary.get("cache_creation_input_tokens") or 0) + cached,
                 "cached_input_tokens": cached, "output_tokens": usage["output_tokens"] or 0}
    return {"summary": summary, "usage": processed, "mcp_calls": len(successful - failed),
            "mcp_errors": len(failed), "valid": valid, "fatal": bool(summary["limit_hit"]),
            "correct": valid and answer_correct(summary["final_text"], query)}


def select_live_tasks(rows: list[dict], corpora: Path, count: int, skip: int = 0) -> list[dict]:
    """Deterministic live tasks: exact-symbol rows with one expected location and
    exactly one declaration of that name in the frozen corpus, round-robin by corpus.
    `skip` leaves out the first tasks so a later round can use fresh ones."""
    declaration = (r"(^|[^[:alnum:]_])(def|class|fn|struct|enum|trait|interface|type|function|const|let|var)"
                   r"[[:space:]]+{}([^[:alnum:]_]|$)")
    pools: dict[str, list[dict]] = {}
    for row in sorted(rows, key=lambda r: r["id"]):
        if row["type"] != "symbol_exact" or len(row["expected"]) != 1 or not re.fullmatch(r"\w+", row["query"]):
            continue
        found = subprocess.run(["git", "grep", "-I", "-n", "-E", declaration.format(row["query"])],
                               cwd=corpora / row["corpus"], capture_output=True, text=True,
                               encoding="utf-8", errors="replace").stdout.splitlines()
        if len(found) == 1 and found[0].split(":", 1)[0] == row["expected"][0][0]:
            pools.setdefault(row["corpus"], []).append(row)
    chosen = []
    while len(chosen) < count + skip and any(pools.values()):
        for corpus in sorted(pools):
            if pools[corpus] and len(chosen) < count + skip:
                chosen.append(pools[corpus].pop(0))
    return chosen[skip:]


def load_results(saved: Path) -> dict:
    results = json.loads(saved.read_text(encoding="utf-8")) if saved.exists() else {"sessions": [], "started": 0}
    if any(s.get("status") == "pending" for s in results["sessions"]):
        results["stopped"] = "Interrupted pending trial; no repeat, retries or fallback"
        write_json(saved, results)
    if results.get("stopped"):
        raise SystemExit(f"BLOCKED: {results['stopped']}")
    return results


def usage_summary(results: dict) -> dict:
    summary = {}
    for tool in TOOLS:
        rows = [s for s in results["sessions"] if s["tool"] == tool]
        counts = {"sessions": len(rows), "valid": sum(s["valid"] for s in rows),
                  "correct": sum(s["correct"] for s in rows), "mcp_calls": sum(s.get("mcp_calls", 0) for s in rows)}
        for key in ("input_tokens", "cached_input_tokens", "output_tokens", "reasoning_output_tokens"):
            counts[key] = sum(s.get("usage", {}).get(key, 0) for s in rows)
        counts["total_tokens"] = counts["input_tokens"] + counts["output_tokens"]
        counts["uncached_input_tokens"] = counts["input_tokens"] - counts["cached_input_tokens"]
        summary[tool] = counts
    return summary


@contextmanager
def trial_attempt(saved: Path, results: dict, record: dict, cap: int, halt_on_invalid: bool = True,
                  hard_cap: int = 12):
    if results.get("stopped") or results["started"] >= cap or cap > hard_cap:
        raise SystemExit("Session cap or stopped study; no additional calls authorized")
    if any(s["key"] == record["key"] for s in results["sessions"]):
        raise SystemExit("Trial already attempted; no repeat authorized")
    record.update(status="pending", valid=False, correct=False)
    results["started"] += 1
    results["sessions"].append(record)
    write_json(saved, results)
    try:
        yield record
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as exc:
        record.update(valid=False, correct=False, error=str(exc))
    except Exception as exc:  # harness bug: never leave the trial pending or spend more sessions
        record.update(valid=False, correct=False, error=repr(exc))
        results["stopped"] = f"harness error, study stopped: {exc!r}"
    record["status"] = "complete" if record["valid"] else "invalid"
    if not record["valid"] and not results.get("stopped") and (halt_on_invalid or record.get("fatal")):
        results["stopped"] = f"{record['client']}: failed, missing usage or unsuccessful supplied-server MCP call; no retries or fallback"
    write_json(saved, results)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(LIVE_FLAG, action="store_true")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--codex", type=Path, required=True)
    parser.add_argument("--cbm", type=Path, required=True)
    parser.add_argument("--graphify-python", type=Path, required=True)
    parser.add_argument("--max-sessions", type=int, default=12)
    parser.add_argument("--tasks", type=int, default=0,
                        help="pick this many single-definition tasks by rule instead of the manifest's live_query_ids")
    parser.add_argument("--skip-tasks", type=int, default=0, help="skip the first N rule-selected tasks (fresh tasks per round)")
    parser.add_argument("--client", choices=["both", "codex", "claude"], default="both")
    parser.add_argument("--codex-model", help="override the model from config.toml; drops service_tier")
    parser.add_argument("--codex-effort", help="override model_reasoning_effort from config.toml")
    args = parser.parse_args()
    args.out = args.out.resolve()
    if not args.enable_live_model_tests or os.environ.get(APPROVAL_ENV) != APPROVAL:
        raise SystemExit("NOT_RUN: explicit live flag and subscription approval required")
    if os.environ.get("CI") or os.environ.get("GITHUB_ACTIONS"):
        raise SystemExit("BLOCKED: live tests never run in CI")
    if not 1 <= args.max_sessions <= 12:
        raise SystemExit("BLOCKED: maximum 12 sessions; cannot widen the authorized cap")
    saved = args.out / "live-results.json"
    results = load_results(saved)
    manifest = json.loads((args.out / "manifest.json").read_text(encoding="utf-8"))
    if args.tasks:
        tasks = select_live_tasks(manifest["queries"], REPO / "target/tool-comparison/corpora", args.tasks, args.skip_tasks)
    else:
        tasks = [next(q for q in manifest["queries"] if q["id"] == key) for key in manifest["live_query_ids"]] if "live_query_ids" in manifest else [
            next(q for q in manifest["queries"] if q["corpus"] == corpus and q["type"] == "symbol_exact")
            for corpus in ("requests", "zod")]
    if not tasks or len({q["id"] for q in tasks}) != len(tasks) or (args.tasks and len(tasks) != args.tasks):
        raise SystemExit("BLOCKED: the live study needs the requested number of distinct frozen tasks")
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
    if args.client == "claude":
        codex_block = "NOT_RUN: Claude-only study"
    claude_status = {}
    if args.client != "codex" and shutil.which("claude"):
        status = subprocess.run(["claude", "auth", "status"], env=clean, capture_output=True,
                                text=True, encoding="utf-8", errors="replace", timeout=30)
        try:
            claude_status = json.loads(status.stdout)
        except ValueError:
            pass
    claude_block = live_model_gate(True, env=dict(clean, **{APPROVAL_ENV: APPROVAL}), status=claude_status)
    if args.client == "codex":
        claude_block = "NOT_RUN: Codex-only study"
    home = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex")))
    config = tomllib.loads((home / "config.toml").read_text(encoding="utf-8"))
    profile = {k: config[k] for k in ("model", "model_reasoning_effort", "service_tier") if k in config}
    if args.codex_model or args.codex_effort:
        profile.pop("service_tier", None)
        profile.update({k: v for k, v in (("model", args.codex_model),
                                          ("model_reasoning_effort", args.codex_effort)) if v})
    registered_cbm = config.get("mcp_servers", {}).get("codebase-memory", {})
    cbm_environment = {k: v for k, v in registered_cbm.get("env", {}).items()
                       if k in ("CBM_RUNTIME_DIR", "CBM_CACHE_DIR")}
    plan = {"tasks": [q["id"] for q in tasks], "tools": list(TOOLS), "max_sessions": args.max_sessions,
            "codex_profile": profile, "claude_model": "haiku",
            "blocked": {"codex": codex_block, "claude": claude_block}}
    write_json(args.out / "live-plan.json", plan)
    pending = sum(len(tasks) * len(TOOLS) for reason in (codex_block, claude_block) if reason is None)
    if results["started"] + pending > args.max_sessions and not results["sessions"]:
        raise SystemExit("BLOCKED: planned sessions exceed the cap")
    findex_state = Path(os.environ.get("FINDEX_LIVE_FINDEX_STATE") or REPO / "target/tool-comparison/state/findex")
    findex_bin = REPO / "target/release/findex.exe"

    def findex_env(corpus: str) -> dict:
        # The desktop-app sandbox can make the default %TEMP% and index paths unusable for SQLite.
        root = findex_state / corpus
        (root / "tmp").mkdir(parents=True, exist_ok=True)
        return {"LOCALAPPDATA": str(root), "TEMP": str(root / "tmp"), "TMP": str(root / "tmp")}

    if codex_block is None or claude_block is None:  # prove findex answers before spending any session
        for query in tasks:
            probe = subprocess.run([str(findex_bin), "find-symbol", query["query"]],
                                   cwd=REPO / "target/tool-comparison/corpora" / query["corpus"],
                                   env=dict(clean, **findex_env(query["corpus"])), capture_output=True,
                                   text=True, encoding="utf-8", errors="replace", timeout=120)
            if probe.returncode or query["expected"][0][0] not in probe.stdout.replace("\\", "/"):
                raise SystemExit(f"BLOCKED: findex preflight failed for {query['id']}: {probe.stderr.strip()[:200]}")
    for client, blocked in (("codex", codex_block), ("claude", claude_block)):
        if blocked:
            print(f"{client}: {blocked}", flush=True)
            continue
        for query in tasks:
            corpus = query["corpus"]
            repo = REPO / "target/tool-comparison/corpora" / corpus
            graph = REPO / "target/tool-comparison/state/graphify" / corpus / "graphify-out/graph.json"
            servers = {
                "findex": {"command": str(findex_bin), "args": ["mcp"], "env": findex_env(corpus)},
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
                question = f"Locate the definition of `{query['query']}`" if query["type"] == "symbol_exact" else f"Locate the definition best matching these code terms: `{query['query']}`"
                prompt = (f"{question} in this repository. "
                          f"Start with the supplied {tool} MCP server, then read source only if needed. "
                          "Do not edit files, install anything, index another project, or use the network. "
                          "Do not launch agents, subagents or other model sessions. "
                          "Return exactly one JSON object, no list and no prose: "
                          'path (repository-relative) and line (1-based definition), e.g. {"path":"src/a.rs","line":12}. ')
                if tool == "findex":
                    prompt += ('findex_context is loaded at session start: call it with the task as a plain sentence. '
                               'For a lookup tool use format="compact" with findex_find_symbol or findex_search; columns names the fields in each hits row. ')
                if tool == "codebase-memory":
                    prompt += f'Use existing indexed project `{projects[corpus]}`; do not reindex it. Use format="tree" for search_graph; prefer an anchored name_pattern for an exact symbol. '
                mcp = dest / "mcp.json"
                mcp.write_text(json.dumps({"mcpServers": {tool: server}}), encoding="utf-8")
                record = {"key": key, "client": client, "tool": tool, "task": query["id"]}
                with trial_attempt(saved, results, record, args.max_sessions, halt_on_invalid=False):
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
                        record.update(codex_record(proc.returncode, events, tool, query))
                        record["fatal"] = bool(re.search(r"usage limit|rate limit", proc.stdout + proc.stderr, re.I))
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
                        record.update(claude_record(session, tool, query))
                print(f"{key}: correct={record['correct']}", flush=True)
                if results.get("stopped"):
                    return
    write_json(args.out / "live-usage-summary.json", {"summary": usage_summary(results),
        "limits": f"{len(tasks)} tasks, one repetition per arm; cache differences included. Not evidence of general superiority or monetary savings."})


if __name__ == "__main__":
    main()
