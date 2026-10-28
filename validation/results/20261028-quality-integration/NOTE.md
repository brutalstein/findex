# Native integration gates — 2026-10-28

Product source: `3e9d4d2`, unchanged during evaluation. Candidate and rebuilt
release SHA-256:
`ccb6299d315c18d7d65c06d86b640f9e54100f1092f928e51bd65d2638243c73`.
The later experiment-harness and documentation edits are checked separately.
All commands run natively on Windows; no model calls or paid APIs.

| Gate | Result | Evidence |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | `gates.json` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | `gates.json` |
| `cargo test --workspace --all-targets` | PASS: 167 tests | `gates.json`, `tests-summary.json` |
| `cargo build --release -p findex` | PASS | `gates.json`, `manifest.json` |
| MSRV 1.85.0 all-target workspace tests | PASS: 167 tests | `tests-summary.json` |
| Offline Python discovery | PASS: 42 tests | `tests-summary.json` |
| Native security quick suite | PASS: 29, zero skipped | [security](../20261028-quality-security/security.json) |
| Five-corpus coverage | PASS: 199/199; one name makes no claim | [coverage](../20261028-quality-coverage/coverage_check.json) |
| Rendered budget | PASS: 960 reports, zero overrun | [requests](../20261028-quality-budget-requests/budget_check.json), [fmt](../20261028-quality-budget-fmt/budget_check.json), [ripgrep](../20261028-quality-budget-ripgrep/budget_check.json), [Tokio](../20261028-quality-budget-tokio/budget_check.json) |
| Navigation | PASS: 115 old hit lists, 115 compact roundtrips, 115 competitor parser checks | [summary](../20261028-quality-navigation/summary.json) |
| Official Codex, no model | PASS: six workspace-write/exec/boundary + MCP checks | [preflight](../20261028-bugfix-live/native-preflight-v2.json) |
| Offline plugin contract | PASS: eight tools and native fixture evidence | `plugin.json` |
| Sensitive path exclusion | PASS | `sensitive-paths.json` |
| Corrupt-cache/MCP privacy hardening | PASS | `hardening.json` |
| Project overview and MCP smoke | PASS | `overview.json` |

Stable tests use rustup MSVC 1.97.0; MSRV uses rustup MSVC 1.85.0 with its
explicit `RUSTC` and toolchain bin first on PATH. The native installer upgrade,
repeat-install and corrupt-runtime guard run inside the Python suite with a
fake Codex registration command; they do not establish fresh official plugin
discovery or live Claude compatibility for this engine.

Coverage uses the independent grep/comment oracle over admitted files, not a
new type-resolution oracle. Budgets use rendered characters / 4. The security
suite is a bounded black-box check, not a certification. Fresh cloud hydration,
SMB, large-scale timing, signing and public-package verification were not run.

`checksums.sha256` binds committed result JSON and this gate record. Raw command
logs and model transcripts stay local under ignored paths. Earlier failed
formatting attempts remain in their original result directories; they are not
substituted for this passing gate.
