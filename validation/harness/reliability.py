"""Windows reliability checks not covered by security.py.

1. branch-switch: a running `findex mcp` (native watcher) serves a ripgrep
   worktree; `git checkout` swaps it between two releases (hundreds of files
   change at once). Afterwards, `findex_context` must find a function that
   exists only in the new checkout and must not cite one that exists only in
   the old one. Symbols are chosen mechanically from `git grep` at each
   revision. Repeated both ways.
2. onedrive: index the Findex repository where it lives (a OneDrive-synced
   folder) read-only and compare the indexed file count with the admission
   rule; cited files must be hash-verified (no stale results).
Usage: reliability.py  -> reliability.json
"""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import time
from pathlib import Path

from common import REPO, WORK, CommandLog, findex_binary, isolated_env, results_dir, write_json
from golden import admitted_files
from issues import HISTORY
from mcp_client import McpProcess

OLD, NEW = "13.0.0", "14.1.1"


def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, encoding="utf-8",
                          errors="replace", check=True).stdout


def functions(repo: Path, rev: str) -> set[str]:
    out = git(repo, "grep", "-h", "-o", "-E", r"\bfn [a-z][a-z0-9_]{7,}\(", rev, "--", "*.rs")
    return {line[3:-1] for line in out.splitlines()}


def cites(mcp: McpProcess, name: str) -> tuple[bool, float]:
    _, text, seconds, error = mcp.call_tool("findex_context", {"task": f"where is `{name}` defined"})
    assert not error, text
    return bool(re.search(rf"^definition \S+ .*\b{name}\b", text, re.M)), seconds


def wait_for(mcp: McpProcess, present: str, absent: str, limit: float = 120.0) -> dict:
    started = time.perf_counter()
    while True:
        found, _ = cites(mcp, present)
        stale, _ = cites(mcp, absent)
        elapsed = time.perf_counter() - started
        if (found and not stale) or elapsed > limit:
            return {"present_found": found, "absent_cited": stale, "seconds_to_correct": round(elapsed, 2),
                    "pass": found and not stale}
        time.sleep(1.0)


def branch_switch(log: CommandLog) -> dict:
    history = HISTORY / "ripgrep"
    tree = WORK / "reliability" / "ripgrep"
    if tree.exists():
        subprocess.run(["git", "-C", str(history), "worktree", "remove", "--force", str(tree)], capture_output=True)
        shutil.rmtree(tree, ignore_errors=True)
    subprocess.run(["git", "-C", str(history), "worktree", "prune"], capture_output=True)
    git(history, "worktree", "add", "-q", "--detach", str(tree), NEW)
    changed = len(git(tree, "diff", "--name-only", OLD, NEW).splitlines())
    old_fns, new_fns = functions(tree, OLD), functions(tree, NEW)
    only_new, only_old = sorted(new_fns - old_fns)[0], sorted(old_fns - new_fns)[0]
    state = WORK / "reliability" / "state"
    shutil.rmtree(state, ignore_errors=True)
    mcp = McpProcess(findex_binary("current"), tree, isolated_env(state))
    try:
        initial = wait_for(mcp, only_new, only_old)
        steps = []
        for rev, present, absent in ((OLD, only_old, only_new), (NEW, only_new, only_old)):
            git(tree, "checkout", "-q", "--detach", rev)
            steps.append({"checkout": rev, **wait_for(mcp, present, absent)})
    finally:
        mcp.proc.kill()
    return {"status": "EXECUTED", "corpus": "ripgrep", "revisions": [NEW, OLD], "files_changed": changed,
            "only_new_function": only_new, "only_old_function": only_old, "initial": initial, "switches": steps,
            "pass": initial["pass"] and all(s["pass"] for s in steps)}


def onedrive(log: CommandLog) -> dict:
    root = REPO
    if "onedrive" not in str(root).lower():
        return {"status": "SKIPPED", "reason": f"repository is not in a OneDrive folder: {root}"}
    exe = findex_binary("current")
    state = WORK / "reliability" / "onedrive-state"
    shutil.rmtree(state, ignore_errors=True)
    proc = log.run([str(exe), "index"], cwd=root, env=isolated_env(state), timeout=600)
    report = json.loads(log.run([str(exe), "context", "--json", "callers of `storage_dir`"], cwd=root,
                                env=isolated_env(state), timeout=120).stdout)
    expected = len(admitted_files(root))
    examined = re.search(r"examined (\d+)", proc.stdout + proc.stderr)
    return {"status": "EXECUTED", "root_kind": "OneDrive-synced folder (files present locally; cloud-only "
            "placeholders not tested: creating them would change the user's OneDrive)",
            "index_exit_code": proc.returncode, "index_output": (proc.stdout + proc.stderr).strip()[-300:],
            "tracked_admitted_files": expected, "examined_reported": int(examined.group(1)) if examined else None,
            "note": "examined also counts untracked, non-ignored files; tracked_admitted_files counts tracked only",
            "context_evidence": len(report["evidence"]), "stale_results_discarded": report["stale_results_discarded"],
            "pass": proc.returncode == 0 and len(report["evidence"]) > 0 and report["stale_results_discarded"] == 0}


def main() -> int:
    out = results_dir(None)
    log = CommandLog(out)
    result = {"branch_switch": branch_switch(log), "onedrive": onedrive(log)}
    write_json(out / "reliability.json", result)
    print(json.dumps({k: v.get("pass", v.get("status")) for k, v in result.items()}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
