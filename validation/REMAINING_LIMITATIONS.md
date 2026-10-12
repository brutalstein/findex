# Remaining limitations

Each entry: severity, exact technical cause, missing evidence, action
required. Severity is for a local developer tool that never uploads code:
*high* = can return wrong evidence or lose data in a supported workflow,
*medium* = degraded or unproven in a plausible workflow, *low* = narrow edge
or reporting gap.

## Retrieval and structure

1. **Golden set not human-reviewed** (medium). The 642 labels come from
   `harness/golden.py` regex definition finders written by the same agent that
   tuned ranking. They are independent of Findex's Tree-sitter code but not of
   the author. *Missing:* a human audit of a random sample, and tasks drawn
   from real issue histories. *Action:* review >= 100 random labels; add
   issue-derived queries.
2. **Only symbol-shaped and line-shaped queries benchmarked** (medium). No
   natural-language concept queries ("where is redirect auth stripped") are in
   the golden set, so the planner's behaviour on them is measured only
   indirectly by the A/B tasks. *Action:* add concept queries with
   human-chosen answers.
3. **No embeddings** (low). Not added: no measured failure of lexical +
   definition retrieval justified a model dependency. *Action:* revisit if
   concept-query recall proves low.
4. **Semantic edges measured on one Rust corpus only** (medium).
   `relations.py` verifies edges from rust-analyzer SCIP on ripgrep. Python,
   TypeScript and C++ SCIP indexers (scip-python, scip-typescript, scip-clang)
   were not installed, so cross-language edge precision is unmeasured. Edge
   recall is only a `git grep` proxy. Most direct edges on ripgrep are module or
   crate `use` references, so function-level call evidence is a minority of
   what `findex_impact` returns.
5. **Tree-sitter extraction is syntactic** (low, by design). Dynamic dispatch,
   macros that generate items, and overloaded C++ functions are not resolved.
   `findex_symbols` labels results as syntax definitions, not references.

## Reliability

6. **Relations database open has no BUSY retry** (low). `findex-relations`
   uses `BEGIN IMMEDIATE` for writes, but its first-time schema creation does
   not retry `SQLITE_BUSY` the way the lexical `open_schema` does. Two
   processes importing SCIP into a fresh project at the same instant can see
   "database is locked". *Missing:* a concurrent-import test. *Action:* reuse
   the lexical retry loop and add the test.
7. **Corruption detected at open time only** (low). A database corrupted
   while a server is running surfaces as a query error, not an automatic
   rebuild, until the next open.
8. **Watcher overflow and branch switches** (medium). The watcher workload
   covered 25 edits/creates/renames/deletes per run. Mass changes (checkout of
   another branch touching thousands of files) rely on the full-rescan
   fallback and were not stress-tested.
9. **OneDrive / network volumes not tested** (medium). Corpora and state ran
   on a local NVMe volume. Cloud placeholder files and SMB shares were not
   exercised.

## Claude Code and tokens

10. **A/B study is small** (medium). Six tasks, three repetitions, one model
    (Haiku) and one effort level. Measured: B vs A total tokens +5.5 %, paired
    difference +10.2k with a 95 % half-width of ±36.8k (run
    `20261012T001625Z-bf4967d90524`), so no saving or cost is demonstrated.
    Per-task standard deviations reach 65 % of the mean (t3 B). Results must
    not be extrapolated to Sonnet/Opus or to large tasks. *Action:* more
    repetitions, a stronger model, tasks sized to large repositories.
11. **Shared prompt cache across sessions** (medium, measurement). Cache
    reuse is account-wide, so cache-creation vs cache-read splits depend on
    run order. Counterbalancing (ABBA) spreads but does not remove this; total
    tokens are the primary metric for this reason.
12. **The model decides whether to call Findex** (medium). With Findex
    available, Haiku called it in 3 of 18 sessions (t1 and t2 only; 4 calls, 0.2
    per session overall) and used built-in Grep/Glob/Read otherwise. Savings
    depend on tool choice, which Findex cannot force. *Action:* measure with
    tool-use guidance in the project's `CLAUDE.md`, reported as a separate
    arm.
13. **Only Claude Code 2.1.281 tested; it negotiates 2025-11-25** (low). The
    2026-07-28 stateless path is covered by unit and black-box tests, not by a
    real client.
14. **OpenTelemetry not used** (low). Per-session usage came from
    `stream-json`. Enabling OTel would require user-level settings that the
    study excludes.
15. **Cost figures are Claude Code's client-side estimates** (low), not
    invoices. The account is on a subscription plan.

## Performance and resources

16. **Single machine, laptop timings** (medium). One AMD Ryzen 9 8945HX
    laptop, Windows 11. Small absolute differences (a few milliseconds, or
    less than 10 % on sub-second cold indexes) are within noise. No Windows 10
    machine was available.
17. **Larger index and cold-index memory** (low). The `terms` FTS column and
    the identifier expression index grow the database 11–13 % (tokio,
    vscode). Peak working set while cold-indexing vscode rose 25 % (24.7 →
    30.9 MiB) and was not root-caused. Cold indexing itself is 1.2–16×
    faster (run `20261012T005742Z-746f47e6bb69`). *Action:* profile
    allocation during the vscode cold index.

## Supply chain

18. **Attestation covers CI builds only** (low). The workflow attests the
    release ZIP on pushes to `main` and manual runs; pull requests get no
    OIDC token. Verified for CI run 38191592325 (`493627f`):
    `gh attestation verify findex-windows-x64.zip -R brutalstein/findex`
    passed, signer `.github/workflows/ci.yml@refs/heads/main`. Locally built
    binaries carry no attestation.
19. **Binaries are unsigned** (medium). No Authenticode signing certificate
    is available. SHA-256 checksums, a CycloneDX SBOM and (on non-PR runs) a
    build-provenance attestation are produced instead.
20. **Toolchain sensitivity** (low). A standalone `gnullvm` Rust earlier on
    `PATH` bypasses `rust-toolchain.toml` and yields a binary that needs
    `libunwind.dll`. The validation scripts force the rustup MSVC toolchain;
    nothing in the build system enforces it for ad-hoc builds.
