# Evidence-backed quality targets

> Execute sequentially in the current checkout; no subagents or paid APIs.
> Use the existing approved optimization scope and the native Rust/SQLite engine.

**Goal:** Improve real-issue localization, call-site precision and honest coverage
without sacrificing existing navigation, source validation or bounded resources.

**Architecture:** Keep lexical search's public contract stable. Diagnose and improve
task-context selection inside `crates/findex-index/src/context.rs`; prefer source
evidence and explicit uncertainty. Reuse the existing real-issue and SCIP-oracle
harnesses, with immutable binary identities and failure detection.

**Acceptance targets:** issue hit@1 >= .60 and gold recall@10 >= .90; resolvable
call precision >= .90, with total site recall and confident-call coverage reported
to expose abstention tradeoffs; zero false certainty on critical adversarial
fixtures; every context response states source/scope status. Agent bugfix outcome
requires a new paired live campaign and is not inferred from retrieval scores.

**Constraints:** Preserve 100k-file / 512-KiB admission, privacy and stale-source
checks; no runtime model/API/dependency addition. The authorized cumulative live
session cap is already exhausted. Historical test splits are reused regression
evidence, not a fresh independent held-out study. Do not tune on test failures.

- [x] Freeze current native binary and golden/oracle inputs; restore public
  corpus histories in owned `target/quality` directories. Measure dev baseline.
- [x] Harden existing localization evaluation against failed commands and invalid
  reports; support paired context binaries and record exact identities. Test
  failure cases before using its results.
- [x] Diagnose dev localization misses, add generic failing source fixtures,
  implement the smallest bounded context-ranking change and measure dev gains.
- [x] Diagnose owner/receiver false positives using dev oracle only; add failing
  ambiguity/alias/trait fixtures before changing classification. Preserve possible
  sites and report certainty coverage rather than hiding missing calls.
- [x] Add explicit machine-readable scope/provenance where the current context
  contract lacks it. Verify stale, skipped, capped and ambiguous cases.
- [x] Freeze candidate; evaluate all historical issue controls, dev/heldout
  oracle samples and the 115-query navigation regression. Report misses and
  target failures honestly. Run native fmt/clippy/tests/release + MSRV, privacy,
  budget and coverage checks appropriate to the changes.
- [x] Update evidence/README, refresh graphify without API calls, and keep main
  current only after passing checks. Do not infer bugfix success or universal
  superiority from these local measurements.

## Coding checkpoint — 2026-10-28

- Engine and harness changes are implemented sequentially. Native stable gates:
  fmt, Clippy `-D warnings`, 167 Rust tests, release build and RustSec all pass in
  `validation/results/20261028-quality-code-final/`. The prior attempt preserves
  a formatting failure; it is not the final passing gate.
- Rust receiver annotations are bounded to 2048 hints and 200,000 scope-walk
  steps per source. Overflow or incomplete syntax never fabricates a type.
- Earlier dev experiments are exploratory: conditional call precision improved
  while confident-site coverage declined. They do not pass the complete quality
  target and must not be advertised as broad improvement.
- User sequencing: finish coding first, then evaluate the frozen final candidate
  and conduct subscription live tests within an explicit renewed session budget.
  The old cumulative 12-session authorization is exhausted. No new live sessions
  were started during implementation.
- Renewed authorization: exactly six additional Codex subscription sessions,
  two bugfix tasks × three tools × one repetition, approved on 2026-10-28.
  No paid APIs, no agents/subagents, no fallback/retry beyond this cap.
- Frozen post-coding candidate SHA-256:
  `ccb6299d315c18d7d65c06d86b640f9e54100f1092f928e51bd65d2638243c73`.
  Subsequent work prepares test fixtures/harnesses; engine sources stay frozen
  during measurement. At this checkpoint, new live starts were zero.

## Verification and integration checkpoint — 2026-10-28

- All 65 real-issue controls and eight 80-function paired call-oracle runs are
  recorded. Hit@1 is 40.0%, gold recall@10 79.23%; the targets remain unmet.
  Conditional call precision improves at substantial confident-coverage cost.
- Fresh native stable/MSRV gates pass 167 Rust tests; 42 Python tests, 29
  security checks, 115 navigation equivalences, 199 coverage claims and 960
  rendered-budget checks pass. Graphify was refreshed AST-only without models.
- Five subscription sessions started; four reported read-only execution, the
  fifth was interrupted, and the sixth did not start. The audited journal has
  zero evaluable bugfix trials and preserves original flags separately.
- A corrected zero-model preflight verifies workspace write/exec, outside-write
  rejection and MCP availability in all six arms. No global permissions or
  authentication files were changed. The updated Codex executable differs from
  the frozen pilot; drift checks prevent reusing the old campaign.
- The user declined new sessions and requested verified source/documentation
  integration into main. No new live model calls are authorized or performed.
- Implementation and measurement work are complete for this source checkpoint;
  the quality targets and public release remain open. See
  `docs/engineering/QUALITY_TARGETS_20261028.md` for exact limits.
