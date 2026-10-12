# Relationship results — call sites against a compiler oracle

Run: `validation/results/20261012T101245Z-3a2e7de81241/callsites-{ripgrep,tokio}/`.
Harness: `validation/harness/callsites.py`. Code commit `3a2e7de`.

## Method

- Oracle: rust-analyzer SCIP (`rust-analyzer scip .`). For a function symbol,
  every non-definition occurrence is a true reference site. SCIP is produced
  by the compiler front end, independently of Findex.
- Sample: 80 functions/methods per corpus with 1–200 references and names of
  3+ characters, drawn with seed 7.
- Findex receives only `callers of `name`` (no type information) and returns
  `call`, `reference` and `import` sites (budget 8000 tokens).
- Metrics: call precision (Findex `call` lines that are oracle sites), site
  recall (oracle sites found by any Findex kind), caller-file precision and
  recall.
- ripgrep was used while developing the Rust method-syntax rule (dev). tokio
  was measured once, after the commit. It is held out only for this oracle
  measurement: the context engine was probed during development on one tokio
  function (`poll_proceed`, A/B task t2).

## Results (macro means over functions)

| Corpus | Names | n | Call precision | Site recall | Caller-file P | Caller-file R |
|---|---|---|---|---|---|---|
| ripgrep (dev) | unique | 34 | 0.822 | 1.000 | 0.897 | 0.975 |
| ripgrep (dev) | 2–3 definitions | 19 | 0.503 | 1.000 | 0.589 | 0.842 |
| ripgrep (dev) | 4+ definitions | 27 | 0.148 | 0.854 | 0.217 | 0.833 |
| ripgrep (dev) | all | 80 | 0.519 | 0.951 | 0.594 | 0.896 |
| tokio (held-out) | unique | 33 | 0.787 | 1.000 | 0.845 | 0.894 |
| tokio (held-out) | 2–3 definitions | 16 | 0.263 | 1.000 | 0.378 | 0.938 |
| tokio (held-out) | 4+ definitions | 31 | 0.061 | 0.686 | 0.070 | 0.666 |
| tokio (held-out) | all | 80 | 0.391 | 0.878 | 0.441 | 0.814 |

Micro call precision (all call lines pooled) is 0.108 (ripgrep) and 0.076
(tokio): names like `new`, `poll` or `len` produce hundreds of name matches
and dominate the pool.

## Interpretation

- For a uniquely named function, Findex finds every compiler-confirmed site
  (recall 1.0 on both corpora) with about 4 of 5 call lines correct.
- For shared names, most call lines belong to other functions of the same
  name. Findex says so in the report (`` `x` has N definitions; call sites are
  name matches, not type-resolved ``), but it cannot tell which ones are right.
- Recall drops for 4+ definitions because the 8000-token budget truncates
  (11 of 27 ripgrep and 14 of 31 tokio reports were truncated, and say so).
- A typed graph is available only through optional SCIP import
  (`findex graph-import-scip`, `findex_impact`). Python, TypeScript and C++
  SCIP indexers were not installed, so cross-language edge precision is
  **unmeasured** (unchanged from REMAINING_LIMITATIONS #4).

## Not done

The spec's typed multi-language relationship graph with independent
validation (P2) was not built. The measured weakness is name ambiguity, and
resolving it requires type information that only a compiler-grade indexer
provides; a Tree-sitter heuristic graph would carry the same ambiguity under a
more confident label.
