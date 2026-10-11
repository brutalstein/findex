"""Static checks, the full Rust test suite and a release build; writes tests.json.

Each step is PASS / FAIL / SKIPPED (tool missing). Per-test outcomes are
parsed from libtest output so a failing test is named, not just counted.
Usage: unit.py [--out DIR] [--skip-clippy]
"""
from __future__ import annotations

import argparse
import re
import shutil
import sys

from common import REPO, CommandLog, results_dir, write_json

TEST_LINE = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)", re.M)
RESULT_LINE = re.compile(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=None)
    parser.add_argument("--skip-clippy", action="store_true")
    args = parser.parse_args()
    out = results_dir(args.out)
    log = CommandLog(out)
    steps = [("fmt", ["cargo", "fmt", "--all", "--", "--check"]),
             ("clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]),
             ("test", ["cargo", "test", "--workspace", "--locked", "--no-fail-fast"]),
             ("release_build", ["cargo", "build", "--release", "--locked", "-p", "findex"]),
             ("audit", ["cargo", "audit", "--deny", "warnings"])]
    report = {"steps": {}, "tests": {}}
    for name, argv in steps:
        if name == "clippy" and args.skip_clippy:
            report["steps"][name] = {"status": "SKIPPED", "reason": "--skip-clippy"}
            continue
        if name == "audit" and not shutil.which("cargo-audit"):
            report["steps"][name] = {"status": "SKIPPED", "reason": "cargo-audit not installed"}
            continue
        print(f"[unit] {name}", file=sys.stderr)
        proc = log.run(argv, cwd=REPO, timeout=3600)
        step = {"status": "PASS" if proc.returncode == 0 else "FAIL", "exit_code": proc.returncode}
        if name == "test":
            text = proc.stdout + proc.stderr
            outcomes = TEST_LINE.findall(text)
            report["tests"] = {test: outcome for test, outcome in outcomes}
            totals = [tuple(map(int, m[1:])) for m in RESULT_LINE.findall(text)]
            step.update(passed=sum(t[0] for t in totals), failed=sum(t[1] for t in totals),
                        ignored=sum(t[2] for t in totals), suites=len(totals),
                        failed_tests=[t for t, o in outcomes if o == "FAILED"])
        if proc.returncode != 0:
            step["tail"] = (proc.stdout + proc.stderr)[-1500:]
        report["steps"][name] = step
    write_json(out / "tests.json", report)
    test = report["steps"].get("test", {})
    print(f"tests passed={test.get('passed')} failed={test.get('failed')} ignored={test.get('ignored')}")
    return 0 if all(s["status"] != "FAIL" for s in report["steps"].values()) else 1


if __name__ == "__main__":
    raise SystemExit(main())
