# Section 3 — adaptive context work (review candidate)

Base: `enterprise/p0-safety` (now merged to main). Branch: `feat/section3-adaptive-context`.

Scope: intent classification, context-evidence selection, truthful negative and
bounded evidence. **Does not implement type-resolved calls, live LSP, a learned
semantic planner, or project-wide graph-based reasoning.** Those belong to
later semantic graph work and must not be marked complete.

Changes:
- Expanded English and Turkish caller/test/impact intent signals.
- Explicit navigation of filenames outranks incidental `tests/` path names.
- File deletion is treated as impact, with a clear unresolved graph warning.
- Test evidence is prioritized for test questions; concepts prioritize code
  over prose; definitions remain on the cheap exact lookup path.
- Concept lexical candidate window raised from 12 to 24 with truncation
  disclosure when the shortlist is incomplete.
- Missing verified source evidence is explicitly reported.
- Rust regression fixtures and native executable black-box smoke in Windows CI.

Verification:
- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace --all-targets` (dev and frozen held-out router sets)
- Rust 1.85 MSRV checks and tests
- release build and `validation/harness/section3_smoke.py`
- RustSec audit, no real Claude sessions

Limitations: no independent new natural-language golden set, no external full
localization benchmark rerun, no model-level token savings measurement on this
patch. Do not claim universal Section 3 acceptance until these are addressed.
