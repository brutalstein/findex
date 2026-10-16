# Final decision

Branch `enterprise/p0-safety`, based on `main` `247576e`. Every number
below is in the linked result documents; there is no aggregate score.

## Release state

| State | Decision | Reason |
|---|---|---|
| DEVELOPER_READY | **Yes** | Local Windows gate passes (fmt, clippy `-D warnings`, all workspace tests, release build); offline validation cannot start a model client; coverage claim holds 279/279 against an independent checker; retrieval unchanged; security 30/30. |
| PRODUCTION_CANDIDATE | **No** | Blocking: CI on this branch not yet confirmed green (UNVERIFIED); MCP errors can carry absolute paths (A11, FAIL); router held-out accuracy 50/60 with callers 5/10; the new router/lexer/budget behaviour was not checked with a live model (NOT_RUN by policy); Windows 10, network volumes and cloud-only OneDrive files NOT_RUN. |
| ENTERPRISE_READY | **No** | Additionally: no Authenticode signature (BLOCKED, no certificate purchased); typed semantic graph not built (NOT_RUN); index directory ACL not tested (A12); relations database unversioned (A7); prompt-injection resistance of the model UNVERIFIED. |

## Done in this campaign

| Phase | Item | Status |
|---|---|---|
| P0 | Repository audit (`docs/engineering/COMPLETE_REPOSITORY_AUDIT.md`), file inventory, baseline freeze | PASS |
| P0 | Offline-by-default runner, fail-closed live-model gate, CI spawn test | PASS |
| P1 | Typed graph, ambiguous names, compiler-backed graph tests | NOT_RUN |
| P2 | Intent router with ≥ 250 EN/TR/mixed cases, file intent, frozen held-out set | PASS (dev 258/260, held-out 50/60) |
| P2 | Budgeted packing counts the whole report | PASS (0/240 reports over budget) |
| P2 | Comment/string lexer behind the coverage claim | PASS (279/279) |
| P3 | Locked files kept in refresh and full reconciliation | PASS |
| P3 | Newer schema refused; 3 → 4 migration | PASS |
| P4 | Removed text zeroed from the cache | PASS (main DB file); WAL limitation open |
| P4 | Docs: README, SECURITY, architecture, doctor | PASS |
| P4 | MCP error path redaction (A11), ACL test (A12) | FAIL / NOT_RUN |
| P4 | Official GitHub Release | NOT_RUN (needs approval) |
| P5 | ≤ 12-session live smoke study | NOT_RUN (needs approval; plan in MODEL_VALIDATION) |

## Model usage and cost

New live model sessions: 0. Paid API calls: 0. Paid infrastructure: 0.

## Next steps, in order

1. Confirm CI on the pushed branch; merge through a reviewed pull request.
2. Fix A11 (strip absolute paths from MCP error texts) with a test.
3. Add held-out-failing phrasings only to a *new* dev set, then write a
   fresh held-out set; never edit `router_heldout.tsv`.
4. With operator approval, run the ≤ 12-session smoke study.
5. P1 typed graph and relations schema versioning.
