"""Native bounded-body regression: wide sources, equal hits, alternating warm calls.

Fresh indexes are not timed; this isolates SQL retrieval after MCP startup.
No models or APIs. Assertions are the harness's deterministic self-checks.
"""
from __future__ import annotations

import argparse
from contextlib import ExitStack
import hashlib
import os
from pathlib import Path
import shutil
import tempfile

from common import REPO, environment, percentiles, write_json
from mcp_client import McpProcess
from payload_cost import expand_hits


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, default=REPO / "target/release/findex.exe")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        raise ValueError("Preserve attempts; choose a new output directory")
    args.out.mkdir(parents=True)
    binaries = {"baseline": args.baseline.resolve(strict=True), "candidate": args.candidate.resolve(strict=True)}
    hashes = {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()}
    count, body_bytes, repetitions = 1200, 65536, 10
    write_json(args.out / "manifest.json", {"binary_sha256": hashes, "files": count, "body_bytes": body_bytes,
        "repetitions": repetitions, "environment": environment(), "model_sessions": 0,
        "scope": "Synthetic equal-rank wide-body common-term queries, OS caches not flushed"})
    results = {name: [] for name in binaries}
    with tempfile.TemporaryDirectory(prefix="findex-wide-query-") as folder, ExitStack() as stack:
        root = Path(folder)
        clients = {}
        for name, source in binaries.items():
            binary = root / name / "findex.exe"
            binary.parent.mkdir()
            shutil.copy2(source, binary)
            assert hashlib.sha256(binary.read_bytes()).hexdigest() == hashes[name]
            project = root / name / "project"
            project.mkdir()
            (project / ".git").mkdir()
            for number in range(count):
                header = f"pub fn wide_marker_{number:05}() {{}}\n// "
                (project / f"{number:05}.rs").write_text(header + "x" * (body_bytes - len(header) - 1) + "\n", encoding="utf-8")
            state = root / name / "state"
            state.mkdir()
            clients[name] = McpProcess(binary, project, dict(os.environ, LOCALAPPDATA=str(state)))
            stack.callback(clients[name].close)
            assert not clients[name].call_tool("findex_search", {"query": "warmup_marker", "limit": 1})[3]
        reference = None
        for rep in range(repetitions):
            order = list(clients) if rep % 2 == 0 else list(reversed(clients))
            for name in order:
                payload, text, seconds, error = clients[name].call_tool("findex_search", {"query": "pub", "limit": 8, "format": "compact"})
                expanded = expand_hits(payload or {})
                assert not error and len(expanded["hits"]) == 8 and expanded["truncated"] is True
                assert expanded["sync"]["examined"] == 0
                assert [hit["path"] for hit in expanded["hits"]] == [f"{i:05}.rs" for i in range(8)]
                assert all(hit["line"] == 1 for hit in expanded["hits"])
                if reference is None:
                    reference = expanded
                assert expanded == reference, "SQL optimization changed evidence or diagnostics"
                results[name].append({"seconds": seconds, "chars": len(text), "memory": clients[name].memory()})
        write_json(args.out / "results.json", {"status": "PASS", "equal_payloads": repetitions * len(clients),
            "records": results, "latency_s": {name: percentiles([r["seconds"] for r in rows]) for name, rows in results.items()}})
    print("PASS: 20 equivalent native wide-source responses", flush=True)


if __name__ == "__main__":
    main()
