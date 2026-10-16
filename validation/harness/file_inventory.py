"""Tracked-file inventory and frozen-evidence manifest.

Hashes every tracked file as stored at HEAD (`git show HEAD:<path>`, so
working-copy line endings do not matter), verifies every archived
`checksums.sha256`, and writes:
  validation/campaigns/enterprise/FILE_AUDIT.md        (per-file table)
  validation/campaigns/enterprise/baseline_manifest.json (hash of each result file)
Usage: file_inventory.py
"""
from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / "validation" / "campaigns" / "enterprise"

# One line per active file: purpose and the review notes that matter.
PURPOSE = {
    ".cargo/config.toml": "cargo build settings (static CRT for the MSVC target)",
    ".gitattributes": "line-ending policy; results stored byte-exact (-text)",
    ".github/CODEOWNERS": "review ownership",
    ".github/dependabot.yml": "dependency update policy (cargo, actions)",
    ".github/workflows/ci.yml": "Windows CI: fmt, clippy, tests, live-gate test, release build, MCP smokes, ZIP+SBOM, attestation; RustSec audit; MSRV 1.85",
    ".gitignore": "excludes target/, local validation state and raw transcripts",
    "AGENTS.md": "agent rules: Claude separation, ranking-change evidence, Windows gate",
    "CONTRIBUTING.md": "contribution rules",
    "Cargo.lock": "locked dependency graph (CI checks --locked)",
    "Cargo.toml": "workspace manifest, MSRV, lints",
    "LICENSE": "license text",
    "README.md": "user documentation; stale in places (see COMPLETE_REPOSITORY_AUDIT D-list)",
    "SECURITY.md": "security policy; stale SBOM/audit claim (D2)",
    "rust-toolchain.toml": "pinned toolchain channel and components",
    "plugin/.claude-plugin/plugin.json": "Claude Code plugin manifest (MCP server only)",
    "plugin/.mcp.json": "plugin MCP server definition: `findex mcp`",
    "crates/findex-core/Cargo.toml": "core crate manifest (blake3, regex)",
    "crates/findex-core/src/lib.rs": "project root, relative-key normalization, storage dir, scan exclusions, sensitive names, excerpt redaction",
    "crates/findex-index/Cargo.toml": "index crate manifest (rusqlite bundled, ignore, blake3)",
    "crates/findex-index/src/lib.rs": "SQLite/FTS5 schema v4 (secure delete), newer-schema refusal, full reconciliation, BM25 search with definition stage, hash-verified results, corrupt-cache rebuild",
    "crates/findex-index/src/context.rs": "findex_context: EN/TR intent router, file intent, identifier extraction, definitions, per-language comment lexer, lexical call sites, coverage line, whole-report budget packing, rendering",
    "crates/findex-index/src/refresh.rs": "targeted refresh of already-indexed paths (watcher path); never admits new paths; keeps locked files",
    "crates/findex-index/src/symbols.rs": "persistent Tree-sitter definition table keyed by content hash + extractor revision",
    "crates/findex-index/tests/windows_fs.rs": "Windows filesystem behaviour: junctions, symlinks, long paths, case, locks",
    "crates/findex-mcp/Cargo.toml": "MCP crate manifest",
    "crates/findex-mcp/src/lib.rs": "stdio JSON-RPC MCP server: framing (1 MiB), version negotiation 2024-11-05..2026-07-28, 5 tools, instructions/trust rule",
    "crates/findex-relations/Cargo.toml": "relations crate manifest (scip, protobuf)",
    "crates/findex-relations/src/lib.rs": "semantic-reference store (SCIP/LSP), admission + hash checks, file-level impact BFS",
    "crates/findex-relations/src/scip_import.rs": "bounded SCIP protobuf import (16 MiB, 1000 docs, 100k occurrences)",
    "crates/findex-relations/src/lsp.rs": "captured LSP reference snapshot import",
    "crates/findex-syntax/Cargo.toml": "syntax crate manifest (tree-sitter grammars)",
    "crates/findex-syntax/src/lib.rs": "Tree-sitter definition extraction for Rust, Python, TS/TSX/JS, C++ (2048-symbol cap)",
    "crates/findex-watch/Cargo.toml": "watch crate manifest (notify)",
    "crates/findex-watch/src/lib.rs": "native watcher index: bounded queue, overflow/rename/create -> full reconcile, 120 s periodic reconcile, scan fallback",
    "crates/findex-cli/Cargo.toml": "CLI crate manifest",
    "crates/findex-cli/src/main.rs": "CLI commands, `findex claude` launcher of the official native claude.exe, doctor/status",
    "docs/architecture.md": "architecture overview; titled v0.1, no findex_context (D3)",
    "docs/roadmap.md": "milestones and checkboxes",
    "docs/incremental-refresh.md": "refresh design",
    "docs/adr/0001-native-cli-and-local-index.md": "ADR: native CLI + local index",
    "docs/adr/0002-watched-mcp-index.md": "ADR: watcher-backed MCP index",
    "docs/adr/0003-tree-sitter-definitions.md": "ADR: Tree-sitter definitions",
    "docs/adr/0004-persistent-symbol-index.md": "ADR: persistent symbol index",
    "docs/adr/0005-build-provenance.md": "ADR: SBOM + attestation, unsigned binaries",
    "docs/adr/0006-scip-lsp-evidence-graph.md": "ADR: SCIP/LSP evidence graph",
    "validation/README.md": "how to run validation; live-model gate rules",
    "validation/FINAL_VALIDATION_REPORT.md": "first production-validation campaign report",
    "validation/REMAINING_LIMITATIONS.md": "numbered limitation register",
    "validation/acceptance-criteria.md": "acceptance criteria of the first campaign",
    "validation/methodology.md": "measurement methodology",
    "validation/requirements.md": "harness requirements",
    "validation/test-matrix.md": "test matrix of the first campaign",
    "validation/golden/symbol_navigation.jsonl": "642 symbol queries; labels by the same author (bias noted in REMAINING_LIMITATIONS #1)",
    "validation/golden/issue_localization.jsonl": "65 real fixed issues (title -> changed non-test files), dev/test split by parity",
    "validation/research/ENGINEERING_DECISIONS.md": "decision log D1-D9 with evidence",
    "validation/harness/ab_stats.py": "paired bootstrap statistics for A/B runs",
    "validation/harness/bootstrap.py": "paired bootstrap for localization",
    "validation/harness/callsites.py": "call sites vs rust-analyzer SCIP oracle",
    "validation/harness/claude_ab.py": "Claude Code A/B study; behind the live-model gate, --max-sessions 12",
    "validation/harness/claude_integration.py": "offline CLI/plugin checks + gated real-session checks",
    "validation/harness/claude_session.py": "live-model gate, isolated workspace copy, one Claude session, stream-json accounting",
    "validation/harness/common.py": "paths, command log, environment record (no claude probe), JSON writing with path sanitizing",
    "validation/harness/corpora.py": "pinned corpora clone/verify, frozen baseline build",
    "validation/harness/coverage_check.py": "coverage-line claims vs git grep -w, independent comment lexer",
    "docs/engineering/COMPLETE_REPOSITORY_AUDIT.md": "repository audit: findings A1-A12, D1-D4 with status",
    "validation/harness/budget_check.py": "rendered findex context size vs requested token budget",
    "crates/findex-index/src/router_cases.tsv": "router dev set: 260 labeled EN/TR/mixed tasks",
    "crates/findex-index/src/router_heldout.tsv": "router held-out set: 60 tasks, frozen before first run",
    "validation/harness/file_inventory.py": "this inventory and the frozen-evidence manifest",
    "validation/harness/golden.py": "golden-set admission rules and symbol selection",
    "validation/harness/issues.py": "builds the issue-localization set from fix commits",
    "validation/harness/localization.py": "issue localization benchmark (dev/test split)",
    "validation/harness/mcp_client.py": "minimal MCP stdio client for tests",
    "validation/harness/mcp_trace_proxy.py": "MCP trace proxy for session accounting",
    "validation/harness/perf.py": "indexing/query performance and memory measurements",
    "validation/harness/relations.py": "relations/impact validation",
    "validation/harness/reliability.py": "branch-switch and OneDrive reliability checks",
    "validation/harness/report.py": "run report: manifest, regressions, failures, checksums",
    "validation/harness/retrieval.py": "642-query symbol retrieval benchmark",
    "validation/harness/sbom.py": "CycloneDX SBOM from Cargo.lock",
    "validation/harness/security.py": "30 black-box security checks",
    "validation/harness/test_live_gate.py": "proves offline runs start no live model client (spawn shim)",
    "validation/harness/unit.py": "runs cargo test and records results",
}
GROUPS = [("validation/campaigns/", "campaign report (vNext/enterprise)"),
          ("validation/scripts/", "PowerShell entry point for one validation component")]


def git(*args: str) -> bytes:
    return subprocess.run(["git", "-C", str(REPO), *args], check=True, capture_output=True).stdout


def kind(path: str) -> str:
    if path.startswith("validation/results/"):
        return "archived evidence"
    if path.startswith("validation/golden/"):
        return "fixture (golden data)"
    if path.endswith(".rs") and "/tests/" in path:
        return "test"
    if path.endswith((".rs", ".py", ".ps1")):
        return "active code"
    if path.endswith((".toml", ".json", ".yml", ".lock")) or path.startswith(".") or path == "LICENSE":
        return "configuration"
    return "documentation"


def tests_in(path: str, text: str) -> int:
    if path.endswith(".rs"):
        return len(re.findall(r"#\[test\]", text))
    if path.endswith(".py"):
        return len(re.findall(r"^\s*def test_", text, re.M)) + len(re.findall(r"^assert ", text, re.M))
    return 0


def local_deps(path: str, text: str) -> str:
    if path.endswith(".rs"):
        return " ".join(sorted(set(re.findall(r"\b(findex_[a-z]+)::", text)) - {path.split("/")[1].replace("-", "_")}))
    if path.endswith(".py"):
        names = {p.stem for p in (REPO / "validation" / "harness").glob("*.py")}
        return " ".join(sorted(set(re.findall(r"^from (\w+) import", text, re.M)) & names))
    return ""


def purpose(path: str) -> str:
    if path in PURPOSE:
        return PURPOSE[path]
    return next((text for prefix, text in GROUPS if path.startswith(prefix)), "")


def verify_checksums(files: list[str]) -> list[dict]:
    out = []
    for listing in [f for f in files if f.endswith("checksums.sha256")]:
        base = listing.rsplit("/", 1)[0]
        bad, total = [], 0
        for line in git("show", f"HEAD:{listing}").decode().splitlines():
            digest, name = line.split(maxsplit=1)
            total += 1
            try:
                actual = hashlib.sha256(git("show", f"HEAD:{base}/{name.strip()}")).hexdigest()
            except subprocess.CalledProcessError:
                actual = None
            if actual != digest:
                bad.append(name.strip())
        out.append({"run": base.split("/")[-1], "listed": total, "mismatched_or_missing": bad})
    return out


def main() -> int:
    files = git("ls-files", "-z").decode().split("\0")[:-1]
    head = git("rev-parse", "HEAD").decode().strip()
    rows, manifest = [], {}
    for path in files:
        blob = git("show", f"HEAD:{path}") if path in git("ls-tree", "-r", "--name-only", "HEAD", "--", path).decode() else (REPO / path).read_bytes()
        digest = hashlib.sha256(blob).hexdigest()
        if path.startswith("validation/results/"):
            manifest[path] = {"sha256": digest, "bytes": len(blob)}
            continue
        text = blob.decode("utf-8", errors="replace")
        rows.append((path, kind(path), len(blob), digest[:12], text.count("\n"), tests_in(path, text),
                     local_deps(path, text), purpose(path)))
    missing = [r[0] for r in rows if not r[7]]
    assert not missing, f"inventory has no purpose for: {missing}"
    checks = verify_checksums(files)
    OUT.mkdir(parents=True, exist_ok=True)
    runs = sorted({p.split("/")[2] for p in manifest})
    (OUT / "baseline_manifest.json").write_text(json.dumps(
        {"commit": head, "hash": "sha256 of the blob at HEAD", "result_files": manifest,
         "archived_checksum_verification": checks, "runs": runs}, indent=1) + "\n", encoding="utf-8")
    lines = [f"# File audit (tracked files at `{head[:12]}`)", "",
             "Generated by `validation/harness/file_inventory.py`; hashes are SHA-256 of the",
             "blob at HEAD (first 12 hex digits). Tests = `#[test]` functions (Rust) or",
             "`def test_` + module self-check `assert`s (Python). Archived results are",
             "listed per run below and hashed in `baseline_manifest.json`.", "",
             "| Path | Kind | Bytes | SHA-256 | Lines | Tests | Local deps | Purpose / review note |",
             "|---|---|---:|---|---:|---:|---|---|"]
    lines += [f"| `{p}` | {k} | {b} | `{h}` | {n} | {t} | {d} | {u} |" for p, k, b, h, n, t, d, u in rows]
    lines += ["", "## Archived evidence (validation/results)", "",
              "| Run | Files | Bytes | Archived checksums |", "|---|---:|---:|---|"]
    for run in runs:
        own = [v for p, v in manifest.items() if p.split("/")[2] == run]
        check = next((c for c in checks if c["run"] == run), None)
        status = "none recorded" if not check else (
            f"{check['listed']} listed, all match" if not check["mismatched_or_missing"]
            else f"{check['listed']} listed, MISMATCH: {', '.join(check['mismatched_or_missing'])}")
        lines.append(f"| `{run}` | {len(own)} | {sum(v['bytes'] for v in own)} | {status} |")
    (OUT / "FILE_AUDIT.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(json.dumps({"files": len(files), "active": len(rows), "result_files": len(manifest),
                      "checksum_runs": [(c["run"], c["listed"], len(c["mismatched_or_missing"])) for c in checks]}))
    return 0


assert kind("validation/results/x/a.json") == "archived evidence" and kind("crates/a/src/lib.rs") == "active code"
assert tests_in("x.rs", "#[test]\nfn a(){}\n#[test]") == 2

if __name__ == "__main__":
    raise SystemExit(main())
