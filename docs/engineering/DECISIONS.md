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
