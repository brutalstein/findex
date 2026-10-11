# Contributing

The initial project is Windows-first and Rust-only. Contributions must preserve the official Claude Code authentication and permission model. Do not intercept credentials, impersonate the official client, or silently upload source code.

Before opening a pull request, run these on Windows with stable Rust:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --release -p findex
```

Report resource usage, retrieval relevance and correctness separately. All product claims require reproducible baselines. Prefer focused, well-tested modules over broad unverified changes. File an issue first for protocol or schema changes.
