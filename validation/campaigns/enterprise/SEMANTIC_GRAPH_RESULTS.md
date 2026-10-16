# Semantic graph results

## P1 typed graph: NOT_RUN

The typed relationship graph (edge kinds, ambiguous-name resolution,
compiler-backed tests) was **not built** in this campaign. The relations
database still has no schema version and no typed edge kinds; impact is file
level; `findex_context` does not use SCIP data (audit A7, open).

## Existing evidence (unchanged, not re-run)

Call-site precision/recall of `findex context` against a rust-analyzer SCIP
oracle, 80 sampled functions per corpus, seed 7, at `3a2e7de`
(`results/20261012T101245Z-3a2e7de81241/callsites-*`):

| Corpus | Call precision (macro) | Site recall (macro) | Unique-name functions: precision / recall |
|---|---|---|---|
| ripgrep | 0.519 | 0.951 | 0.822 / 1.000 (34 functions) |
| tokio | 0.391 | 0.878 | 0.787 / 1.000 (33 functions) |

Low precision comes from same-named functions elsewhere: Findex matches
names, not resolved symbols, and says so in every report. These numbers
predate this branch; the call-site path changed only in comment/string
lexing (more code lines shown, never fewer), so precision can only have
moved by lines that were previously hidden as comments. Status: UNVERIFIED
at HEAD (not re-run; the SCIP oracle generation step was not repeated).
