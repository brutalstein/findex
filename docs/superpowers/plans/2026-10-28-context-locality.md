# Context locality and live reliability implementation plan

**Goal:** Improve task localization at the existing context budget, remove unused
semantic setup from ordinary comparisons, and preserve actionable live failure
evidence. Broad superiority is an evaluation outcome, never an assumption.

**Architecture:** Reuse SQLite FTS5, bounded candidate selection and existing
source validation. Change concept ranking only; exact search/symbol APIs retain
their ranking. No dependencies, paid APIs, hidden indexers or subagents.

**Execution:** Inline, sequential, as requested. The user requested planning
followed by implementation and live testing without a separate approval round.
The named `codex-max` skill is absent from both installed skill roots; existing
Ponytail, planning, TDD and verification workflows are used instead.

## Baseline and alternatives

- Baseline commit cc3c375; binary SHA-256
  5ccdbc390cfcfbcf198437c1ca6e0f41885306cf1dad5546e65196d5693e2f8e.
- 65 historical issues: hit@1 40.0%, gold recall@10 79.23%, goals 60%/90%.
- Primary live: Findex 1/2 (one unexplained timeout); both alternatives 2/2.
  One diagnostic retry passes; it cannot replace the primary failure.
- Rust semantic preparation adds about 40 seconds, yet the diagnostic uses
  context only. Semantic identity lookup is explicitly separate from context.
- Considered adding embeddings or a compiler call to every task: rejected here
  because setup, dependencies and language coverage grow without measured gain.
- Chosen: bounded path-aware lexical ranking and local evidence, plus explicit
  optional semantic setup and failure diagnostics. If measured quality regresses,
  reject or narrow the candidate rather than change labels or acceptance rules.

## Tasks and gates

- [ ] Write failing reference tests in `crates/findex-index/src/context.rs` for
  exact filename terms against unrelated mentions, substring collisions, source
  validation and nearby evidence. Implement at `search_ranked_in_db` in
  `crates/findex-index/src/lib.rs` / context assembly, retaining caps/redaction.
- [ ] Measure dev issues against the frozen binary at 1,200 tokens; retain every
  attempted result. Require no lower dev hit@1/recall@10; then freeze code and
  report test aggregate once. Existing exposed test split is not a new holdout.
- [ ] Add offline tests and minimal changes to `bugfix_live.py`: explicit SCIP
  opt-in, recorded setup mode, path-free event diagnostics after normal/timeout
  exits. Keep the 300-second cap, no automatic retries, official ChatGPT login,
  immutable inputs, native sandbox and independent Windows grading.
- [ ] Run native fmt, Clippy, all Rust/Python tests, release, security, plugin,
  budget and navigation controls; real compiler fixtures remain separate.
- [ ] Freeze a new campaign: at most six subscription starts, the same two
  public tasks x three tools, one repetition, no paid API/subagents. No previous
  attempt is removed. No semantic snapshot unless explicitly selected before
  freezing. Report preparation, model/grading seconds, cache/uncached/output
  tokens, source-only guards and supplied regression success separately.
- [ ] Update README/evidence with actual results and limitations, refresh the
  AST graph, verify committed hashes, integrate and push main when gates pass.

## Interpretation and stop conditions

All known public tasks are exploratory, not blind proof. Six sessions cannot
establish universal superiority. A timeout counts against success and prevents
whole-study token ranking when usage is missing. Preserve invalid trials and
stop on an infrastructure/authorization guard failure; never spend an extra
session automatically. Full-language semantics, >100k files, signed release and
fresh SMB/OneDrive soak are outside this bounded iteration.
