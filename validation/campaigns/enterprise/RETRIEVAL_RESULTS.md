# Retrieval results

## Symbol / lexical retrieval (642 golden queries, 7 corpora)

`validation/harness/retrieval.py --systems findex-current,findex-current-symbol`
at `01ff624` (`results/20261016T172700Z-01ff6241d30e`) vs. the baseline run at
`fb1b000` (`results/20261016T105926Z-fb1b000c2bef`):

| Slice | Baseline file R@1 / MRR | `01ff624` file R@1 / MRR |
|---|---|---|
| all (642) | 0.9844 / 0.991 | 0.9844 / 0.991 |
| held-out split (190) | 0.9737 / 0.985 | 0.9737 / 0.985 |
| `symbol_exact` (280) | — | 0.986 / 0.992 |
| `identifier_words` (239) | — | 0.979 / 0.987 |

Status: PASS, unchanged. `findex search` ranking was not modified in this
campaign; the schema-4 change only alters deletion behaviour.

## Issue localization (65 real fixed issues, `findex context`)

`validation/harness/localization.py`, title only, evaluated at each fix's
parent commit. Same session, same corpora:

| Build | hit@1 | hit@3 | hit@10 | MRR | gold recall@10 | empty |
|---|---|---|---|---|---|---|
| `7068bff` (before router changes) | 0.292 | 0.508 | 0.662 | 0.417 | 0.646 | 0 |
| `fbc73ad` (router + file intent) | 0.308 | 0.508 | 0.662 | 0.426 | 0.646 | 0 |
| `d60e34b` (HEAD, budget fix) | 0.308 | 0.508 | 0.662 | 0.426 | 0.646 | 0 |

Per task, 2 of 65 changed, both better (`fmt-3813` rank 2 → 1;
`ripgrep-2664` not found → rank 13); no task got worse
(`results/20261016T171736Z-fbc73ade35e4`, `results/20261016T173010Z-d60e34be88a1`).
Status: PASS (no false-negative regression). 65 tasks are too few to call
the improvement significant.

False-negative considerations for the router change: a task that names a
file but no identifier now lists that file's definitions instead of running
a text search; if the file is not indexed the engine falls back to the text
search and says so. A title with a dotted word that looks like a file
(`node.js`) is treated as a file request first. Neither case occurred in the
65 localization titles.
