# Candidate-recall decomposition and dev-only ranking experiments — 2026-10-30

Tool: `validation/harness/candidate_recall.py` (dev half only: v1-dev 37 + v2-dev 28 = 65 tasks). For each
task it rebuilds the concept query, reads the raw FTS5 BM25 order from the index, and records the inputs of the
concept score for the 128-candidate window.

Decomposition with the previous binary (`../20261030-recall-v1-dev`, `../20261030-recall-v2-dev`):

| cause | v1-dev (37) | v2-dev (28) | total (65) |
|---|---:|---:|---:|
| gold never matched a query term | 0 | 2 | 2 |
| gold outside the 128-candidate window | 0 | 0 | 0 |
| in window, ranked beyond 10 or absent | 4 | 10 | 14 |
| ranked 2-10 | 12 | 9 | 21 |
| ranked 1 | 21 | 7 | 28 |

Candidate recall is therefore not the bottleneck; ordering is. Four of the 14 deep misses were absent from the
output although raw BM25 held the gold file at rank 2-18 (requests-2207, requests-1293, ripgrep-2479, zod-3437). Cause: 13 of the 65 tasks contain an identifier
(`MissingSchema`, `.extend`, ...) and were routed to the identifier lookup, which returned definitions only and
an empty `ranked`. Concept-only ranking of those 13 gives hit@3 11/13 (7/13 before) and hit@10 12/13 (8/13).
That is the change shipped in d4718e9.

Offline replay (`replay.py`) reproduces the Rust ordering exactly for the tasks that were concept-routed in
the previous binary; the `-p` captures were taken with the candidate binary, so the 5 tasks it reports as
mismatches are the ones the new routing changed. Experiments on the 65 dev tasks (outputs in the `.txt` files):

- BM25 length parameter b, filename and symbol-name multipliers, path weights (`sweeps.txt`, pooled 65):
  every variant moves 1-3 tasks. The two largest, per half: b = 0.1 gives v1-dev hit@10 +1 task and v2-dev
  hit@1 +1 and hit@10 +1 (recall@10 +0.03 / +0.02); symbol multiplier 3 gives recall@10 +0.05 on both halves
  but v2-dev hit@1 -2 tasks. All are within one or two tasks of noise at n=28-37, so none was adopted.
- IDF-weighted and plain term coverage as a multiplier (`coverage.txt`): helps v2-dev, hurts v1-dev. Rejected.
- Term proximity (best line / five-line window) as a multiplier (`proximity.txt`): no hit@1 gain; hit@10 +1
  task on v1-dev only. Rejected.

No ranking weight was changed. The remaining hit@1 misses are mostly large hub files (`app.rs`, `format.h`,
`core.h`) outranking a small file that holds the fix; title-only lexical signals do not separate them.
