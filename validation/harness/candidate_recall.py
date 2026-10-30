"""Decompose a concept-ranking miss into its cause, on the dev half only.

For each task the repository is indexed at the fix's parent commit. The
concept query is rebuilt as `context.rs` builds it (stopword lists parsed from
the Rust source, not copied; words are lowercased before the 12-term cap), and
the raw FTS5 BM25 order is read from the index. Per task:
  raw_rank    gold position in the raw BM25 order over every match
  in_window   raw_rank <= WINDOW (the 128 candidates the reranker sees)
  final_rank  gold position in `findex context --json` (ranked + evidence)
A miss is then one of: never-matched (vocabulary gap), outside-window
(candidate recall) or in-window-misordered (reranker).
Usage: candidate_recall.py [--dataset PATH] [--out DIR] [--split dev|test]
Development must use --split dev; the test half is aggregate-only.
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import sqlite3
import subprocess
import sys
from pathlib import Path

from common import REPO, VALIDATION, WORK, CommandLog, findex_binary, isolated_env, results_dir, write_json
from issues import HISTORY
from localization import freeze_binary, split

WINDOW = 128
CONTEXT_RS = REPO / "crates" / "findex-index" / "src" / "context.rs"


def rust_list(text: str, name: str) -> set[str]:
    body = re.search(rf"const {name}: &\[&str\] = &\[(.*?)\];", text, re.S)
    return set(re.findall(r'"([^"]*)"', body.group(1))) if body else set()


def query_terms(query: str, stop: set[str]) -> list[str]:
    words = [w for w in re.split(r"[^\w]+", query.lower()) if w]
    content = [p for w in words for p in w.split("_") if len(p) > 1 and p not in stop]
    return [t for t in re.split(r"[^\w]+", " ".join(content)) if t][:12]


def camel_parts(token: str) -> list[str]:
    parts, start = [], 0
    for i in range(1, len(token)):
        prev, nxt = token[i - 1], token[i + 1] if i + 1 < len(token) else ""
        if token[i].isupper() and (prev.islower() or prev.isdigit() or (prev.isupper() and nxt.islower())):
            parts.append(token[start:i].lower())
            start = i
    parts.append(token[start:].lower())
    return parts


def identifier_words(text: str) -> set[str]:
    """The path/name half of `identifier_terms` in lib.rs: each token and its camelCase parts."""
    words: set[str] = set()
    for token in re.split(r"[^\w]+|_", text):
        if token:
            words.add(token.lower())
            words.update(camel_parts(token))
    return words


def proximity(body: str, terms: list[str]) -> tuple[int, int]:
    """Most distinct terms on one line, and within a five-line window: the snippet logic in lib.rs."""
    masks = []
    for line in body.lower().splitlines():
        masks.append(sum(1 << i for i, t in enumerate(terms) if t in line))
    line_best = max((m.bit_count() for m in masks), default=0)
    window = 0
    for centre in range(len(masks)):
        merged = 0
        for m in masks[max(centre - 2, 0):centre + 3]:
            merged |= m
        window = max(window, merged.bit_count())
    return line_best, window


def candidate_features(conn: sqlite3.Connection, terms: list[str], expression: str) -> dict:
    """Every input of the concept score for the first WINDOW matches, so a ranking can be replayed offline.
    `matched` lists the query terms an FTS5 token match finds in each file; `df` is each term's document frequency."""
    rows = conn.execute(
        "SELECT files.path, docs.rank, length(CAST(docs.body AS BLOB)), docs.body FROM docs JOIN files ON files.id = docs.rowid "
        "WHERE docs MATCH ? ORDER BY docs.rank ASC, files.path ASC LIMIT ?", (expression, WINDOW)).fetchall()
    holders = {t: {p for (p,) in conn.execute(
        "SELECT files.path FROM docs JOIN files ON files.id = docs.rowid WHERE docs MATCH ?", ('"' + t.replace('"', '""') + '"',))}
        for t in dict.fromkeys(terms)}
    out = []
    for path, bm25, size, body in rows:
        line_best, window = proximity(body, terms)
        stem = path.rsplit("/", 1)[-1].rsplit(".", 1)[0]
        filename = sum(1 for w in identifier_words(stem) if len(w) >= 3 and w in terms)
        symbols: set[str] = set()
        for (name,) in conn.execute("SELECT name FROM symbols WHERE path = ?", (path,)):
            symbols |= identifier_words(name)
        out.append({"path": path, "bm25": bm25, "size": size, "filename": filename,
                    "line_terms": line_best, "window_terms": window,
                    "symbol_ratio": sum(1 for t in terms if t in symbols) / max(len(terms), 1),
                    "matched": [t for t in dict.fromkeys(terms) if path in holders[t]]})
    return {"files": conn.execute("SELECT count(*) FROM files").fetchone()[0],
            "df": {t: len(h) for t, h in holders.items()}, "rows": out}


def cause(row: dict) -> str:
    if row["raw_rank"] is None:
        return "never-matched"
    if not row["in_window"]:
        return "outside-window"
    rank = row["final_rank"]
    return "ok@1" if rank == 1 else "ok@10" if rank and rank <= 10 else "in-window-misordered"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dataset", type=Path, default=VALIDATION / "golden" / "issue_localization.jsonl")
    parser.add_argument("--out", default=None)
    parser.add_argument("--split", choices=("dev", "test"), default="dev")
    args = parser.parse_args()
    out = results_dir(args.out)
    text = CONTEXT_RS.read_text(encoding="utf-8")
    stop = rust_list(text, "STOPWORDS") | rust_list(text, "CALLER_WORDS") | rust_list(text, "TEST_WORDS")
    exe, digest = freeze_binary(findex_binary("current"))
    write_json(out / "manifest.json", {"binary_sha256": digest, "dataset": args.dataset.name, "split": args.split})
    log = CommandLog(out)
    scratch = (WORK / "candidate-recall" / out.name).resolve()
    rows = []
    for line in args.dataset.open(encoding="utf-8"):
        task = json.loads(line)
        if split(task) != args.split:
            continue
        tree, state = scratch / "tree", scratch / "state"
        history = HISTORY / task["corpus"]
        if tree.exists():
            subprocess.run(["git", "-C", str(history), "worktree", "remove", "--force", str(tree)], capture_output=True)
            shutil.rmtree(tree, ignore_errors=True)
        subprocess.run(["git", "-C", str(history), "worktree", "prune"], capture_output=True)
        subprocess.run(["git", "-C", str(history), "worktree", "add", "-q", "--detach", str(tree), task["base_commit"]],
                       check=True, capture_output=True)
        shutil.rmtree(state, ignore_errors=True)
        if log.run([str(exe), "index"], cwd=tree, env=isolated_env(state), timeout=900).returncode:
            raise RuntimeError(f"index failed for {task['id']}")
        proc = log.run([str(exe), "context", "--json", "--budget", "8000", task["query"]],
                       cwd=tree, env=isolated_env(state), timeout=600)
        if proc.returncode:
            raise RuntimeError(f"context failed for {task['id']}")
        report = json.loads(proc.stdout)
        paths = list(dict.fromkeys(report["ranked"] + [e["path"] for e in report["evidence"]]))
        conn = sqlite3.connect(f"file:{next(state.rglob('index.sqlite3')).as_posix()}?mode=ro", uri=True)
        terms = query_terms(task["query"], stop)
        expression = " OR ".join('"' + t.replace('"', '""') + '"' for t in terms)
        order = [r[0] for r in conn.execute(
            "SELECT files.path FROM docs JOIN files ON files.id = docs.rowid WHERE docs MATCH ? "
            "ORDER BY docs.rank ASC, files.path ASC", (expression,))] if terms else []
        features = candidate_features(conn, terms, expression) if terms else {"files": 0, "df": {}, "rows": []}
        conn.close()
        gold = set(task["gold_files"])
        raw = [i + 1 for i, p in enumerate(order) if p in gold]
        final = [i + 1 for i, p in enumerate(paths) if p in gold]
        row = {"id": task["id"], "query": task["query"], "terms": terms, "matches": len(order),
               "raw_rank": raw[0] if raw else None, "in_window": bool(raw and raw[0] <= WINDOW),
               "final_rank": final[0] if final else None, "gold": task["gold_files"], "top3": paths[:3],
               "intent": report["intent"], "identifiers": report["identifiers"], "ranked": report["ranked"],
               "candidates": features}
        row["cause"] = cause(row)
        rows.append(row)
        print(f"[recall] {row['id']}: raw={row['raw_rank']} final={row['final_rank']} {row['cause']}", file=sys.stderr)
    tally: dict[str, int] = {}
    for row in rows:
        tally[row["cause"]] = tally.get(row["cause"], 0) + 1
    write_json(out / "candidate-recall.json", {"status": "EXECUTED", "split": args.split, "window": WINDOW,
                                               "tasks": len(rows), "causes": tally, "per_task": rows})
    print(json.dumps(tally), file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
