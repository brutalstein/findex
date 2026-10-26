"""Findex on an SMB network path: index, queries, MCP watcher freshness.

The project is created on a local disk and opened through the loopback SMB
administrative share (`\\\\localhost\\C$\\...`), so every file operation goes
through the Windows SMB client redirector and server. A remote file server
with different latency, caching or permissions is not covered.
LOCALAPPDATA is isolated.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from mcp_client import McpProcess


def run(exe: Path, cwd: Path, env: dict[str, str], *args: str) -> str:
    done = subprocess.run([str(exe), *args], cwd=cwd, env=env, text=True, capture_output=True,
                          timeout=120, encoding="utf-8", errors="replace")
    assert done.returncode == 0, (args, done.stdout, done.stderr)
    return done.stdout


def paths(mcp: McpProcess, query: str) -> set[str]:
    payload, text, _, error = mcp.call_tool("findex_search", {"query": query, "limit": 10})
    assert not error and payload is not None, text
    return {hit["path"] for hit in payload["hits"]}


def wait_until(mcp: McpProcess, query: str, expected: set[str], limit: float = 15.0) -> float | None:
    """Seconds until the search for `query` returns exactly `expected`, or None."""
    started = time.perf_counter()
    while time.perf_counter() - started < limit:
        if paths(mcp, query) == expected:
            return round(time.perf_counter() - started, 2)
        time.sleep(0.25)
    return None


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path)
    opts = parser.parse_args()
    exe = opts.binary.resolve(strict=True)
    checks: dict[str, object] = {}
    with tempfile.TemporaryDirectory(prefix="findex-smb-") as temp:
        local = Path(temp).resolve() / "project"
        local.mkdir()
        subprocess.run(["git", "init", "-q", str(local)], check=True)
        (local / "a.rs").write_text("pub fn smb_marker() {}\n", encoding="utf-8")
        (local / "b.rs").write_text("fn user() { smb_marker(); }\n", encoding="utf-8")
        drive, rest = os.path.splitdrive(str(local))
        unc = Path(f"\\\\localhost\\{drive[0]}$" + rest)
        if not unc.is_dir():
            raise SystemExit(f"loopback administrative share unavailable: {unc}")
        env = os.environ.copy()
        env["LOCALAPPDATA"] = str(Path(temp) / "state")
        Path(env["LOCALAPPDATA"]).mkdir()

        checks["index"] = run(exe, unc, env, "index").strip()
        checks["cli_search_hits"] = run(exe, unc, env, "search", "smb_marker").count(".rs:")
        context = run(exe, unc, env, "context", "who calls smb_marker")
        checks["cli_context_call_found"] = "call b.rs:1" in context and "every code occurrence" in context
        checks["project_identity"] = "UNC" in run(exe, unc, env, "status").splitlines()[0]

        mcp = McpProcess(exe, unc, env)
        try:
            checks["mcp_initial"] = sorted(paths(mcp, "smb_marker"))
            (unc / "a.rs").write_text("pub fn smb_edited_marker() {}\n", encoding="utf-8")
            checks["edit_seconds"] = wait_until(mcp, "smb_edited_marker", {"a.rs"})
            (unc / "b.rs").unlink()
            checks["delete_seconds"] = wait_until(mcp, "smb_marker", set())
            (unc / "c.rs").write_text("pub fn smb_created_marker() {}\n", encoding="utf-8")
            checks["create_seconds"] = wait_until(mcp, "smb_created_marker", {"c.rs"})
            payload, _, _, _ = mcp.call_tool("findex_search", {"query": "smb_created_marker"})
            checks["mcp_mode"] = {"retrieval": payload["retrieval"], "examined": payload["sync"]["examined"]}
        finally:
            checks["mcp_exit"] = mcp.close()

    passed = (checks["cli_search_hits"] == 2 and checks["cli_context_call_found"]
              and checks["project_identity"] and checks["mcp_initial"] == ["a.rs", "b.rs"]
              and all(checks[k] is not None for k in ("edit_seconds", "delete_seconds", "create_seconds"))
              and checks["mcp_exit"] == 0)
    result = {"model_sessions": 0, "paid_calls": 0, "verdict": "PASS" if passed else "FAIL",
              "scope": "loopback SMB administrative share", "checks": checks}
    text = json.dumps(result, indent=2)
    if opts.output:
        opts.output.write_text(text + "\n", encoding="utf-8")
    print(text)
    raise SystemExit(0 if passed else 1)


if __name__ == "__main__":
    main()
