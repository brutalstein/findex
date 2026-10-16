"""Windows offline source-backed project discovery smoke: CLI and MCP.

No Claude models, external services or user caches. Tests a disposable mixed-
language project, manifest/entrypoint candidate precision and stale removals.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def run(exe: Path, root: Path, env: dict[str, str], *args: str,
        payload: str | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(exe), *args], cwd=root, env=env, input=payload,
        text=True, capture_output=True, timeout=180,
        encoding="utf-8", errors="replace",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("executable", type=Path)
    parser.add_argument("--output", type=Path)
    opts = parser.parse_args()
    executable = opts.executable.resolve(strict=True)
    evidence = {"status": "NOT_RUN", "model_calls": 0, "tests": {}}
    with tempfile.TemporaryDirectory(prefix="findex-project-overview-") as directory:
        tmp = Path(directory)
        project = tmp / "New project with spaces"
        project.mkdir()
        examples = {
            "Cargo.toml": "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
            "crates/core/src/main.rs": "fn main() {}\n",
            "crates/core/src/lib.rs": "pub fn core() {}\n",
            "packages/web/package.json": "{\"name\": \"web\"}\n",
            "packages/web/src/index.ts": "export const entry = true;\n",
            "tests/test_core.py": "def test_core(): pass\n",
        }
        for relative, body in examples.items():
            target = project / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(body, encoding="utf-8")
        (project / ".env").write_text("TOP_SECRET_SHOULD_NOT_INDEX", encoding="utf-8")
        env = os.environ.copy()
        env["LOCALAPPDATA"] = str(tmp / "local state")
        Path(env["LOCALAPPDATA"]).mkdir()
        report = run(executable, project, env, "overview")
        assert report.returncode == 0, report.stderr
        data = json.loads(report.stdout)
        assert data["indexed_files"] == len(examples), data
        assert data["probable_test_files"] == 1, data
        assert data["stale_landmarks_discarded"] == 0, data
        assert not data["truncated"], data
        assert set(data["manifest_candidates"]) == {
            "Cargo.toml", "packages/web/package.json",
        }, data
        assert set(data["entrypoint_candidates"]) == {
            "crates/core/src/main.rs", "packages/web/src/index.ts",
        }, data
        modules = {x["name"]: x["files"] for x in data["top_level_modules"]}
        assert modules["crates/core"] == 2, modules
        assert modules["packages/web"] == 2, modules
        assert "TOP_SECRET" not in report.stdout
        evidence["tests"]["project_counts_and_source_landmarks"] = "PASS"

        messages = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2025-11-25"}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            {"jsonrpc": "2.0", "id": 3, "method": "tools/call",
             "params": {"name": "findex_overview", "arguments": {}}},
        ]
        response = run(executable, project, env, "mcp", payload="\n".join(
            json.dumps(x, separators=(",", ":")) for x in messages) + "\n")
        assert response.returncode == 0, response.stderr
        replies = [json.loads(line) for line in response.stdout.splitlines() if line.strip()]
        assert len(replies) == 3, replies
        assert "findex_overview" in {
            tool["name"] for tool in replies[1]["result"]["tools"]
        }
        assert replies[2]["result"]["isError"] is False, replies[2]
        native = json.loads(replies[2]["result"]["content"][0]["text"])
        assert native["indexed_files"] == len(examples)
        assert native["entrypoint_candidates"] == data["entrypoint_candidates"]
        evidence["tests"]["native_mcp_optional_discovery"] = "PASS"

        (project / "crates/core/src/main.rs").unlink()
        after = run(executable, project, env, "overview")
        assert after.returncode == 0, after.stderr
        updated = json.loads(after.stdout)
        assert updated["indexed_files"] == len(examples) - 1, updated
        assert "crates/core/src/main.rs" not in updated["entrypoint_candidates"]
        evidence["tests"]["source_removal_not_cited"] = "PASS"
    evidence["status"] = "PASS"
    output = json.dumps(evidence, ensure_ascii=False, indent=2)
    if opts.output:
        opts.output.parent.mkdir(parents=True, exist_ok=True)
        opts.output.write_text(output + "\n", encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
