"""Paired native MCP stale-prefix regression; no model or network calls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "validation/harness"))
from common import CommandLog, write_json
from mcp_client import McpProcess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir()
    scratch = ROOT / "target/retrieval-integrity/semantic-refill"
    scratch.mkdir()
    log = CommandLog(out)
    rows = {}
    for name, binary in (("baseline", args.baseline.resolve()), ("candidate", args.candidate.resolve())):
        project = scratch / name
        project.mkdir()
        (project / ".git").mkdir()
        for filename, source in (("a.rs", "fn first() { parse(); }\n"),
                                 ("b.rs", "fn second() { parse(); }\n"),
                                 ("c.rs", "fn parse() {}\n")):
            (project / filename).write_text(source, encoding="utf-8")
        snapshot = {"definition": {"path": "c.rs", "line": 0, "character": 3},
                    "references": [{"path": "a.rs", "line": 0, "character": 13},
                                   {"path": "b.rs", "line": 0, "character": 14}]}
        (project / "capture.json").write_text(json.dumps(snapshot), encoding="utf-8")
        env = dict(os.environ, LOCALAPPDATA=str(project / "state"))
        result = log.run([str(binary), "graph-import-lsp", "capture.json"], cwd=project, env=env, timeout=60)
        assert result.returncode == 0, result.stderr
        for filename in ("a.rs", "b.rs"):
            (project / filename).write_text("fn changed() {}\n", encoding="utf-8")
        client = McpProcess(binary, project, env)
        try:
            report, text, seconds, error = client.call_tool("findex_graph_symbol", {"symbol": "lsp:c.rs:0:3", "limit": 1})
            assert not error, text
            passed = ([site["path"] for site in report["sites"]] == ["c.rs"]
                      and report["stale_evidence_discarded"] == 2 and not report["truncated"])
            rows[name] = {"sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                          "oracle_pass": passed, "result": report}
        finally:
            client.close()
        write_json(out / "results.json", rows)
    assert not rows["baseline"]["oracle_pass"], "baseline no longer reproduces the defect"
    assert rows["candidate"]["oracle_pass"], "candidate must retain the live site behind stale rows"
    print("Native stale-prefix oracle: baseline FAIL, candidate PASS", flush=True)


if __name__ == "__main__":
    main()
