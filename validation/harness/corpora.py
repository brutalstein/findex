"""Fetch the pinned benchmark corpora and build the frozen baseline binary.

Idempotent: existing corpora are verified against the commits recorded in
the golden dataset instead of being re-fetched. Needs network only for
missing corpora. Usage: corpora.py [--verify-only]
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys

from common import CORPORA, CORPUS_PINS, VALIDATION, WORK, findex_binary, git


def expected_commits() -> dict:
    commits = {}
    for line in (VALIDATION / "golden" / "symbol_navigation.jsonl").open(encoding="utf-8"):
        row = json.loads(line)
        commits.setdefault(row["corpus"], row["commit"])
    return commits


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()
    expected, problems = expected_commits(), 0
    for name, (url, ref, _) in CORPUS_PINS.items():
        root = CORPORA / name
        if not root.exists() and not args.verify_only:
            CORPORA.mkdir(parents=True, exist_ok=True)
            if name == "findex":  # a commit, not a tag: full clone then detach
                subprocess.run(["git", "clone", "-q", url, str(root)], check=True)
                subprocess.run(["git", "-C", str(root), "checkout", "-q", "--detach", ref], check=True)
            else:
                subprocess.run(["git", "clone", "-q", "--depth", "1", "--branch", ref, url, str(root)], check=True)
        head = git("rev-parse", "HEAD", cwd=root) if root.exists() else ""
        want = expected.get(name)
        ok = bool(head) and (want is None or head == want) and (name != "findex" or head.startswith(ref))
        problems += not ok
        print(f"{'OK ' if ok else 'BAD'} {name:9} {head[:12] or 'missing'} (pin {ref}, golden {str(want)[:12]})")
    baseline = findex_binary("baseline")
    if not baseline.exists() and not args.verify_only:
        env = dict(os.environ, CARGO_TARGET_DIR=str(WORK / "build" / "baseline-target"))
        subprocess.run(["cargo", "build", "--release", "--locked", "-p", "findex"], cwd=CORPORA / "findex", env=env, check=True)
        baseline.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(WORK / "build" / "baseline-target" / "release" / "findex.exe", baseline)
    print(f"{'OK ' if baseline.exists() else 'BAD'} baseline binary {baseline.name}")
    problems += not baseline.exists()
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
