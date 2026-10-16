"""Verify the `coverage` claim of `findex context` against `git grep -w`.

For every distinct name in the frozen golden set, `findex context` is asked
"callers of `name`". When the report is not truncated and carries a coverage
line, it claims to show every code occurrence of the name in the indexed
files, omitting only the comment lines it counts. Truth: `git grep -n -w`
over the files Findex admits (golden.admitted_files). A grep line is covered
when Findex lists it or it lies inside a listed definition span. The claim
holds for a name when every uncovered line is a comment by this checker's own
whole-file lexer (Python: the stdlib `tokenize`; other languages: a scanner
written independently of Findex's per-line rule), their number equals the
omitted-comment count, and only reported method-syntax lines are otherwise
missing.
Usage: coverage_check.py [--corpora ripgrep,tokio,...]  -> coverage_check.json
"""
from __future__ import annotations

import argparse
import io
import json
import re
import shutil
import subprocess
import sys
import tokenize

from common import CORPORA, VALIDATION, WORK, CommandLog, findex_binary, isolated_env, results_dir, write_json
from golden import admitted_files

NAME = re.compile(r"^[A-Za-z_][A-Za-z0-9_]{2,}$")
COMMENTS = re.compile(r"\((\d+) comment lines? omitted\)")
EXCLUDED = re.compile(r"excluded (\d+) method-syntax")
HASH = {"py", "pyi", "rb", "sh", "bash", "ps1", "psm1", "toml", "yaml", "yml", "cmake", "pl", "r"}
CLIKE = {"c", "cc", "cjs", "cpp", "cs", "cxx", "go", "h", "hh", "hpp", "hxx", "java", "js", "jsx", "kt", "lua", "m",
         "mjs", "mm", "php", "rs", "scala", "swift", "ts", "tsx"}


def comment_spans(path: str, text: str) -> dict[int, list[tuple[int, int]]]:
    """1-based line -> [start, end) character ranges that are comments."""
    ext = path.rsplit(".", 1)[-1].lower() if "." in path else ""
    spans: dict[int, list[tuple[int, int]]] = {}
    if ext in ("py", "pyi"):
        try:
            for tok in tokenize.generate_tokens(io.StringIO(text).readline):
                if tok.type == tokenize.COMMENT:
                    spans.setdefault(tok.start[0], []).append((tok.start[1], tok.end[1]))
            return spans
        except (tokenize.TokenError, SyntaxError):
            spans = {}  # unparsable Python: fall back to the generic hash scanner
    if ext not in HASH and ext not in CLIKE:
        return spans
    clike = ext in CLIKE
    quotes = ('"', "`") if clike else ('"', "'")
    line, col, i, state, start = 1, 0, 0, None, 0
    while i < len(text):
        c, nxt = text[i], text[i + 1] if i + 1 < len(text) else ""
        if state == "block" and c == "*" and nxt == "/":
            spans.setdefault(line, []).append((start, col + 2))
            state, i, col = None, i + 2, col + 2
            continue
        if state == "line" and c == "\n":
            spans.setdefault(line, []).append((start, col))
            state = None
        elif state in quotes:
            if c == "\\":
                i, col = i + 2, col + 2
                continue
            if c == state or c == "\n":
                state = None
        elif state is None:
            if (clike and c == "/" and nxt == "/") or (not clike and c == "#"):
                state, start = "line", col
            elif clike and c == "/" and nxt == "*":
                state, start, i, col = "block", col, i + 2, col + 2
                continue
            elif c in quotes:
                state = c
        if c == "\n":
            if state == "block":
                spans.setdefault(line, []).append((start, col + 1))
                start = 0
            line, col = line + 1, 0
        else:
            col += 1
        i += 1
    if state in ("line", "block"):
        spans.setdefault(line, []).append((start, col + 1))
    return spans


def is_comment(text: str, name: str, spans: list[tuple[int, int]]) -> bool:
    """Every whole-word occurrence of `name` on the line lies in a comment."""
    hits = [m.start() for m in re.finditer(rf"(?<![A-Za-z0-9_]){re.escape(name)}(?![A-Za-z0-9_])", text)]
    return bool(hits) and all(any(a <= h < b for a, b in spans) for h in hits)


_C = '#define F f(x)\nint *o = f(1); // f\n/* f\n f */ g("/*");\nh(); // f\n'
assert [is_comment(line, "f", comment_spans("a.c", _C).get(n, [])) for n, line in enumerate(_C.splitlines(), 1)] \
    == [False, False, True, True, True], comment_spans("a.c", _C)
_P = "# f\nx = f()  # f\ns = '# f'\n"
assert [is_comment(line, "f", comment_spans("a.py", _P).get(n, [])) for n, line in enumerate(_P.splitlines(), 1)] \
    == [True, False, False]


def grep_lines(root, name: str, admitted: set[str]) -> dict[tuple[str, int], tuple[str, bool]]:
    """(path, line) -> (text, is a comment by this checker's lexer)."""
    out = subprocess.run(["git", "-C", str(root), "grep", "-n", "-w", "-I", "-F", "--no-color", name],
                         capture_output=True, text=True, encoding="utf-8", errors="replace").stdout
    lines, cache = {}, {}
    for row in out.splitlines():
        path, line, text = row.split(":", 2)
        if path in admitted:
            if path not in cache:
                source = (root / path).read_text(encoding="utf-8", errors="replace")
                cache[path] = comment_spans(path, source)
            lines[(path, int(line))] = (text, is_comment(text, name, cache[path].get(int(line), [])))
    return lines


def check(report: dict, truth: dict[tuple[str, int], tuple[str, bool]], name: str) -> dict | None:
    """None when the report makes no exhaustiveness claim."""
    if report["truncated"] or not report.get("coverage"):
        return None
    shown = {(e["path"], e["line"]) for e in report["evidence"] if e["kind"] != "definition"}
    spans = [(e["path"], e["line"], e["end_line"]) for e in report["evidence"] if e["kind"] == "definition"]
    # A definition covers its own code lines; comments inside it are omissions.
    uncovered = {key for key in set(truth) - shown
                 if truth[key][1] or not any(key[0] == q and a <= key[1] <= b for q, a, b in spans)}
    comments = {key for key in uncovered if truth[key][1]}
    code = uncovered - comments
    claimed_comments = sum(int(n) for c in report["coverage"] for n in COMMENTS.findall(c))
    excluded = sum(int(n) for m in report["missing"] for n in EXCLUDED.findall(m))
    extra = shown - set(truth)
    return {"truth_lines": len(truth), "omitted_comments": len(comments), "claimed_comments": claimed_comments,
            "omitted_code": len(code), "reported_method_exclusions": excluded, "extra_lines": len(extra),
            "holds": len(comments) == claimed_comments and len(code) == excluded and not extra,
            "examples": sorted(code)[:3] + sorted(extra)[:3]}


assert check({"truncated": False, "coverage": ["coverage `f`: ... (1 comment line omitted)"], "missing": [],
              "evidence": [{"kind": "call", "path": "a.rs", "line": 2, "end_line": 2},
                           {"kind": "definition", "path": "a.rs", "line": 5, "end_line": 7}]},
             {("a.rs", 2): ("f()", False), ("a.rs", 6): ("fn f() {", False), ("b.rs", 1): ("// f", True)},
             "f")["holds"]
# A code line counted as an omitted comment is a violation even when the counts match.
assert not check({"truncated": False, "coverage": ["coverage `f`: ... (1 comment line omitted)"], "missing": [],
                  "evidence": []}, {("a.rs", 1): ("#[x(f)]", False)}, "f")["holds"]
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
    result = {"status": "EXECUTED", "truth": "git grep -n -w -I -F over golden.admitted_files; comments by "
              "tokenize (Python) or an independent whole-file scanner", "corpora": {}}
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
