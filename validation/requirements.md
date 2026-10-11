# Requirements

| Need | Used by | Notes |
|---|---|---|
| Windows 10 or 11, x64 | all | Tests in `crates/findex-index/tests/windows_fs.rs` are `cfg(windows)` |
| Rust toolchain >= 1.85 (MSRV) via rustup, `x86_64-pc-windows-msvc` host, `cargo fmt`, `cargo clippy` | unit | `CARGO_TARGET_DIR` defaults to `%LOCALAPPDATA%\findex-build\target` (outside OneDrive). The scripts put `%USERPROFILE%\.cargo\bin` first on `PATH`: a standalone `gnullvm` Rust install earlier on `PATH` bypasses `rust-toolchain.toml` and yields a `findex.exe` that needs `libunwind.dll`, which fails when launched without that directory on `PATH` (observed: exit `0xC0000135`) |
| `cargo-audit` | unit | Optional; step is `SKIPPED` without it |
| Git | corpora, retrieval baseline (`git grep`), A/B workspaces | |
| Python 3.12 + `psutil` | harness | `py -3.12 -m pip install psutil` |
| Network | first `corpora.py` run, Claude components | Retrieval, performance, security and unit run offline once corpora exist |
| Official Claude Code CLI, already logged in | integration, A/B | Never logged in by the harness; `BLOCKED` otherwise. Pro/Max OAuth works; `--bare` needs an API key and is not used |
| Developer Mode or admin | security (file symlink check) | `SKIPPED` without privilege |
| ~6 GB free disk | corpora (vscode is the largest) and per-run state | |

Corpora (pinned, permissive licences): findex `44f1a38`, ripgrep `14.1.1`,
requests `v2.32.3`, zod `v3.23.8`, fmt `11.0.2`, pybind11 `v2.13.6`, tokio
`tokio-1.40.0`, vscode `1.94.0` (`d78a74b`). Resolved commits are checked
against `golden/symbol_navigation.jsonl` by `harness/corpora.py`.
