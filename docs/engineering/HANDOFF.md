# Findex Engineering Handoff

Mandatory resumable agent state. Updated at the end of every meaningful
checkpoint so a future agent (or AI reviewer) can continue without repeating
the investigation.

## Current state

| Field | Value |
|---|---|
| Current branch | feat/fresh-holdout-v2 (PR #41 merged into main as 2559464) |
| Current commit SHA | 2559464 (main) plus the fresh-holdout, report-routing (d4718e9) and evidence commits on this branch |
| Working-tree status | Fresh holdout, report routing, evidence and docs committed on the branch; eight untracked `validation/results/20261029T...` directories from other sessions preserved |
| Last verified checkpoint | Local Windows gates for the report-routing change (fmt, Clippy, 250 of 251 Rust tests, 63 Python tests, release); no milestone completion claim |
| Current work | Fresh 55-issue holdout scored; candidate-recall decomposition done (recall is not the limit); problem reports around an identifier now also run the concept search; coverage, proximity and weight sweeps rejected on dev |
| Current error or blocker | No source-gate blocker. Initial TEMP/cache permission failures and broad inherited test-state ACL were resolved with isolated workspace fixtures and private test-state ACLs |
| Most recent successful command | Native fmt, Clippy, all-target Rust tests (249 pass), release build; 63 offline Python tests; six compiler-backed CLI/MCP cases; 115 unchanged navigation controls; eight native hardening checks; 200–100k scaling and 100001-file transactional rollback/recovery PASS |
| Most recent failed command | Initial hardening fixture selected parent Git root (839 files instead of one); fixed by explicit fixture Git roots; rerun passed |
| Exact next action | Read FX-DEC-20261030-004 and `validation/results/20261030-recall-dev-experiments/NOTE.md`. Accuracy needs a new signal class (not title-only lexical weights); architecture gates 2 and 3 of RETRIEVAL_INTEGRITY_20261030.md remain open |
| Files currently being modified | `crates/findex-index/src/context.rs` (report routing), localization harness (`candidate_recall.py`, `--dataset` options), engineering records |
| Tests that must be rerun after further source changes | Native fmt, Clippy, workspace all-target tests and release; semantic fixtures and relevant frozen controls must identify the new binary |
| GitHub publication status | PR #40 and #41 merged; this branch is pushed and submitted as a PR; no release |
| Known risks | Fresh 55 issues: hit@1 34.5%, hit@3 72.7%, recall@10 76.1% (targets 60%/90% unmet); exposed 65: hit@1 58.5%, recall@10 90.0%, which overstates generalization by about 20 points; the fresh test half (27) is unchanged by the routing change, so the change is supported by the dev halves only; bugfix evidence is only a two-task, four-start Codex pilot of old vs new Findex (retrieval-delivery); standalone semantic queries/new-path events still reconcile whole project |

## Evidence links

- Worklog: docs/engineering/WORKLOG.md
- Issues: docs/engineering/ISSUES.md
- Benchmarks: docs/engineering/BENCHMARKS.md
- Decisions: docs/engineering/DECISIONS.md
- Status: docs/engineering/STATUS.md
- Release readiness: docs/engineering/RELEASE_READINESS.md
- Validation: validation/
- CI: .github/workflows/ci.yml
- Pull request: none yet
- Current assessment: docs/engineering/RETRIEVAL_INTEGRITY_20261030.md
- Current native evidence: validation/results/20261030-retrieval-integrity/

## Next three engineering priorities

1. Audit candidate generation recall independently of ranking and snippet
   packing, with a new frozen issue set rather than tuning the reused 65 tasks.
2. Evaluate a compiler-backed context adapter above the index/relations crates;
   measure precision, recall and confident coverage; retain ambiguity and
   inspect alias traversal bounds/provenance.
3. Profile dirty-path scheduling and snapshot generations before partitioning.
   Keep new files, ignore changes, branch switches and event loss conservative;
   require fresh native scale/rollback evidence for each changed binary.
