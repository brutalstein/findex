"""Verify the `coverage` claim of `findex context` against `git grep -w`.

For every distinct name in the frozen golden set, `findex context` is asked
"callers of `name`". When the report is not truncated and carries a coverage
line, it claims to show every code occurrence of the name in the indexed
files, omitting only the comment lines it counts. Truth: `git grep -n -w`
over the files Findex admits (golden.admitted_files). A grep line is covered
when Findex lists it or it lies inside a listed definition span. The claim
holds for a name when the uncovered lines are exactly the omitted comments
(plus any method-syntax lines it reports excluding).
Usage: coverage_check.py [--corpora ripgrep,tokio,...]  -> coverage_check.json
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys

from common import CORPORA, VALIDATION, WORK, CommandLog, findex_binary, isolated_env, results_dir, write_json
from golden import admitted_files

NAME = re.compile(r"^[A-Za-z_][A-Za-z0-9_]{2,}$")
COMMENTS = re.compile(r"\((\d+) comment lines? omitted\)")
EXCLUDED = re.compile(r"excluded (\d+) method-syntax")


def is_comment(text: str, name: str) -> bool:
    stripped = text.lstrip()
    return stripped.startswith(("//", "#", "/*", "*")) or "//" in text.split(name, 1)[0]


def grep_lines(root, name: str, admitted: set[str]) -> dict[tuple[str, int], str]:
    out = subprocess.run(["git", "-C", str(root), "grep", "-n", "-w", "-I", "-F", "--no-color", name],
                         capture_output=True, text=True, encoding="utf-8", errors="replace").stdout
    lines = {}
    for row in out.splitlines():
        path, line, text = row.split(":", 2)
        if path in admitted:
            lines[(path, int(line))] = text
    return lines


def check(report: dict, truth: dict[tuple[str, int], str], name: str) -> dict | None:
    """None when the report makes no exhaustiveness claim."""
    if report["truncated"] or not report.get("coverage"):
        return None
    shown = {(e["path"], e["line"]) for e in report["evidence"] if e["kind"] != "definition"}
    spans = [(e["path"], e["line"], e["end_line"]) for e in report["evidence"] if e["kind"] == "definition"]
    # A definition covers its own (non-comment) lines; comments inside it are omissions.
    uncovered = {(p, l) for p, l in set(truth) - shown
                 if is_comment(truth[(p, l)], name) or not any(p == q and a <= l <= b for q, a, b in spans)}
    allowed = sum(int(n) for c in report["coverage"] for n in COMMENTS.findall(c))
    allowed += sum(int(n) for m in report["missing"] for n in EXCLUDED.findall(m))
    extra = shown - set(truth)
    return {"truth_lines": len(truth), "uncovered": len(uncovered), "allowed_omissions": allowed,
            "extra_lines": len(extra), "holds": len(uncovered) == allowed and not extra,
            "examples": sorted(uncovered)[:3] + sorted(extra)[:3]}


assert check({"truncated": False, "coverage": ["coverage `f`: ... (1 comment line omitted)"], "missing": [],
              "evidence": [{"kind": "call", "path": "a.rs", "line": 2, "end_line": 2},
                           {"kind": "definition", "path": "a.rs", "line": 5, "end_line": 7}]},
             {("a.rs", 2): "f()", ("a.rs", 6): "fn f() {", ("b.rs", 1): "// f"}, "f")["holds"]
assert check({"truncated": True, "coverage": ["x"], "missing": [], "evidence": []}, {}, "f") is None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpora", default="")
    parser.add_argument("--out", default=None)
    args = parser.parse_args()
    out = results_dir(args.out)
    log = CommandLog(out)
    rows = [json.loads(line) for line in (VALIDATION / "golden" / "symbol_navigation.jsonl").open(encoding="utf-8")]
    corpora = args.corpora.split(",") if args.corpora else sorted({r["corpus"] for r in rows})
    exe = findex_binary("current")
    result = {"status": "EXECUTED", "truth": "git grep -n -w -I -F over golden.admitted_files", "corpora": {}}
    for corpus in corpora:
        root = CORPORA / corpus
        names = sorted({r["name"] for r in rows if r["corpus"] == corpus and NAME.match(r["name"] or "")})
        admitted = set(admitted_files(root))
        state = WORK / "coverage-state" / corpus
        shutil.rmtree(state, ignore_errors=True)
        log.run([str(exe), "index"], cwd=root, env=isolated_env(state), timeout=1800)
        checked = []
        for name in names:
            proc = log.run([str(exe), "context", "--json", "--budget", "8000", f"callers of `{name}`"],
                           cwd=root, env=isolated_env(state), timeout=300)
            verdict = check(json.loads(proc.stdout), grep_lines(root, name, admitted), name)
            checked.append({"name": name, **(verdict or {"claim": False})})
            if verdict and not verdict["holds"]:
                print(f"[coverage] {corpus} {name}: VIOLATION {verdict}", file=sys.stderr)
        claims = [c for c in checked if "holds" in c]
        result["corpora"][corpus] = {"names": len(checked), "claims": len(claims),
                                     "claims_hold": sum(c["holds"] for c in claims),
                                     "violations": [c for c in claims if not c["holds"]], "rows": checked}
        print(f"[coverage] {corpus}: {sum(c['holds'] for c in claims)}/{len(claims)} claims hold "
              f"({len(checked) - len(claims)} names without a claim)", file=sys.stderr)
    write_json(out / "coverage_check.json", result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
