# Findex Engineering Handoff

Mandatory resumable agent state. Updated at the end of every meaningful
checkpoint so a future agent (or AI reviewer) can continue without repeating
the investigation.

## Current state

| Field | Value |
|---|---|
| Current branch | fix/retrieval-integrity (PR #40 merged into main as 083c0d3) |
| Current commit SHA | 083c0d3 (main) plus the retrieval-integrity commit on this branch |
| Working-tree status | Retrieval/semantic/watcher fixes, regression tests, harness and checkpoint docs; previous untracked validation runs preserved |
| Last verified checkpoint | Local Windows retrieval-integrity gates; no new milestone completion or CI claim |
| Current work | Existing ranking exported consistently to JSON; semantic stale/truncated diagnostics and stale-prefix refill fixed; excluded watcher events filtered before queue admission; hardening fixture project roots isolated |
| Current error or blocker | No source-gate blocker. Initial TEMP/cache permission failures and broad inherited test-state ACL were resolved with isolated workspace fixtures and private test-state ACLs |
| Most recent successful command | Native fmt, Clippy, all-target Rust tests (249 pass), release build; 63 offline Python tests; six compiler-backed CLI/MCP cases; 115 unchanged navigation controls; eight native hardening checks; 200–100k scaling and 100001-file transactional rollback/recovery PASS |
| Most recent failed command | Initial hardening fixture selected parent Git root (839 files instead of one); fixed by explicit fixture Git roots; rerun passed |
| Exact next action | Read RETRIEVAL_INTEGRITY_20261030.md and its evidence before continuing; next product gate is candidate-recall decomposition on a fresh frozen issue set |
| Files currently being modified | context JSON, semantic-site loader/tests, watcher admission/tests, localization/hardening harness and engineering records |
| Tests that must be rerun after further source changes | Native fmt, Clippy, workspace all-target tests and release; semantic fixtures and relevant frozen controls must identify the new binary |
| GitHub publication status | PR #40 merged; the retrieval-integrity repairs are committed on `fix/retrieval-integrity` and submitted as a separate PR; no release |
| Known risks | 65 exposed tasks: hit@1 58.46%, recall@10 86.92%, below 60%/90%; JSON ranking correction moves fmt-2695 from gold rank 3 to 4; no new model-session bugfix evidence; standalone semantic queries/new-path events still reconcile whole project |

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
