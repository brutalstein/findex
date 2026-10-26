"""Call-site precision/recall of `findex context` against a compiler oracle.

Oracle: rust-analyzer SCIP occurrences (validation/harness/relations.py
generates %FINDEX_VALIDATION_HOME%/scip/<corpus>.scip). For a deterministic
sample of functions, every non-definition occurrence of the function's SCIP
symbol is a true reference site. Findex sees only the name (`callers of
`name``), so same-named functions elsewhere count against its precision.
Reported: precision of Findex `call` lines, recall of oracle lines by any
Findex site kind, and caller-file precision/recall.
With --qualified the task names the function the way a developer would,
`Type::name` or `module::name` from the SCIP symbol (unchanged name-only task
when the symbol has no type or module). --split heldout draws a disjoint
sample (seed 11) that was frozen before any qualifier heuristic was written.
Usage: callsites.py [--corpus ripgrep] [--sample 80] [--qualified] [--split dev|heldout]
"""
from __future__ import annotations

import argparse
import json
import random
import re
import shutil
import sys
from collections import defaultdict

from common import CORPORA, WORK, CommandLog, findex_binary, isolated_env, percentiles, results_dir, write_json


def varint(buf: bytes, i: int) -> tuple[int, int]:
    value = shift = 0
    while True:
        byte = buf[i]
        i += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, i
        shift += 7


def fields(buf: bytes):
    """Yield (field number, wire type, value) of one protobuf message."""
    i = 0
    while i < len(buf):
        key, i = varint(buf, i)
        number, wire = key >> 3, key & 7
        if wire == 0:
            value, i = varint(buf, i)
        elif wire == 2:
            length, i = varint(buf, i)
            value, i = buf[i:i + length], i + length
        elif wire == 1:
            value, i = buf[i:i + 8], i + 8
        elif wire == 5:
            value, i = buf[i:i + 4], i + 4
        else:
            raise ValueError(f"unsupported wire type {wire}")
        yield number, wire, value


def occurrences(index: bytes):
    """(path, 1-based line, symbol, roles) for every SCIP occurrence."""
    for number, _, document in fields(index):
        if number != 2:  # Index.documents
            continue
        path, occs = None, []
        for field, _, value in fields(document):
            if field == 1:  # Document.relative_path
                path = value.decode("utf-8")
            elif field == 2:  # Document.occurrences
                start_line, symbol, roles = None, "", 0
                for f, wire, v in fields(value):
                    if f == 1:  # range: packed int32 (or repeated varint)
                        start_line = varint(v, 0)[0] if wire == 2 else v
                    elif f == 2:
                        symbol = v.decode("utf-8")
                    elif f == 3:
                        roles = v
                occs.append((start_line, symbol, roles))
        for start_line, symbol, roles in occs:
            if path and start_line is not None and symbol and not symbol.startswith("local "):
                yield path.replace("\\", "/"), start_line + 1, symbol, roles


FUNCTION = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\([^)]*\)\.$")


def function_name(symbol: str) -> str | None:
    match = FUNCTION.search(symbol)
    return match.group(1) if match else None


IMPL = re.compile(r"impl#\[`?&?(?:'\w+ )?(?:mut )?([A-Za-z_]\w*)[^\]]*\](?:\[([A-Za-z_]\w*)[^\]]*\])?[A-Za-z_]\w*\([^)]*\)\.$")
OWNER = re.compile(r"([A-Za-z_]\w*)[#/][A-Za-z_]\w*\([^)]*\)\.$")


def qualifier(symbol: str) -> str | None:
    """`Type` or `module` that owns a SCIP function symbol; None at a crate root."""
    descriptors = symbol.split(" ", 4)[-1]
    impl = IMPL.search(descriptors)
    if impl:
        # A one-letter type is a generic parameter (`impl<M> Trait for &M`): name the trait.
        return impl.group(2) if len(impl.group(1)) == 1 and impl.group(2) else impl.group(1)
    owner = OWNER.search(descriptors)
    return owner.group(1) if owner and owner.group(1) != "impl" else None


assert qualifier("rust-analyzer cargo r 1 search/impl#[`SearchWorker<W>`]should_preprocess().") == "SearchWorker"
assert qualifier("rust-analyzer cargo g 1 impl#[`&'a M`][Matcher]capture_index().") == "Matcher"
assert qualifier("rust-analyzer cargo r 1 flags/defs/impl#[IgnoreCase][Flag]doc_long().") == "IgnoreCase"
assert qualifier("rust-analyzer cargo r 1 misc/ignore_hidden().") == "misc"
assert qualifier("rust-analyzer cargo t 1 io/Ready#is_empty().") == "Ready"
assert qualifier("rust-analyzer cargo tokio 1.40.0 join_size().") is None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", default="ripgrep")
    parser.add_argument("--sample", type=int, default=80)
    parser.add_argument("--out", default=None)
    parser.add_argument("--qualified", action="store_true")
    parser.add_argument("--split", choices=("dev", "heldout"), default="dev")
    args = parser.parse_args()
    out = results_dir(args.out)
    log = CommandLog(out)
    scip = WORK / "scip" / f"{args.corpus}.scip"
    if not scip.exists():
        write_json(out / "callsites.json", {"status": "BLOCKED", "reason": f"{scip.name} missing: run relations.py first"})
        return 0
    defs, refs = {}, defaultdict(set)
    for path, line, symbol, roles in occurrences(scip.read_bytes()):
        if not function_name(symbol):  # functions and methods only
            continue
        if roles & 1:  # SymbolRole.Definition
            defs.setdefault(symbol, (path, line))
        else:
            refs[symbol].add((path, line))
    names = defaultdict(list)
    for symbol in defs:
        names[function_name(symbol)].append(symbol)
    candidates = sorted(s for s in defs if 1 <= len(refs[s]) <= 200 and len(function_name(s)) >= 3)
    sample = random.Random(7).sample(candidates, min(args.sample, len(candidates)))
    if args.split == "heldout":
        rest = sorted(set(candidates) - set(sample))
        sample = random.Random(11).sample(rest, min(args.sample, len(rest)))
    exe = findex_binary("current")
    root = CORPORA / args.corpus
    state = WORK / "callsites-state"
    shutil.rmtree(state, ignore_errors=True)
    log.run([str(exe), "index"], cwd=root, env=isolated_env(state), timeout=900)
    rows = []
    for symbol in sample:
        name = function_name(symbol)
        owner = qualifier(symbol) if args.qualified else None
        target = f"{owner}::{name}" if owner else name
        proc = log.run([str(exe), "context", "--json", "--budget", "8000", f"callers of `{target}`"],
                       cwd=root, env=isolated_env(state), timeout=300)
        report = json.loads(proc.stdout)
        sites = [(e["path"], e["line"], e["kind"]) for e in report["evidence"]
                 if e["kind"] in ("call", "possible call", "reference", "import")]
        truth = refs[symbol]
        calls = {(p, l) for p, l, k in sites if k == "call"}
        # `possible call`: Findex marks the receiver type unknown; reported separately.
        possible = {(p, l) for p, l, k in sites if k == "possible call"}
        found = {(p, l) for p, l, _ in sites}
        truth_files, call_files = {p for p, _ in truth}, {p for p, _ in calls}
        rows.append({"symbol": symbol, "name": name, "task_target": target,
                     "same_name_definitions": len(names[name]),
                     "oracle_sites": len(truth), "findex_calls": len(calls), "findex_sites": len(found),
                     "call_precision": len(calls & truth) / len(calls) if calls else None,
                     "possible_calls": len(possible),
                     "call_or_possible_precision": len((calls | possible) & truth) / len(calls | possible)
                     if calls | possible else None,
                     "site_recall": len(found & truth) / len(truth),
                     "caller_file_precision": len(call_files & truth_files) / len(call_files) if call_files else None,
                     "caller_file_recall": len(call_files & truth_files) / len(truth_files),
                     "truncated": report["truncated"],
                     "missed_examples": sorted(truth - found)[:3]})
        print(f"[calls] {name}: P={rows[-1]['call_precision']} R={rows[-1]['site_recall']:.2f}", file=sys.stderr)

    def mean(key, subset=rows):
        values = [r[key] for r in subset if r[key] is not None]
        return sum(values) / len(values) if values else None

    unique = [r for r in rows if r["same_name_definitions"] == 1]
    micro_calls = sum(r["findex_calls"] for r in rows)
    write_json(out / "callsites.json", {
        "status": "EXECUTED", "corpus": args.corpus, "oracle": "rust-analyzer SCIP non-definition occurrences",
        "functions": len(rows), "seed": 7 if args.split == "dev" else "11 (disjoint from seed 7)",
        "split": args.split, "qualified_task": args.qualified,
        "qualified_functions": sum(r["task_target"] != r["name"] for r in rows),
        "macro": {k: mean(k) for k in ("call_precision", "call_or_possible_precision", "site_recall",
                                        "caller_file_precision", "caller_file_recall")},
        "macro_unique_names": {k: mean(k, unique) for k in ("call_precision", "site_recall", "caller_file_precision", "caller_file_recall")},
        "unique_name_functions": len(unique),
        "micro": {"call_precision": sum((r["call_precision"] or 0) * r["findex_calls"] for r in rows) / micro_calls if micro_calls else None,
                  "site_recall": sum(r["site_recall"] * r["oracle_sites"] for r in rows) / sum(r["oracle_sites"] for r in rows)},
        "oracle_sites_per_function": percentiles([float(r["oracle_sites"]) for r in rows]),
        "limits": "SCIP misses references inside unexpanded macros and cfg-disabled code; Findex sees only the name",
        "rows": rows})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
