# Agent instructions for Findex

Findex is a Rust native Windows application. Keep the official Claude Code executable and its authentication entirely separate from Findex; do not fork/impersonate Claude or access its credentials. Avoid false performance claims. Any change to retrieval ranking must come with reference tests, measurements and consideration of false negatives.

Review module boundaries in `docs/architecture.md`, threat boundaries in `SECURITY.md`, and milestone criteria in `docs/roadmap.md` before modifying code. Keep MCP stdout strictly JSON-RPC and logging on stderr. Never read unrelated directories or follow symlinks outside the authorized project. Native Windows test evidence is mandatory for marking a milestone complete.

Run on Windows: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, and `cargo build --release -p findex`.
