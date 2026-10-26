# 0.2.0 release preparation — 2026-10-26

Native Windows verification of the working tree based on `e4fe732`, including
the existing 0.2.0/forget changes and the argument-validation fix.
Toolchain: rustc 1.96.1, cargo 1.96.1, Windows MSVC.

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace --all-targets` | PASS: 145 tests, zero failures or ignored tests |
| `cargo build --release -p findex` | PASS |
| `hardening_offline.py target/release/findex.exe` | PASS: 8 checks; see `hardening-offline.json` |
| `plugin_check.py` against the release binary and repository plugin | PASS; see `plugin-check.json` |
| `cargo metadata --no-deps --locked` and plugin manifest | All seven workspace packages and the plugin report 0.2.0 |
| Release `--version` and `--help` | 0.2.0; recovery and existing context/graph commands listed |
| `git diff --check` | PASS |

Regression evidence: before the fix, `forget unexpected` exited successfully
and deleted the disposable corrupt cache. With the argument guard, the harness
passes and confirms that invalid arguments preserve the cache, recovery works,
repeated deletion succeeds, source contents survive, and another project's
index bytes remain unchanged.

Release binary SHA-256:
`34191ef07264d830c425de6424e6a54240f5746b9edbd09739cebb75735a39b3`.

No live Claude/model sessions or paid API calls. No publication, signing, MSRV,
Windows 10 or new performance/accuracy campaign was run. Imported SCIP/LSP
evidence must be reimported after `forget`; stop project MCP sessions first.

Graphify was built locally with `--code-only` and refreshed with
`graphify update .`. Cargo.toml produced no graph nodes and was inspected
directly. Documentation was read directly; no semantic/API extraction ran.
