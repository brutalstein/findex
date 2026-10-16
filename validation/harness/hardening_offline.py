"""Zero-model fault and compatibility tests against the native Windows Findex binary.

The fixture uses a disposable LOCALAPPDATA. It never alters a user's existing
indexes, providers, credentials or global Claude settings.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def execute(exe: Path, cwd: Path, env: dict[str, str], *args: str,
            input: str | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(exe), *args], cwd=cwd, env=env, input=input,
        text=True, capture_output=True, timeout=120,
        encoding="utf-8", errors="replace",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path)
    opts = parser.parse_args()
    exe = opts.binary.resolve(strict=True)
    results: dict[str, object] = {
        "model_sessions": 0,
        "paid_calls": 0,
        "checks": {},
    }
    checks: dict[str, str] = results["checks"]  # type: ignore[assignment]
    with tempfile.TemporaryDirectory(prefix="findex-hardening-offline-") as temp:
        root = Path(temp)
        project = root / "isolated source with spaces"
        project.mkdir()
        source = project / "app.rs"
        source.write_text("pub fn safe_marker() -> u32 { 17 }\n", encoding="utf-8")
        env = os.environ.copy()
        env["LOCALAPPDATA"] = str(root / "isolated state")
        Path(env["LOCALAPPDATA"]).mkdir()
        initial = execute(exe, project, env, "index")
        assert initial.returncode == 0, initial.stderr
        health = execute(exe, project, env, "index-check")
        assert health.returncode == 0 and "1 files, 1 documents" in health.stdout, health
        checks["baseline_integrity"] = "PASS"

        dbs = list((Path(env["LOCALAPPDATA"]) / "Findex" / "indexes").glob("*/index.sqlite3"))
        assert len(dbs) == 1, dbs
        db = dbs[0]
        # No Findex process holds a connection at this point. Tamper with
        # the derived cache only; preserve the original bytes as evidence.
        original = b"NOT_SQLITE_PRIVATE_MARKER_" * 512
        for extra in ["-wal", "-shm", "-journal"]:
            Path(str(db) + extra).unlink(missing_ok=True)
        db.write_bytes(original)
        corrupted = execute(exe, project, env, "index")
        assert corrupted.returncode != 0, corrupted.stdout
        assert db.read_bytes() == original, "automatic rebuild destroyed corrupted cache"
        assert source.read_text(encoding="utf-8").startswith("pub fn safe_marker")
        checks["corrupt_cache_preserved"] = "PASS"

        failure = execute(exe, project, env, "index-check")
        assert failure.returncode != 0, "corrupt cache reported healthy"
        checks["integrity_rejects_corruption"] = "PASS"

        frames = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2025-11-25"}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
             "params": {"name": "findex_context",
                        "arguments": {"task": "who calls `safe_marker`"}}},
            {"jsonrpc": "2.0", "id": 3, "method": "tools/call",
             "params": {"name": "findex_capabilities", "arguments": {}}},
        ]
        proc = execute(
            exe, project, env, "mcp",
            input="\n".join(json.dumps(f, separators=(",", ":")) for f in frames) + "\n",
        )
        assert proc.returncode == 0, proc.stderr[-500:]
        replies = [json.loads(row) for row in proc.stdout.splitlines() if row.strip()]
        assert len(replies) == 3, replies
        message = replies[1]["result"]["content"][0]["text"]
        assert replies[1]["result"]["isError"] is True
        assert "Read/Grep/Glob" in message, message
        assert str(project).lower() not in message.lower()
        assert str(db).lower() not in message.lower()
        assert "PRIVATE_MARKER" not in message
        assert "sqlite" not in message.lower()
        checks["mcp_private_errors_and_fallback"] = "PASS"
        cap = json.loads(replies[2]["result"]["content"][0]["text"])
        assert cap["relationships"]["complete_typed_call_graph"] is False
        assert cap["source_policy"]["max_admitted_files_per_scan"] == 100000
        assert cap["models_or_network_calls_per_query"] == 0
        checks["capability_contract_works_with_corrupt_cache"] = "PASS"

    results["status"] = "PASS"
    output = json.dumps(results, indent=2)
    if opts.output:
        opts.output.parent.mkdir(parents=True, exist_ok=True)
        opts.output.write_text(output + "\n", encoding="utf-8")
    print(output)


if __name__ == "__main__":
    main()
