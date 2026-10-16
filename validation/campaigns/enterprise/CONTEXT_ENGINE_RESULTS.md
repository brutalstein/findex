# Context engine results

`findex context` / MCP `findex_context` (`crates/findex-index/src/context.rs`).

## Exhaustive-coverage claim (A1)

For every distinct golden name (280 across 7 corpora) the engine is asked
"callers of `name`" at budget 8000. When the report is untruncated and has a
coverage line, `validation/harness/coverage_check.py` compares it with
`git grep -n -w` over the files Findex admits. The checker's comment
detection is its own: Python `tokenize` for `.py`, and a whole-file scanner
for other languages written separately from Findex's line lexer (nested
block comments, char literals, multi-line strings). A claim holds only if
every missing line is a comment by that scanner, the comment count equals
the reported count, and only reported method-syntax lines are otherwise
missing.

| Binary | Claims holding | Run |
|---|---|---|
| frozen `main` `247576e` | 268/279 | `results/20261016T170902Z-7068bff0db43/coverage-main-247576e` |
| `7068bff` (nesting) | 278/279 (raw string in ripgrep `defs.rs` opened a comment) | `results/20261016T170902Z-7068bff0db43/coverage-branch` |
| `fbc73ad` (cross-line strings) | 279/279 | `results/20261016T171736Z-fbc73ade35e4/coverage-branch` |
| `d60e34b` (HEAD) | 279/279 | `results/20261016T173010Z-d60e34be88a1/coverage` |

The earlier "279/279" at `ac4ea21` used the same `#`/`*` rule in engine and
checker and could not see A1; it is superseded, not altered.

Known limits of both lexers (shortcut comments in code): C++ `R"(...)"` raw
strings and Python triple-quoted strings are not recognised. One requests
name (`httpbin`) is truncated and makes no claim, so 279 claims are checked.

## Intent router (A2, A3)

Labels: `router_cases.tsv` (dev, 260: definition 50, callers 50, tests 40,
impact 40, concept 50, file 30; English, Turkish, mixed) and
`router_heldout.tsv` (60, 10 per intent, written after tuning and frozen
before its first run). Same author wrote labels and rules, so neither set is
independent. The "before" figures come from a Python emulation of the
pre-change rules (it reproduces the 178/260 that the Rust test printed at
`cd2b397`).

| Set | Before (`cd2b397` rules) | After (`fbc73ad`) |
|---|---|---|
| dev | 178/260 | 258/260 |
| held-out | 30/60 | 50/60 |

Held-out after, per intent: definition 10/10, concept 10/10, file 9/10,
tests 9/10, impact 7/10, callers 5/10. The 50/60 vs 258/260 gap is the
overfitting estimate. Held-out misses: "who depends on", "what code paths
reach", "instantiate", "örneğini kim oluşturuyor", "callsites" (callers);
"does the suite verify" (tests); "delete", "silinirse", "blast radius of
editing" (impact); "show tests/util.rs" (file, routed to tests). Dev misses:
bare "implement" (kept out because "implemented" asks for a definition).
Both scores are pinned by unit tests (`router_accuracy_on_labeled_tasks` ≥ 258,
`router_accuracy_on_held_out_tasks` ≥ 50).

Effect on retrieval: RETRIEVAL_RESULTS (localization 0 tasks worse, 2 better).

## Budget (A8)

`validation/harness/budget_check.py` (ripgrep, 40 names × callers/definition),
rendered length / 4 ÷ budget, `results/20261016T173010Z-d60e34be88a1/budget-*`:

| Budget | `edd41bf` max / over budget | `d60e34b` max / over budget |
|---|---|---|
| 200 | 1.32 / 24 of 80 | 0.96 / 0 of 80 |
| 500 | 1.12 / 8 of 80 | 0.96 / 0 of 80 |
| 2000 | 1.00 / 1 of 80 | 0.96 / 0 of 80 |

The token estimate is characters / 4, not a model tokenizer: UNVERIFIED
against real token counts.
