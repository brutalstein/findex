# Findex against published issue-localization work

Checked 2026-10-30. The comparison is indicative only: the benchmarks, inputs and
methods differ in ways that decide the numbers (see "Why the numbers do not line up").
The Findex 60% hit@1 and 90% recall@10 targets remain unmet; this file does not change that.

## Findex, as measured here

Model-free: no LLM runs at query time. Input is the issue title only. Output is a
ranked list of at most five files inside a 1,200-token budget.

| Set | Tasks | hit@1 | hit@3 | hit@10 | gold recall@10 |
|---|---|---|---|---|---|
| Fresh holdout v2 (frozen before scoring; requests, ripgrep, fmt: Python, Rust, C++) | 55 | 0.345 | 0.727 | 0.800 | 0.761 |
| Exposed set (tuned against for many rounds, so optimistic) | 65 | 0.585 | not recorded here | not recorded here | 0.900 |

With n of about 55 the standard error is about 6 points. Sources: FX-BM-019 and FX-BM-020 in
[BENCHMARKS.md](BENCHMARKS.md), and FX-DEC-20261030-004 in [DECISIONS.md](DECISIONS.md).

Same fresh set, same proxy, offline, rivals run by the same harness (FX-BM-021): git grep words
0.164 hit@1, codebase-memory 0.127, Graphify 0.036, against Findex 0.345. That is a file-ranking
proxy, not a verdict on the graph questions those tools target.

## Published systems

All rows are SWE-bench Lite file-level localization (300 Python issues), full issue text, an
LLM in the loop. They are quoted from the comparison table of the BLAgent paper
(Al Mamun and Uddin, TOSEM 2026, [arXiv 2605.17965](https://arxiv.org/abs/2605.17965)),
not from each original paper; none was re-run here.

| System | Top-1 | Top-3 | Top-10 | Note |
|---|---|---|---|---|
| Agentless, GPT-4o (embedding retrieval plus LLM) | 0.630 | 0.817 | 0.883 | BLAgent's reproduction |
| CoSIL, Qwen-2.5-32B (LLM-driven graph search) | 0.613 | 0.780 | not quoted | |
| LocAgent, Claude-3.5 (graph-guided agent) | 0.777 | 0.919 | not quoted | Top-5 0.941 |
| BLAgent, GPT-OSS-120B (agentic RAG) | 0.786 | not quoted | not quoted | MRR 0.851; above 0.86 Top-1 with a closed model |

The older information-retrieval line of work (BugLocator, BLUiR, AmaLgam, Locus) is evaluated on
Java projects in Bench4BL (10,017 reports, 51 subjects) and reported mostly as MAP. No Top-1 figure
was found to quote, so no number from that line is placed next to Findex's.

## Reading it honestly

- Findex trails the LLM-driven systems on hit@1 by a wide margin: 0.345 on the fresh set against
  0.61 to 0.79. A model-free title query is not a substitute for a model that reads the report and
  the code. The exposed-set hit@1 of 0.585 is the only Findex figure near the 0.61 to 0.63 band, and
  that set was tuned against.
- The gap narrows with depth: fresh hit@3 is 0.727 against published Top-3 of 0.78 to 0.92. That
  supports using Findex as a cheap first pass that hands an agent a short candidate list, not a
  claim of parity.
- On the model-free proxy it leads the three local rivals measured on the same tasks.

## Why the numbers do not line up

- Language and repositories: SWE-bench Lite is Python only. The fresh set mixes Python, Rust and C++
  repositories with different layouts.
- Query: published systems read the whole issue; Findex is scored on the title alone.
- Metric: published rows are Top-k accuracy over single gold files; Findex reports hit@k and gold
  recall@k, which counts every gold file of a multi-file fix.
- Model: the published rows use a model in the loop; Findex does not.
- Sample size: 300 tasks against 55, so the standard errors differ.
- Provenance: the published rows are third-party reproductions inside one paper, not independent runs.
