"""Semantic relationship precision: SCIP (rust-analyzer) -> findex_impact.

Every direct (depth 1) edge Findex reports is checked against the files on
disk: the cited reference line and definition line must both contain the
referenced symbol's name. Indirect edges must be labelled as potential, not
proven. Recall proxy: for crate-unique function names, files that call
`name(` per `git grep` should appear among the direct hits.
Usage: relations.py [--corpus ripgrep] [--files 60]; writes relations.json.
"""
from __future__ import annotations

import argparse
import random
import re
import shutil
import subprocess
import sys
from collections import Counter

from common import CORPORA, WORK, CommandLog, findex_binary, isolated_env, results_dir, write_json
from golden import admitted_files, definitions
from mcp_client import McpProcess

IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def short_name(symbol: str) -> str | None:
    # "rust-analyzer cargo ignore 0.4.23 gitignore/GitignoreBuilder#build()." -> "build"
    fields = symbol.split(" ")
    if fields[-1] == "crate/" and len(fields) >= 4:
        # A crate root is referenced as `grep_searcher` or re-exported `grep::searcher`.
        return fields[2].split("-")[-1]
    names = IDENT.findall(fields[-1])
    return names[-1] if names else None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", default="ripgrep")
    parser.add_argument("--files", type=int, default=60)
    parser.add_argument("--out", default=None)
    args = parser.parse_args()
    out = results_dir(args.out)
    log = CommandLog(out)
    root = CORPORA / args.corpus
    if not shutil.which("rust-analyzer"):
        write_json(out / "relations.json", {"status": "BLOCKED", "reason": "rust-analyzer not installed"})
        return 0
    scip = WORK / "scip" / f"{args.corpus}.scip"
    scip.parent.mkdir(parents=True, exist_ok=True)
    made = log.run(["rust-analyzer", "scip", ".", "--output", str(scip)], cwd=root, timeout=1800)
    exe = findex_binary("current")
    env = isolated_env(WORK / "state" / out.name / "relations")
    log.run([str(exe), "index"], cwd=root, env=env)
    imported = log.run([str(exe), "graph-import-scip", str(scip)], cwd=root, env=env)
    files = sorted(f for f in admitted_files(root) if f.endswith(".rs"))
    sample = random.Random(11).sample(files, min(args.files, len(files)))
    lines_cache: dict[str, list[str]] = {}
    # `use a::generate as generate_man_page;` -> a reference may cite the alias.
    renamed: dict[str, set[str]] = {}
    for rel in files:
        for original, alias in re.findall(r"\b([A-Za-z_]\w*)\s+as\s+([A-Za-z_]\w*)",
                                          (root / rel).read_text(encoding="utf-8", errors="replace")):
            renamed.setdefault(original, set()).add(alias)

    def line(path: str, number: int) -> str:
        if path not in lines_cache:
            lines_cache[path] = (root / path).read_text(encoding="utf-8", errors="replace").splitlines()
        rows = lines_cache[path]
        return rows[number - 1] if 0 < number <= len(rows) else ""

    client = McpProcess(exe, root, env)
    checked, invalid, labels, truncated_targets, depth2, module_edges = 0, [], Counter(), 0, Counter(), 0
    direct_by_target: dict[str, set[str]] = {}
    for target in sample:
        payload, text, _, is_error = client.call_tool("findex_impact", {"path": target, "depth": 2, "limit": 32})
        if is_error or payload is None:
            continue
        truncated_targets += bool(payload.get("truncated"))
        direct_by_target[target] = set()
        for hit in payload.get("hits", []):
            labels[(hit["depth"], hit["interpretation"])] += 1
            if hit["depth"] != 1:
                depth2[hit["interpretation"]] += 1
                continue
            direct_by_target[target].add(hit["path"])
            name = short_name(hit["via_symbol"])
            checked += 1
            # SCIP defines a module (`.../name/`) by its file, so there is no named definition line,
            # and `crate::` / `super::` / `self::` are references to it by keyword.
            module = hit["via_symbol"].endswith("/")
            module_edges += module
            ref_words = set(IDENT.findall(line(hit["path"], hit["referenced_at_line"])))
            aliases = {"crate", "super", "self"}
            if hit["via_symbol"].endswith(" crate/"):
                aliases.add(hit["via_symbol"].split(" ")[2].replace("-", "_"))
            ref_ok = bool(name) and (name in ref_words or (module and bool(ref_words & aliases))
                                     or bool(ref_words & renamed.get(name, set())))
            def_ok = module or (bool(name) and name in line(hit["definition_path"], hit["definition_line"]))
            if not (ref_ok and def_ok):
                invalid.append({"hit": hit, "name": name, "ref_line": line(hit["path"], hit["referenced_at_line"])[:160],
                                "def_line": line(hit["definition_path"], hit["definition_line"])[:160]})
    client.close()

    # Recall proxy over corpus-unique, non-test function names defined in sampled files.
    defs = definitions(root, files)
    expected_total, found_total, per_name = 0, 0, []
    for target, direct in direct_by_target.items():
        for name, places in defs.items():
            if len(places) != 1 or places[0][0] != target or len(name) < 6 or name.startswith("test"):
                continue
            grep = subprocess.run(["git", "-C", str(root), "grep", "-l", "-w", "-E", rf"{re.escape(name)}\s*\("],
                                  capture_output=True, text=True).stdout.split()
            callers = {p for p in grep if p != target and p.endswith(".rs") and p in set(files)}
            if not callers:
                continue
            expected_total += len(callers)
            found_total += len(callers & direct)
            per_name.append({"name": name, "file": target, "grep_callers": len(callers), "found": len(callers & direct)})
    report = {
        "status": "EXECUTED", "corpus": args.corpus, "scip_generator": "rust-analyzer scip",
        "scip_exit_code": made.returncode, "import": imported.stdout.strip(), "import_exit_code": imported.returncode,
        "targets_queried": len(direct_by_target), "targets_truncated_at_32_hits": truncated_targets,
        "direct_edges_checked": checked, "direct_edges_to_modules": module_edges, "direct_edges_invalid": len(invalid),
        "direct_edge_precision": round(1 - len(invalid) / checked, 4) if checked else None,
        "invalid_samples": invalid[:10],
        "labels": [{"depth": d, "interpretation": i, "count": c} for (d, i), c in sorted(labels.items())],
        "indirect_edges_labelled": dict(depth2),
        "recall_proxy": {"definition": "files calling a corpus-unique fn name per git grep (-w name\\s*\\() found among direct hits",
                         "expected_files": expected_total, "found_files": found_total,
                         "recall": round(found_total / expected_total, 4) if expected_total else None,
                         "caveat": "grep also matches comments/strings/macros and same-named methods; hit limit 32 truncates",
                         "names": per_name[:50]},
    }
    write_json(out / "relations.json", report)
    print({k: report[k] for k in ("direct_edges_checked", "direct_edges_invalid", "direct_edge_precision")},
          report["recall_proxy"]["recall"], file=sys.stderr)
    return 0 if not invalid else 1


if __name__ == "__main__":
    raise SystemExit(main())
