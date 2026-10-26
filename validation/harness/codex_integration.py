"""Exercise the installed plugin through Codex app-server, without a model call.

Uses an isolated CODEX_HOME containing only the locally installed test plugin.
The native Codex client starts the MCP server and calls it on a disposable repo.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import tempfile
import threading
import time

from common import write_json
from plugin_check import EXPECTED_TOOLS


def fixture_evidence(result: dict) -> bool:
    if result.get("isError") is not False:
        return False
    return any(block.get("type") == "text" and re.search(
        r"^definition fixture\.rs:1 \[function codex_plugin_fixture\] pub fn codex_plugin_fixture\(\) \{\}\s*$",
        block.get("text", ""), re.M) for block in result.get("content", []))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", type=Path, required=True)
    parser.add_argument("--codex-home", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="findex-codex-") as tmp:
        root = Path(tmp)
        repo = root / "Project with spaces"
        repo.mkdir()
        (repo / "fixture.rs").write_text("pub fn codex_plugin_fixture() {}\n", encoding="utf-8")
        env = dict(os.environ, CODEX_HOME=str(args.codex_home.resolve()), LOCALAPPDATA=str(root / "state"))
        messages: queue.Queue = queue.Queue()
        proc = subprocess.Popen([str(args.codex), "app-server", "--stdio"],
                                cwd=repo, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace")
        errors = []

        def reader():
            for line in proc.stdout:
                messages.put(json.loads(line))
            messages.put(None)

        def stderr_reader():
            errors.extend(proc.stderr)

        threading.Thread(target=reader, daemon=True).start()
        threading.Thread(target=stderr_reader, daemon=True).start()
        seq = 0

        def request(method, params):
            nonlocal seq
            seq += 1
            proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": seq, "method": method, "params": params}) + "\n")
            proc.stdin.flush()
            deadline = time.monotonic() + 90
            while time.monotonic() < deadline:
                msg = messages.get(timeout=max(.1, deadline - time.monotonic()))
                if msg is None:
                    raise RuntimeError(f"Codex closed before {method}: {errors[-3:]}")
                if msg.get("id") == seq and ("result" in msg or "error" in msg):
                    if "error" in msg:
                        raise RuntimeError(msg["error"])
                    return msg["result"]
            raise TimeoutError(method)

        try:
            request("initialize", {"clientInfo": {"name": "findex-codex-check", "version": "1"},
                                   "capabilities": {"experimentalApi": True}})
            proc.stdin.write('{"jsonrpc":"2.0","method":"initialized"}\n')
            proc.stdin.flush()
            thread = request("thread/start", {"cwd": str(repo), "ephemeral": True,
                                             "approvalPolicy": "never", "sandbox": "read-only"})
            thread_id = thread["thread"]["id"]
            inventory = request("mcpServerStatus/list", {"threadId": thread_id})
            servers = inventory["data"]
            findex = next(s for s in servers if "findex" in s["name"])
            assert findex.get("pluginId") == "findex@findex-local", findex
            tools = set(findex["tools"])
            assert tools == EXPECTED_TOOLS, (findex, errors[-12:])
            result = request("mcpServer/tool/call", {"threadId": thread_id, "server": findex["name"],
                             "tool": "findex_context", "arguments": {
                                 "task": "where is `codex_plugin_fixture` defined", "budget_tokens": 1000}})
            assert fixture_evidence(result), result
            compact = request("mcpServer/tool/call", {"threadId": thread_id, "server": findex["name"],
                              "tool": "findex_find_symbol", "arguments": {
                                  "name": "codex_plugin_fixture", "format": "compact"}})
            assert compact.get("isError") is False, compact
            payload = json.loads(compact["content"][0]["text"])
            assert payload["sync"] == 0, "Installed plugin must expose the final compact encoding"
            hit, = [dict(zip(payload["columns"], row)) for row in payload["hits"]]
            assert hit["path"] == "fixture.rs" and hit["start_line"] == 1, hit
            assert hit["name"] == "codex_plugin_fixture" and hit["has_syntax_errors"] is False, hit
            report = {"status": "PASS", "client": "Codex app-server", "server": findex["name"],
                                 "tools": sorted(tools), "source_evidence": "PASS", "model_sessions": 0,
                                 "compact_source_evidence": "PASS", "compact_idle_sync": "PASS", "paid_api_calls": 0, "result": result}
        finally:
            proc.stdin.close()
            try:
                proc.wait(timeout=20)
            except subprocess.TimeoutExpired:
                proc.terminate()
                proc.wait(timeout=10)
            log = args.out.parent / "raw/codex-app-server.log"
            log.parent.mkdir(parents=True, exist_ok=True)
            log.write_text("".join(errors), encoding="utf-8")
    write_json(args.out, report)
    print("PASS: native Codex loaded the plugin, discovered 8 tools, and retrieved fixture source; zero model calls")


if __name__ == "__main__":
    main()
