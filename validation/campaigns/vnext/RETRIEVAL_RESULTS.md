# Retrieval results — issue localization

Run: `validation/results/20261012T101245Z-3a2e7de81241/` (`localization.json`,
`localization_bootstrap.json`, `commands.jsonl`). Code commit `3a2e7de`.

## Dataset (frozen, independent)

`validation/golden/issue_localization.jsonl`, built by `harness/issues.py`
(committed in `a430538` before any system was evaluated on it):

- 65 real fixed issues: requests 10, ripgrep 25, zod 5, fmt 25.
- Query = the reporter's GitHub issue title. Gold = non-test source files
  changed by the commit that closed the issue (1–3 files). Each system runs on
  the fix's **parent** commit, so the fix is not in the index.
- Split fixed in advance by issue parity: odd = dev (37, inspected during
  development), even = held-out test (28, never inspected per task).
- Neither the labeller nor Findex produced any label.

## Systems

| System | What it is |
|---|---|
| gitgrep-words | files ranked by count of distinct title words found by `git grep -i -w` |
| findex-main | `findex search <title>` from the frozen main build `caf33fa` |
| findex-context | `findex context --json --budget 8000 <title>` at `3a2e7de` |

## Held-out test split (n = 28)

| System | hit@1 | hit@3 | hit@5 | hit@10 | MRR | gold recall@10 | empty | output bytes p50 |
|---|---|---|---|---|---|---|---|---|
| gitgrep-words | 0.000 | 0.250 | 0.464 | 0.750 | 0.198 | 0.738 | 0 | 205 (paths only) |
| findex-main | 0.107 | 0.286 | 0.571 | **0.786** | 0.281 | **0.756** | 0 | 985 |
| findex-context | **0.286** | **0.607** | **0.750** | 0.750 | **0.451** | 0.750 | 0 | 4357 |

Paired bootstrap (10,000 draws, seed 11), findex-context minus findex-main:

| Metric | Difference | 95 % CI |
|---|---|---|
| MRR | +0.169 | [+0.083, +0.263] |
| hit@1 | +0.179 | [+0.036, +0.321] |
| hit@5 | +0.179 | [0.000, +0.357] |
| hit@10 | −0.036 | [−0.143, +0.071] |

findex-context minus gitgrep-words: MRR +0.252 [+0.142, +0.369], hit@1 +0.286
[+0.143, +0.464], hit@10 0.000 [−0.143, +0.143].

**Reading.** On held-out data, `findex_context` puts the right file higher
(MRR, hit@1, hit@3 improve with CIs excluding zero). It does **not** find
more right files in the top 10: hit@10 is equal or slightly lower, with a CI
that includes zero. The output is about 4× larger than `findex search`,
because it carries code snippets; that is the cost of a one-call answer.

## Dev split (n = 37; used during development)

| System | hit@1 | hit@5 | hit@10 | MRR | empty |
|---|---|---|---|---|---|
| gitgrep-words | 0.108 | 0.486 | 0.568 | 0.294 | 0 |
| findex-main | 0.189 | 0.486 | 0.622 | 0.309 | 0 |
| findex-context before D5 (`96295ee`) | 0.270 | 0.486 | 0.568 | 0.352 | 0.081 |
| findex-context (`3a2e7de`) | 0.297 | 0.514 | 0.595 | 0.391 | 0 |

The D5 change (text fallback, tests after source) was tuned on dev only. On
dev, context minus main MRR is +0.083 with CI [−0.015, +0.189], i.e. not
significant: the held-out gain is larger than the dev gain, so it is not an
artefact of tuning on held-out data.

## Known failure classes (dev split)

- Vendored third-party code inside the package (`requests/packages/urllib3`)
  outranks the package's own files. Not fixed: there is no reliable,
  language-independent vendored-code signal, and guessing would create false
  negatives in projects whose real code lives under `packages/`.
- Issue titles about build flags or platforms (`-Wsign-conversion`, MSVC
  `permissive-`) name no code; all three systems mostly fail.
- `deno/lib` mirrors of `src/` in zod split the gold set; Findex returns one
  of the two copies.

## Frozen symbol benchmark (regression only)

The earlier 642-query symbol set is retained for regression checking. vNext
did not change `findex_search` ranking code (the only shared change is
`DefinitionRow` gaining `end_line`). The benchmark was **not re-run** in this
campaign; the last measured value is R@1 0.984
(`20261012T001625Z-bf4967d90524`).
