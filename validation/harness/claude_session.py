"""Run one isolated, non-interactive official Claude Code session.

Shared by the integration test and the A/B study. Each session gets a fresh
copy of a frozen corpus (git-initialised), its own Findex state directory and
an explicit MCP configuration. Credentials are never read or stored: the
`claude` executable uses its own existing authentication.
"""
from __future__ import annotations

import json
import re
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

from common import CORPORA, REPO, WORK, findex_binary

PROXY = REPO / "validation" / "harness" / "mcp_trace_proxy.py"

# Live-model gate: every path that starts the official `claude` client passes
# it first. Off by default; fails closed (NOT_RUN / BLOCKED) on anything ambiguous.
LIVE_FLAG = "--enable-live-model-tests"
APPROVAL_ENV = "FINDEX_LIVE_MODEL_APPROVAL"
APPROVAL = "I approve Claude subscription usage"
# Variables that route Claude Code to pay-as-you-go APIs or other providers.
BILLING_ENV = ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "CLAUDE_CODE_USE_BEDROCK",
               "CLAUDE_CODE_USE_VERTEX", "CLAUDE_CODE_USE_FOUNDRY", "AWS_BEARER_TOKEN_BEDROCK")
_gate_passed = False


def subscription_status(status: dict) -> str | None:
    """None for a first-party claude.ai subscription sign-in with no API key source; otherwise why not."""
    expected = {"loggedIn": True, "authMethod": "claude.ai", "apiProvider": "firstParty"}
    wrong = [key for key, value in expected.items() if status.get(key) != value]
    if wrong:
        return f"BLOCKED: `claude auth status` is not a claude.ai subscription sign-in ({', '.join(wrong)})"
    if not status.get("subscriptionType"):
        return "BLOCKED: `claude auth status` reports no subscription type"
    if status.get("apiKeySource") not in (None, "", "none"):
        return (f"BLOCKED: an API key source is configured ({status['apiKeySource']}); "
                "usage could be billed outside the subscription")
    return None


def auth_status() -> dict:
    """`claude auth status` (no model call). Empty when the client is missing or the output is unreadable."""
    if not shutil.which("claude"):
        return {}
    try:
        out = subprocess.run(["claude", "auth", "status"], capture_output=True, text=True, timeout=60)
        return json.loads(out.stdout)
    except (ValueError, OSError, subprocess.TimeoutExpired):
        return {}


def live_model_gate(enabled: bool, env: dict | None = None, status: dict | None = None) -> str | None:
    """None when live model sessions may start; otherwise a NOT_RUN/BLOCKED reason."""
    global _gate_passed
    env = os.environ if env is None else env
    if not enabled:
        return f"NOT_RUN: live model tests are off by default; pass {LIVE_FLAG}"
    if env.get("CI") or env.get("GITHUB_ACTIONS"):
        return "BLOCKED: live model tests never run in CI"
    if env.get(APPROVAL_ENV) != APPROVAL:
        return f'BLOCKED: operator approval missing: set {APPROVAL_ENV}="{APPROVAL}"'
    billing = sorted(k for k in BILLING_ENV if env.get(k))
    if billing:
        return f"BLOCKED: {', '.join(billing)} set; only the existing subscription sign-in is allowed"
    reason = subscription_status(auth_status() if status is None else status)
    _gate_passed = reason is None
    return reason


def fresh_workspace(corpus: str, dest: Path) -> Path:
    if dest.exists():
        # Git marks object files read-only; Windows refuses to unlink them.
        shutil.rmtree(dest, onexc=lambda func, path, _: (os.chmod(path, 0o666), func(path)))
    # Copy only the pinned commit's tracked files: build output left in a corpus
    # (e.g. ripgrep/target) is not part of it and overflows MAX_PATH on Windows.
    source = CORPORA / corpus
    repo = dest / "repo"
    tracked = subprocess.run(["git", "ls-files", "-z"], cwd=source, check=True, capture_output=True).stdout
    for name in filter(None, tracked.decode("utf-8").split("\0")):
        target = repo / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / name, target)
    for argv in (["init", "-q"], ["add", "-A"],
                 ["-c", "user.email=validation@local", "-c", "user.name=validation", "commit", "-q", "-m", "base"]):
        subprocess.run(["git", *argv], cwd=repo, check=True, capture_output=True)
    return repo


def mcp_config(dest: Path, binary: str, via: str = "proxy") -> Path:
    exe = findex_binary(binary)
    if not exe.exists():  # Claude Code would only report the server as "failed"
        raise FileNotFoundError(f"{exe} missing: build it or set CARGO_TARGET_DIR")
    server = {"command": sys.executable, "args": [str(PROXY), str(exe), "mcp"],
              "env": {"FINDEX_TRACE_FILE": str(dest / "trace.jsonl"), "LOCALAPPDATA": str(dest / "state")}}
    if via == "direct":
        server = {"command": str(exe), "args": ["mcp"], "env": {"LOCALAPPDATA": str(dest / "state")}}
    path = dest / "mcp.json"
    path.write_text(json.dumps({"mcpServers": {"findex": server}}, indent=2), encoding="utf-8")
    return path


def run_claude(repo: Path, prompt: str, *, model: str, max_turns: int, budget_usd: float,
               allowed_tools: list[str], disallowed_tools: list[str] | None = None,
               mcp_config_path: Path | None = None, extra: list[str] | None = None,
               effort: str | None = None, timeout: float = 900, executable: list[str] | None = None,
               env: dict | None = None, strict_mcp: bool = True, isolate_memory: bool = True) -> dict:
    if not _gate_passed:
        raise RuntimeError("live model gate not passed: call live_model_gate() first")
    argv = (executable or ["claude"]) + [
        "-p", prompt, "--model", model, "--max-turns", str(max_turns), "--max-budget-usd", str(budget_usd),
        "--output-format", "stream-json", "--verbose", "--no-session-persistence",
        # Only project/local settings: the operator's user-level hooks and
        # plugins must not contaminate either study arm.
        "--setting-sources", "project,local", "--permission-mode", "default",
        "--allowedTools", ",".join(allowed_tools)]
    if effort:
        argv += ["--effort", effort]
    if disallowed_tools:
        argv += ["--disallowedTools", ",".join(disallowed_tools)]
    if mcp_config_path:
        argv += ["--mcp-config", str(mcp_config_path), "--strict-mcp-config"]
    elif strict_mcp:
        # Strict mode also drops plugin-provided servers; plugin tests disable it.
        argv += ["--strict-mcp-config", "--mcp-config", '{"mcpServers":{}}']
    argv += extra or []
    if isolate_memory:
        # `--setting-sources` does not stop the operator's ~/.claude/CLAUDE.md
        # from loading (verified 2026-10-12); this documented switch does.
        env = dict(env or os.environ, CLAUDE_CODE_DISABLE_CLAUDE_MDS="1")
    started = time.perf_counter()
    try:
        proc = subprocess.run(argv, cwd=repo, capture_output=True, text=True, encoding="utf-8", errors="replace",
                              timeout=timeout, env=env)
        code, stdout, stderr, timed_out = proc.returncode, proc.stdout, proc.stderr, False
    except subprocess.TimeoutExpired as exc:
        code, stdout, stderr, timed_out = -1, exc.stdout or "", exc.stderr or "", True
    wall = time.perf_counter() - started
    events = []
    for line in stdout.splitlines():
        try:
            events.append(json.loads(line))
        except ValueError:
            pass
    return {"argv": [a if a != prompt else "<prompt>" for a in argv], "exit_code": code, "timed_out": timed_out,
            "wall_s": wall, "events": events, "stderr_tail": stderr[-2000:]}


def blocked_session(final_text: str, result: dict) -> bool:
    """True when the client stopped for a plan limit or authentication failure,
    or failed before any model call (no model usage at all)."""
    if re.search(r"hit your (session|usage|weekly|daily) limit|usage limit reached|failed to authenticate"
                 r"|oauth session expired|not logged in|please run /login", final_text, re.I):
        return True
    return bool(result.get("is_error")) and not result.get("modelUsage")


assert blocked_session("Failed to authenticate: OAuth session expired and could not be refreshed", {})
assert blocked_session("You've hit your session limit · resets 7:50am", {"modelUsage": {"m": {}}})
assert blocked_session("", {"is_error": True, "modelUsage": {}})
assert not blocked_session("The answer is dyn.", {"is_error": False, "modelUsage": {"m": {}}})


def summarize_session(session: dict, markers: tuple[str, ...] = ()) -> dict:
    """Usage and tool accounting from stream-json events. `markers` are
    strings whose first appearance in a tool result counts as the first
    relevant evidence (1-based tool call index)."""
    init = next((e for e in session["events"] if e.get("type") == "system" and e.get("subtype") == "init"), {})
    result = next((e for e in reversed(session["events"]) if e.get("type") == "result"), {})
    tool_calls, mcp_calls, files_read, tool_result_bytes = [], 0, set(), 0
    call_index, first_evidence, findex_inputs = {}, None, []
    for event in session["events"]:
        message = event.get("message") or {}
        for block in message.get("content") or [] if isinstance(message.get("content"), list) else []:
            if block.get("type") == "tool_use":
                tool_calls.append(block.get("name"))
                call_index[block.get("id")] = len(tool_calls)
                if "findex" in str(block.get("name")):
                    findex_inputs.append({"tool": block.get("name"), "input": json.dumps(block.get("input"))[:300]})
                mcp_calls += str(block.get("name", "")).startswith("mcp__")
                path = (block.get("input") or {}).get("file_path")
                if block.get("name") == "Read" and path:
                    files_read.add(path)
            if block.get("type") == "tool_result":
                content = block.get("content")
                serialized = json.dumps(content, ensure_ascii=False)
                tool_result_bytes += len(serialized.encode("utf-8"))
                if first_evidence is None and any(m in serialized for m in markers):
                    first_evidence = call_index.get(block.get("tool_use_id"))
    usage = result.get("usage") or {}
    final_text = result.get("result") or ""
    # Plan/session usage limits and expired sign-ins end a session early. Such a
    # session did not measure the arm and must not be scored as a wrong answer.
    limit_hit = blocked_session(final_text, result)
    return {
        "limit_hit": limit_hit,
        "exit_code": session["exit_code"], "timed_out": session["timed_out"], "wall_s": round(session["wall_s"], 2),
        "result_subtype": result.get("subtype"), "is_error": result.get("is_error"),
        "num_turns": result.get("num_turns"), "duration_api_ms": result.get("duration_api_ms"),
        "input_tokens": usage.get("input_tokens"), "output_tokens": usage.get("output_tokens"),
        "cache_creation_input_tokens": usage.get("cache_creation_input_tokens"),
        "cache_read_input_tokens": usage.get("cache_read_input_tokens"),
        "total_cost_usd_estimate": result.get("total_cost_usd"), "model_usage": result.get("modelUsage"),
        "tool_calls": tool_calls, "tool_call_count": len(tool_calls), "mcp_call_count": mcp_calls,
        "files_read": len(files_read), "tool_result_bytes": tool_result_bytes,
        "first_evidence_tool_call": first_evidence, "findex_inputs": findex_inputs,
        "toolsearch_calls": tool_calls.count("ToolSearch"),
        "builtin_search_calls": sum(tool_calls.count(n) for n in ("Grep", "Glob")),
        "read_calls": tool_calls.count("Read"),
        "init_mcp_servers": init.get("mcp_servers"), "init_model": init.get("model"),
        "init_tools_findex": [t for t in init.get("tools", []) if "findex" in t],
        "init_plugins": init.get("plugins"), "claude_code_version": init.get("claude_code_version"),
        "final_text": final_text[:4000],
    }


def trace(dest: Path) -> list[dict]:
    path = dest / "trace.jsonl"
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
