# Rivals on the fresh holdout (v2) — 2026-10-30

`localization_rivals.py --dataset validation/golden/issue_localization_v2.jsonl`: the same 55 frozen tasks, each
rival indexes the parent commit through its documented surface and receives only the issue title. Graphify:
`extract --code-only` then `query --budget 8000`. codebase-memory: `index_repository` then `search_graph`. No model or
API calls. The result file is copied unchanged into `../20261030-localization-v2-baseline` and
`../20261030-localization-v2-hybrid` so the progress report can read it next to Findex's numbers.

| all 55 | hit@1 | hit@3 | hit@10 | gold recall@10 | MRR |
|---|---:|---:|---:|---:|---:|
| Findex context (`b767d40c2b`) | 0.345 | 0.727 | 0.800 | 0.761 | 0.538 |
| git grep words | 0.164 | 0.309 | 0.691 | 0.606 | 0.307 |
| codebase-memory | 0.127 | 0.382 | 0.545 | 0.461 | 0.278 |
| Graphify | 0.036 | 0.309 | 0.564 | 0.461 | 0.206 |

Scope: this is one offline file-ranking proxy with issue titles as queries, using each rival's generic search
surface; it is not a verdict on either tool for graph questions they were built for. On the exposed 65 the earlier
ranking was codebase-memory 0.246, Graphify 0.123 and git grep 0.062 hit@1 (different task set, so the numbers
are not comparable across the two sets).
