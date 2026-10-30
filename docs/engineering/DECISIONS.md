# Findex Engineering Decisions

Significant architecture and algorithm decisions. Each entry documents problem,
alternatives, evaluation criteria, measurements, decision, tradeoffs,
compatibility consequences, and reversal conditions.

## FX-DEC-20261029-001 — Cross-platform CI matrix (Windows/Linux/macOS)

**Problem:** CI ran only on Windows; platform support for Linux/macOS was
claimed but not externally verified.

**Alternatives:**
1. Keep Windows-only CI (status quo).
2. Add Linux/macOS to the CI matrix.

**Evaluation criteria:** Coverage of supported platforms, CI cost, flakiness
risk, ability to catch Unix path bugs.

**Measurements:** Local Windows clippy/test/build all pass; `storage_dir()`
made cross-platform (LOCALAPPDATA on Windows; XDG_CACHE_HOME/HOME on Unix).

**Decision:** Add `windows-latest`, `ubuntu-latest`, `macos-latest` matrix to
`.github/workflows/ci.yml` with fail-fast disabled.

**Tradeoffs:** Increased CI minutes; Unix path behavior not yet validated by
CI run; `notify` watcher and `rusqlite` behavior on Unix untested.

**Compatibility:** Windows-first native target preserved; no breaking changes.

**Reversal conditions:** If Linux/macOS CI fails persistently, revert to
Windows-only and mark Unix support as unsupported.

**Commit:** f3bdd2d

## FX-DEC-20261029-002 — Property-based test strategy (avoid unwrap/expect)

**Problem:** proptest `string_regex().unwrap()` and `.expect()` violate
`#![deny(clippy::unwrap_used, clippy::expect_used)]` in findex-core and
findex-index test modules.

**Alternatives:**
1. Add `#[allow(clippy::unwrap_used, clippy::expect_used)]` to test modules.
2. Replace `string_regex().unwrap()` with `any::<String>()` + `prop_assume!()` guards.
3. Use `unwrap_or_else(|_| panic!(...))` (still triggers `expect_used`? no, but `panic` in proptest is poor practice).

**Evaluation criteria:** clippy compliance, test quality, adversarial input coverage, simplicity.

**Measurements:** clippy `-D warnings` clean after change; 202 tests pass.

**Decision:** Replace regex strategies with `any::<String>()` + `prop_assume!()` guards for null, length, and ASCII constraints; replace `ok().expect()` with `match` on `Result`.

**Tradeoffs:** `any::<String>()` with assume guards covers a narrower input space than bounded regex strategies; adversarial Unicode/special-character coverage reduced.

**Compatibility:** No API changes; test-only.

**Reversal conditions:** If property tests fail to catch regressions, reintroduce bounded regex strategies with an `#[allow]` on the specific lint.

**Commit:** 84fbfbd

## FX-DEC-20261029-003 — CLI integration test `expect_used` allowance

**Problem:** Test helpers in `crates/findex-cli/tests/cli_integration.rs` use
`expect()` for binary path resolution and process spawning; clippy denies
`expect_used`.

**Alternatives:**
1. Add `#![allow(clippy::expect_used)]` at test file level.
2. Replace all `expect()` with `unwrap_or_else()` + error propagation.
3. Wrap binary resolution in a fallible helper returning `Result`.

**Evaluation criteria:** clippy compliance, test simplicity, failure diagnostics.

**Measurements:** 9 CLI integration tests pass; clippy `-D warnings` clean.

**Decision:** Add `#![allow(clippy::expect_used)]` at the top of the integration test file.

**Tradeoffs:** Test file exempt from expect_used denial; acceptable because tests are not production code and `expect()` messages are diagnostic.

**Compatibility:** No production code changes.

**Reversal conditions:** None anticipated; test-only allowance.

**Commit:** 0490018

## FX-DEC-20261030-001 — Windows-only support; Linux/macOS CI removed

**Problem:** FX-DEC-20261029-001 added Linux and macOS jobs. Both failed on
every push (`symbols::tests::file_inspection_obeys_ignore_admission_and_detects_stale_edits`),
and the same change replaced the 457-line Windows pipeline (release, signing
hook, `cargo deny`, RustSec, Server 2022, MSRV, harnesses, attestation, SBOM)
with a 51-line generic job.

**Decision:** Findex supports Windows x64 only, as stated in AGENTS.md. This
triggers the reversal condition of FX-DEC-20261029-001: the Windows pipeline is
restored from `fdf2b25`, Linux and macOS jobs are removed, and Unix is
unsupported. The release job runs on `ubuntu-latest` only to publish the
already-built Windows artifacts; it does not run Findex.

**Consequences:** The Unix fallback in `storage_dir()` is not exercised and is
not a support claim.

**Commit:** this change

## FX-DEC-20261030-002 — Localization ranking: structure and size signals, no git-history prior yet

**Problem:** Issue localization (65 tasks) sat at hit@1 0.446. Misses were dominated by
vendored copies and docs/support scripts outranking project source, and by FTS5's fixed BM25
length normalisation (b = 0.75) penalising the large central files that fixes touch.

**Decision:** Concept ranking multiplies the BM25 score by path weights (vendored 0.5,
docs/examples/support 0.7) and relaxes length normalisation toward b = 0.3. Both values were
fixed before measuring; no published sweep of b for source files was found. A bug-fix-history
prior (files fixed often are fixed again; BugLocator/AmaLgam) is deferred: obtaining it means
running `git` inside an untrusted repository, where repository-local configuration such as
`core.fsmonitor` can execute commands, which conflicts with the threat boundaries in SECURITY.md.
It needs a parser of the object store or a hardened, reviewed invocation first.

**Evidence:** FX-BM-012 and FX-BM-013 in BENCHMARKS.md (hit@1 0.446 to 0.538; held-out half
0.464 to 0.536; 19 tasks up, 5 down).

**Repository hygiene found:** ten remote feature branches already have merged PRs and are stale;
PR #27 (`feat/section3-adaptive-context`) conflicts with main in `context.rs` and `ci.yml` and
predates the Windows-only pipeline. Neither was deleted or closed here.

**Commit:** this change

## FX-DEC-20261030-003 — Concept output names the ranked files

**Problem:** The first live localization study (FX-BM-015) showed Findex losing sessions it should
win: offline `findex context` ranks the gold file first on seven of the eight issues, yet three live
Haiku sessions ran out of turns and one answered other files. The text output grouped snippets by
kind (definitions first), so the rank order was invisible, and doc-comment snippets of the top file
often looked unrelated to the issue.

**Decision:** Concept output opens with `ranked files, most likely first: a, b, c, d, e` (five
files, rank order, tests behind source as in the ranking). It is rendered text only; `--json` and the
MCP structured output are unchanged, so offline localization numbers do not move. The line is
counted in the token budget.

**Evidence:** FX-BM-016. Same eight issues, Findex only: 4/8 to 6/8 correct, 5/8 to 7/8 valid,
mean turns 13.1 to 5.0, tokens per session 420,352 to 143,014. The change was designed after reading
those failures, so the re-run is a regression check, not an independent test; rivals were not
re-run. Two losses remain (ripgrep-2122: the model overrode the ranking; zod-2184: gold ranks 16th).

**Commit:** this change
