"""Build the frozen issue-localization dataset from real fixed issues.

Independent labels: the query is the reporter's issue title (fetched from
GitHub), the gold answer is the set of non-test source files changed by the
commit that closed the issue, and the search runs on that commit's parent.
Neither the labeler nor Findex wrote any of them. Selection is mechanical and
fixed before any system is evaluated; the dataset is never tuned against.

Usage: issues.py [--per-repo 25]  (needs network and an authenticated `gh`)
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
import subprocess
import sys

from common import CORPUS_PINS, VALIDATION, WORK

REPOS = {  # corpus -> GitHub owner/name
    "requests": "psf/requests",
    "ripgrep": "BurntSushi/ripgrep",
    "zod": "colinhacks/zod",
    "fmt": "fmtlib/fmt",
}
SOURCE = re.compile(r"\.(py|rs|ts|tsx|js|h|hpp|cc|cpp|c)$")
TESTS = re.compile(r"(^|/)(tests?|testing|__tests__|spec|specs|benches|examples?|docs?)/|(^|/)test_|_test\.|\.test\.|\.spec\.")
CLOSES = re.compile(r"(?i)\b(?:fix(?:es|ed)?|close[sd]?|resolve[sd]?)\s+#(\d+)")
HISTORY = WORK / "history"


def git(repo, *args) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, encoding="utf-8",
                          errors="replace", check=True).stdout


def history(corpus: str):
    """Full commit history without blobs: enough for logs and diffs on demand."""
    url, tag, _ = CORPUS_PINS[corpus]
    root = HISTORY / corpus
    if not root.exists():
        HISTORY.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "clone", "-q", "--filter=blob:none", "--no-checkout", url, str(root)], check=True)
    return root, tag


def issue_title(slug: str, number: str) -> str | None:
    proc = subprocess.run(["gh", "api", f"repos/{slug}/issues/{number}"], capture_output=True, text=True,
                          encoding="utf-8")
    if proc.returncode != 0:
        return None
    data = json.loads(proc.stdout)
    return None if "pull_request" in data else data.get("title")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--per-repo", type=int, default=25)
    parser.add_argument("--out", type=Path, default=VALIDATION / "golden" / "issue_localization.jsonl")
    parser.add_argument("--exclude", type=Path,
                        help="tasks already used; their issues are skipped, so the mined set is fresh and disjoint")
    parser.add_argument("--include-merges", action="store_true",
                        help="also accept merge commits (the PR's changes against its first parent)")
    args = parser.parse_args()
    skip ={json.loads(line)["id"] for line in args.exclude.read_text(encoding="utf-8").splitlines() if line} if args.exclude else set()
    rows = []
    for corpus, slug in REPOS.items():
        repo, tag = history(corpus)
        log = git(repo, "log", "--first-parent", "--format=%H%x00%P%x00%s%x00%b%x1e", tag)
        taken = 0
        for record in log.split("\x1e"):
            parts = record.strip("\n").split("\x00")
            if len(parts) < 4 or taken >= args.per_repo:
                continue
            commit, parents, subject, body = parts
            match = CLOSES.search(subject + "\n" + body)
            if not match or (len(parents.split()) != 1 and not args.include_merges) or f"{corpus}-{match.group(1)}" in skip:
                continue
            changed = [p for p in git(repo, "diff", "--name-only", f"{commit}^", commit).splitlines()
                       if SOURCE.search(p) and not TESTS.search(p)]
            if not 1 <= len(changed) <= 3:
                continue
            existing = set(git(repo, "ls-tree", "-r", "--name-only", f"{commit}^").splitlines())
            gold = [p for p in changed if p in existing]  # files the fix created cannot be localized
            if not gold:
                continue
            title = issue_title(slug, match.group(1))
            if not title:
                continue
            rows.append({"id": f"{corpus}-{match.group(1)}", "corpus": corpus, "repo": slug, "issue": int(match.group(1)),
                         "query": title, "fix_commit": commit, "base_commit": parents.split()[0], "gold_files": gold})
            taken += 1
            print(f"[issues] {corpus} #{match.group(1)} {title[:70]}", file=sys.stderr)
    out = args.out
    out.write_text("".join(json.dumps(r) + "\n" for r in rows), encoding="utf-8", newline="\n")
    print(f"{len(rows)} tasks -> {out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
