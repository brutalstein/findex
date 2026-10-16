"""Offline Claude Code plugin contract, marketplace and Windows MCP launcher checks.

Validates the repository plugin and starts *real* findex.exe via the packaged
PowerShell launcher from a directory with spaces. No Claude sessions, APIs,
network, installer scripts, credentials or changes to user settings.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


EXPECTED_TOOLS = {
    "findex_context", "findex_search", "findex_find_symbol",
    "findex_symbols", "findex_impact", "findex_graph_symbol", "findex_capabilities",
}
SKILLS = {"navigate", "map-project", "trace-impact", "maintenance"}


def validate_files(plugin: Path, marketplace: Path) -> dict:
    manifest = json.loads((plugin / ".claude-plugin" / "plugin.json").read_text(encoding="utf-8"))
    config = json.loads((plugin / ".mcp.json").read_text(encoding="utf-8"))
    catalog = json.loads(marketplace.read_text(encoding="utf-8"))
    assert manifest["name"] == "findex"
    assert catalog["name"] == "findex-marketplace"
    assert any(x["name"] == "findex" and x["source"] == "./plugin" for x in catalog["plugins"])
    server = config["mcpServers"]["findex"]
    assert server["command"].lower() == "powershell.exe"
    argv = server["args"]
    assert "-NoProfile" in argv and "-NonInteractive" in argv
    assert "-File" in argv
    assert argv[-1] == "${CLAUDE_PLUGIN_ROOT}/scripts/start-mcp.ps1"
    assert all(key not in server for key in ("url", "headers", "env"))
    assert not (plugin / "hooks").exists(), "Hooks intentionally forbidden in non-invasive release"
    assert not (plugin / "CLAUDE.md").exists(), "Root CLAUDE.md is not a plugin entry point"
    found = set()
    for path in sorted((plugin / "skills").glob("*/SKILL.md")):
        body = path.read_text(encoding="utf-8")
        assert body.startswith("---\n"), path
        front = body.split("---\n", 2)[1]
        name = re.search(r"^name:\s*([a-z0-9-]+)\s*$", front, re.M)
        desc = re.search(r"^description:\s*(\S.+)$", front, re.M)
        assert name and desc, path
        assert name.group(1) == path.parent.name, path
        assert len(desc.group(1)) < 400, path
        found.add(name.group(1))
        if name.group(1) == "maintenance":
            assert "disable-model-invocation: true" in front
        else:
            assert "disable-model-invocation:" not in front
        assert "Claude" in body
    assert found == SKILLS, found
    return {"manifest": "PASS", "marketplace": "PASS",
            "mcp_config": "PASS", "skills": sorted(found)}


def call_launcher(script: Path, cwd: Path, env: dict[str, str],
                  messages: list[dict], expected_rc: int = 0) -> tuple[list[dict], str]:
    payload = "\n".join(json.dumps(x, separators=(",", ":")) for x in messages) + "\n"
    completed = subprocess.run(
        ["powershell.exe", "-NoLogo", "-NoProfile", "-NonInteractive",
         "-ExecutionPolicy", "Bypass", "-File", str(script)],
        input=payload, capture_output=True, text=True, cwd=cwd, env=env, timeout=75,
        encoding="utf-8", errors="replace",
    )
    assert completed.returncode == expected_rc, {
        "exit": completed.returncode, "expected": expected_rc,
        "stderr": completed.stderr[-900:], "stdout": completed.stdout[-900:],
    }
    frames = [json.loads(x) for x in completed.stdout.splitlines() if x.strip()]
    return frames, completed.stderr


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--marketplace", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    plugin = args.plugin.resolve(strict=True)
    runtime = args.binary.resolve(strict=True)
    report = validate_files(plugin, args.marketplace.resolve(strict=True))
    with tempfile.TemporaryDirectory(prefix="findex-plugin-test-") as tmp:
        workspace = Path(tmp) / "Project with spaces"
        workspace.mkdir()
        (workspace / "main.rs").write_text(
            "pub fn plugin_fixture() -> usize { 42 }\n", encoding="utf-8"
        )
        env = os.environ.copy()
        env["LOCALAPPDATA"] = str(Path(tmp) / "Local App Data")
        Path(env["LOCALAPPDATA"]).mkdir()
        installed = Path(tmp) / "Installed Plugin With Spaces"
        (installed / "scripts").mkdir(parents=True)
        (installed / "bin").mkdir()
        shutil.copy2(plugin / "scripts" / "start-mcp.ps1",
                     installed / "scripts" / "start-mcp.ps1")
        shutil.copy2(runtime, installed / "bin" / "findex.exe")
        launcher = installed / "scripts" / "start-mcp.ps1"

        request = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2025-11-25"}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            {"jsonrpc": "2.0", "id": 3, "method": "tools/call",
             "params": {"name": "findex_context",
                        "arguments": {"task": "where is `plugin_fixture` defined",
                                      "budget_tokens": 1100}}},
        ]
        env.pop("FINDEX_BIN", None)
        frames, errors = call_launcher(launcher, workspace, env, request)
        assert len(frames) == 3, frames
        assert frames[0]["result"]["serverInfo"]["name"] == "findex"
        actual_tools = {t["name"] for t in frames[1]["result"]["tools"]}
        assert actual_tools == EXPECTED_TOOLS, actual_tools
        upfront = {t["name"] for t in frames[1]["result"]["tools"]
                   if t.get("_meta", {}).get("anthropic/alwaysLoad")}
        assert upfront == {"findex_context"}, upfront
        assert frames[2]["result"]["isError"] is False, frames[2]
        excerpt = frames[2]["result"]["content"][0]["text"]
        assert "main.rs" in excerpt and "plugin_fixture" in excerpt, excerpt
        report["self_contained_mcp"] = "PASS"
        report["all_seven_tools"] = "PASS"
        report["on_demand_model_context"] = "PASS"
        report["fixture_source_evidence"] = "PASS"

        env["FINDEX_BIN"] = str(runtime)
        frames, _ = call_launcher(launcher, workspace, env, request[:2])
        assert len(frames) == 2 and frames[0]["result"]["serverInfo"]["name"] == "findex"
        report["explicit_runtime_override"] = "PASS"

        env["FINDEX_BIN"] = str(workspace / "missing" / "findex.exe")
        frames, errors = call_launcher(launcher, workspace, env, [], expected_rc=127)
        assert frames == [] and "FINDEX_BIN" in errors, errors
        report["missing_runtime_fails_closed"] = "PASS"

    report.update({"status": "PASS", "model_sessions": 0, "paid_api_calls": 0})
    output = json.dumps(report, indent=2, ensure_ascii=False)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(output + "\n", encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
