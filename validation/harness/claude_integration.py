"""Genuine Claude Code <-> Findex integration checks (small, budget-capped).

Records PASS / FAIL / BLOCKED into compatibility.json. BLOCKED when the
official client is missing or not authenticated; offline checks still run.
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

from claude_session import fresh_workspace, mcp_config, run_claude, summarize_session, trace
from common import REPO, WORK, findex_binary, results_dir, write_json

CHECKS: list[dict] = []


def check(name, status, detail="", evidence=None):
    CHECKS.append({"name": name, "status": status, "detail": str(detail)[:600], "evidence": evidence})
    print(f"[{status}] {name} {str(detail)[:140]}", file=sys.stderr)


def authenticated() -> bool:
    if not shutil.which("claude"):
        return False
    try:
        out = subprocess.run(["claude", "auth", "status"], capture_output=True, text=True, timeout=60)
        return bool(json.loads(out.stdout).get("loggedIn"))
    except (ValueError, OSError, subprocess.TimeoutExpired):
        return False


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default="haiku")
    parser.add_argument("--budget-usd", type=float, default=0.30, help="per session cap")
    parser.add_argument("--out", default=None)
    args = parser.parse_args()
    out = results_dir(args.out)
    base = WORK / "claude" / f"integration-{out.name}"
    exe = findex_binary("current")

    # Offline: CLI and plugin packaging checks that need no model.
    version = subprocess.run([str(exe), "--version"], capture_output=True, text=True)
    check("findex --version", "PASS" if version.returncode == 0 else "FAIL", version.stdout.strip())
    dist = base / "dist"
    shutil.rmtree(dist, ignore_errors=True)
    dist.mkdir(parents=True)
    shutil.copy2(exe, dist / "findex.exe")
    doctor = subprocess.run([str(dist / "findex.exe"), "doctor"], capture_output=True, text=True, cwd=REPO)
    check("plugin absent: doctor reports missing plugin",
          "PASS" if "missing" in doctor.stdout else "FAIL", doctor.stdout.strip().splitlines()[3:5])
    shutil.copytree(REPO / "plugin", dist / "plugin")
    doctor = subprocess.run([str(dist / "findex.exe"), "doctor"], capture_output=True, text=True, cwd=REPO)
    check("plugin present beside exe: doctor detects plugin and native claude.exe",
          "PASS" if "present" in doctor.stdout and "found" in doctor.stdout else "FAIL",
          doctor.stdout.strip().splitlines()[3:5])

    if not authenticated():
        check("real Claude Code session", "BLOCKED", "claude CLI missing or not authenticated")
        write_json(out / "compatibility.json", {"checks": CHECKS})
        return 0

    prompt = ("Use the findex_find_symbol MCP tool to find where the Rust type WatchedIndex is defined. "
              "Answer with exactly one line in the form path:line and nothing else.")
    expected = "crates/findex-watch/src/lib.rs:91"

    # 1. Explicit --mcp-config through the metadata-only trace proxy.
    dest = base / "mcp-config"
    repo = fresh_workspace("findex", dest)
    session = run_claude(repo, prompt, model=args.model, max_turns=4, budget_usd=args.budget_usd,
                         allowed_tools=["mcp__findex__findex_find_symbol"], mcp_config_path=mcp_config(dest, "current"))
    summary = summarize_session(session)
    messages = trace(dest)
    connected = any(s.get("name") == "findex" and s.get("status") == "connected" for s in summary["init_mcp_servers"] or [])
    check("claude: findex MCP server connected", "PASS" if connected else "FAIL", summary["init_mcp_servers"])
    check("claude: findex tools discovered", "PASS" if len(summary["init_tools_findex"]) >= 4 else "FAIL",
          summary["init_tools_findex"])
    init_versions = [m.get("protocolVersion") for m in messages if m.get("method") == "initialize"]
    modern = [m for m in messages if m.get("metaProtocolVersion")]
    check("claude: protocol negotiation observed", "PASS" if init_versions or modern else "FAIL",
          {"initialize_requested": init_versions,
           "initialize_answered": [m.get("protocolVersion") for m in messages if m.get("dir") == "server->client" and m.get("protocolVersion")],
           "modern_meta_requests": len(modern), "client": next((m.get("clientInfo") for m in messages if m.get("clientInfo")), None)})
    called = [m for m in messages if m.get("method") == "tools/call"]
    check("claude: genuine tools/call reached findex", "PASS" if any(m.get("tool") == "findex_find_symbol" for m in called) else "FAIL",
          [m.get("tool") for m in called])
    check("claude: correct answer from Findex evidence", "PASS" if expected in summary["final_text"] else "FAIL",
          summary["final_text"][:200])
    check("claude: session exit code 0", "PASS" if session["exit_code"] == 0 else "FAIL", session["stderr_tail"][-300:])
    evidence = {"mcp_config": summary}

    # 2. Bundled plugin discovered with --plugin-dir and findex.exe on PATH.
    dest = base / "plugin"
    repo = fresh_workspace("findex", dest)
    env = dict(**__import__("os").environ)
    env["PATH"] = str(dist) + ";" + env["PATH"]
    env["LOCALAPPDATA"] = str(dest / "state")  # Findex state only; Claude config uses USERPROFILE
    session = run_claude(repo, prompt, model=args.model, max_turns=4, budget_usd=args.budget_usd,
                         allowed_tools=["mcp__plugin_findex_findex__findex_find_symbol", "mcp__findex__findex_find_symbol"],
                         extra=["--plugin-dir", str(dist / "plugin")], env=env, strict_mcp=False)
    summary = summarize_session(session)
    plugin_tools = summary["init_tools_findex"]
    check("claude --plugin-dir: plugin MCP tools discovered", "PASS" if plugin_tools else "FAIL", plugin_tools)
    check("claude --plugin-dir: tool invoked and answer correct",
          "PASS" if summary["mcp_call_count"] >= 1 and expected in summary["final_text"] else "FAIL",
          {"calls": summary["tool_calls"], "answer": summary["final_text"][:200]})
    evidence["plugin_dir"] = summary

    # 3. `findex claude` launcher: native discovery, plugin injection, argument forwarding, exit code.
    dest = base / "launcher"
    repo = fresh_workspace("findex", dest)
    env = dict(**__import__("os").environ)
    env["LOCALAPPDATA"] = str(dest / "state")
    session = run_claude(repo, prompt, model=args.model, max_turns=4, budget_usd=args.budget_usd,
                         allowed_tools=["mcp__plugin_findex_findex__findex_find_symbol"],
                         executable=[str(dist / "findex.exe"), "claude"], env=env, strict_mcp=False)
    summary = summarize_session(session)
    check("findex claude launcher: forwards args, loads plugin, answer correct",
          "PASS" if session["exit_code"] == 0 and summary["mcp_call_count"] >= 1 and expected in summary["final_text"] else "FAIL",
          {"exit": session["exit_code"], "calls": summary["tool_calls"], "answer": summary["final_text"][:160],
           "stderr": session["stderr_tail"][-200:]})
    evidence["launcher"] = summary
    write_json(out / "compatibility.json", {"checks": CHECKS, "sessions": evidence,
                                            "note": "trace proxy stores protocol metadata only; no tool payloads"})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
