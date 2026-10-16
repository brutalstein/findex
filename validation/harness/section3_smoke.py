"""Offline end-to-end context routing and evidence selection smoke checks.

Requires only the built Findex executable. No model clients, credentials, or
network access. The Rust unit tests hold the detailed behavioral oracles.
"""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path


def context(executable: Path, project: Path, task: str, budget: int = 2000) -> dict:
    result = subprocess.run(
        [str(executable), "context", "--json", "--budget", str(budget), task],
        cwd=project, capture_output=True, text=True, timeout=45, check=True,
    )
    return json.loads(result.stdout)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: python section3_smoke.py PATH_TO_FINDEX_EXE")
    executable = Path(sys.argv[1]).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="findex-section3-") as tmp:
        root = Path(tmp)
        (root / "src").mkdir()
        (root / "tests").mkdir()
        (root / "src" / "lib.rs").write_text(
            "pub fn audited_work() -> u32 { 42 }\n"
            "pub fn other() -> u32 { audited_work() }\n", encoding="utf-8",
        )
        (root / "tests" / "util.rs").write_text(
            "#[test]\nfn verifies_audited_work() {\n"
            "    assert_eq!(audited_work(), 42);\n}\n", encoding="utf-8",
        )
        # The filename is in a tests/ directory, but the user wants a file.
        named = context(executable, root, "show tests/util.rs")
        assert named["intent"] == "file", named
        assert any(item["path"] == "tests/util.rs" for item in named["evidence"]), named

        # For test navigation, actual test evidence must precede definitions.
        tested = context(executable, root, "which tests cover `audited_work`")
        assert tested["intent"] == "tests", tested
        assert tested["evidence"] and tested["evidence"][0]["in_test"], tested

        # A name with no supporting source must never receive an invented cite.
        missing = context(executable, root, "absentmeaninglessjargonwordxx")
        assert not missing["evidence"], missing
        assert any(("no indexed text matches" in text or "no hash-verified" in text)
                   for text in missing["missing"]), missing

        # File deletion impact must not be mislabeled as a file-open operation.
        impact = context(executable, root, "delete src/lib.rs: what breaks")
        assert impact["intent"] == "impact", impact
        assert any("not resolved" in x for x in impact["missing"]), impact

        # The result's *rendered text* (not JSON metadata) has a fixed budget.
        result = subprocess.run(
            [str(executable), "context", "--budget", "200",
             "who calls `audited_work`"],
            cwd=root, capture_output=True, text=True, timeout=45, check=True,
        )
        assert (len(result.stdout.encode("utf-8")) + 3) // 4 <= 200, result.stdout

    print("section3_smoke: PASS (5 end-to-end offline assertions; 0 model calls)")


if __name__ == "__main__":
    main()
