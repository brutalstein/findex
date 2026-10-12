# Findex — Final Validation Report

Machine: AMD Ryzen 9 8945HX (32 logical CPUs), 31.2 GiB RAM, NVMe, Windows 11
Pro 10.0.26200, Rust 1.97.0 `x86_64-pc-windows-msvc`, Python 3.12, Claude
Code 2.1.281 (Pro plan, OAuth). Dates: 2026-10-11/12.

Every number below is taken from a committed run directory under
`validation/results/`. Nothing here is estimated unless labelled so.

## 1. Executive summary

* **Retrieval quality improved substantially.** On 642 golden queries over 7
  pinned corpora, file Recall@1 rose from 0.695 (start build) to 0.984, MRR
  from 0.790 to 0.991, span Recall@1 from 0.522 to 0.970; on the held-out
  split R@1 is 0.974 and R@10 1.000. A careful `git grep` baseline reaches
  R@1 0.790 with ~126× larger payloads.
* **A pre-existing O(files²) indexing defect was found and fixed.** The start
  build read 262 GB and spent 251 s (p50) to cold-index VS Code 1.94 (6,877
  files); the final build reads 0.93 GB and needs 15.5 s.
* **Real Claude Code integration passes** via `--mcp-config`, `--plugin-dir`
  and the `findex claude` launcher (12/12 checks).
* **Token savings were *not* demonstrated.** In a controlled 36-session Haiku
  A/B study, the Findex arm used +5.5 % total tokens (paired difference
  +10.2k ± 36.8k, i.e. not significant) with 17/18 vs 18/18 task success.
  Haiku called Findex in 3 of 18 sessions (2 of 6 tasks).
* Security: 30/30 black-box checks pass; `cargo audit` clean; 0 sockets.
* Release gates: see §18. Not all pass; no 10/10 is claimed.

## 2. Starting and final Git commits

* Start: `44f1a38` (head of `fix/code-intelligence-hardening`, PR #20).
* Final code: `7566f89`; final validation commit: the head of `main` that
  contains this report.
* Branch: `validation/production-engineering`.

## 3. Implemented changes

| Commit | Change |
|---|---|
| `69c3ce8` | Definition-first retrieval (normalized identifier lookup over Tree-sitter definitions) + FTS5 with camelCase/path `terms`, coverage → same-line co-occurrence → BM25 ranking; lazy hash verification of returned hits only; credential redaction in excerpts; corrupt/legacy DB rebuild; dual-era MCP (legacy `initialize` + 2026-07-28 `server/discover`, `_meta` versions, `-32022`), oversized/invalid frame recovery; Tree-sitter fixes (C++ forward declarations, Python methods, TS class-field arrows, JSX via TSX grammar, Rust macros/consts/impls) |
| `1bfec4c` | `BEGIN IMMEDIATE` writes + BUSY retry on schema open; 9 Windows filesystem integration tests |
| `f74bef1`, `6730f7a`, `8727422`, `4d8e593`, `bf4967d` | Validation harness, PowerShell entry points, docs, SBOM, CI audit job + provenance attestation, result integrity |
| `8386913` | FTS rows keyed by `files.id` (removes O(n²) I/O); token-phrase line matching (no more empty line-1 excerpts) |
| `dd16d92`, `8525c50` | Deferred reconcile + whole-pass BUSY retry so unchanged reconciles stay read-only under concurrency |
| `7566f89` | Removes two needless borrows that `dd16d92` left and clippy 1.97 rejects (run 3 recorded the failure) |

## 4. Architecture before and after

Before: one FTS5 table (`path UNINDEXED, body`), BM25-only ranking, first
matching line as excerpt, rows deleted by path, deferred transactions,
legacy-only MCP handshake. After: `files(id, path UNIQUE, hash, size)` +
`docs(rowid = files.id, path, body, terms)` + `symbols` with an expression
index on the normalized identifier; two-stage search (definitions, then
lexical); `PRAGMA user_version` schema versioning with rebuild; MCP serves
both protocol eras. Details: `docs/architecture.md`.

## 5. Defects found and fixed

| Defect | Evidence of fix |
|---|---|
| O(files²) I/O when indexing (FTS5 delete by UNINDEXED column) | `lexical_rows_are_keyed_by_file_id…` test; vscode read 262 GB → 0.93 GB, 251 s → 15.5 s |
| Definitions ranked below usages; snake/camel queries missed | 4 index unit tests; retrieval R@1 0.695 → 0.984 |
| Empty line-1 excerpts for token-only matches (evidence validity 0.993) | `token_only_matches_cite…`; validity 1.000 |
| "database is locked" with concurrent writers | `concurrent_synchronize…` (Windows), 8/8 repeated passes |
| Concurrent no-op reconciles serialized behind the write lock (introduced in `1bfec4c`, measured 71 → 42 rps) | `unchanged_reconcile_does_not_wait…`; see §11 |
| Corrupt or legacy DB caused hard errors | 2 unit tests + black-box random-bytes DB check |
| MCP: oversized frame / invalid JSON ended the session | 4 MCP unit tests + black-box checks |
| C++ forward declarations reported as definitions; Python methods as functions; JSX parse errors | 6 syntax unit tests |
| Credential literals in excerpts | 2 unit tests + black-box check |
| Harness/validation defects (strict-MCP config dropping plugin servers; gnullvm toolchain yielding a `libunwind.dll`-dependent binary; plan-limit sessions scored as wrong; nDCG > 1; CRLF checksums; case-sensitive judge) | fixed in harness, documented in run `NOTE.md` files |

## 6. Tests executed

* Rust: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test
  --workspace`, release build, `cargo audit` (`tests.json`).
* Retrieval benchmark: 642 queries × 4 systems (`retrieval.json`).
* Semantic edges: rust-analyzer SCIP on ripgrep (`relations.json`).
* Performance: ripgrep, tokio, vscode × baseline/current, 3 cold reps each
  (`performance.json`, `memory.json`, `cpu.json`).
* Security/reliability: 30 black-box checks (`security.json`).
* Real Claude Code: 3 integration sessions (`compatibility.json`); 36 A/B
  sessions (`tokens.json`, `comparisons.json`).

## 7. Test pass/fail results

| Check | `746f47e` (run 3) | `7566f89` (run 4) |
|---|---|---|
| `cargo fmt --check` | PASS | PASS |
| `cargo clippy --workspace --all-targets -D warnings` | **FAIL** (2 × `needless_borrows_for_generic_args`) | PASS |
| `cargo test --workspace --all-targets` | 89 passed, 0 failed | 89 passed, 0 failed |
| `cargo build --release -p findex` | PASS | PASS |
| `cargo audit` | PASS | PASS |
| Retrieval benchmark | 642 queries, no quality regression | not re-run (no code change affecting search) |
| Semantic edges | 316/316 verified | not re-run |
| Security black-box | 30/30 | not re-run |
| Claude Code integration | 12/12 | not re-run |

Run 4 (`validation/results/20261012T012018Z-71a4b9c8209e`) re-records only the static checks and tests at the
fixed commit; the only code difference from run 3 is the removal of two `&`.

## 8. Windows compatibility evidence

`crates/findex-index/tests/windows_fs.rs` (all pass on Windows 11): Unicode
file names; paths > 300 characters; case-insensitive project roots share one
index; `mklink /J` junction not followed; invalid UTF-8/oversized/binary
skipped; exclusively locked file removed then recovered; `..\`, drive,
`\\?\`, UNC, ADS (`lib.rs:stream`), trailing-dot and device (`CON`) paths
rejected; four concurrent writers; hidden/build directories excluded. Real
watcher events (edit/create/atomic rename/delete) all observed (§11). Not
tested: Windows 10, OneDrive placeholders, network shares.

## 9. Retrieval benchmarks

Run `20261012T001625Z-bf4967d90524`, file-level metrics, line tolerance ±1,
payload = MCP/grep output bytes. Run 3 at `746f47e` reproduced every
quality figure below exactly (p95 latency there: 21.9 ms final, 41.2 ms start).

| System | Queries | R@1 | R@10 | MRR | span R@1 | nDCG@10 | top-1 FP | evidence valid | payload B | p95 ms |
|---|---|---|---|---|---|---|---|---|---|---|
| git grep | 642 | 0.790 | 0.980 | 0.860 | 0.597 | 0.890 | 0.210 | n/a | 175 781 | 146.5 |
| Findex start (`44f1a38`) | 642 | 0.695 | 0.964 | 0.790 | 0.522 | 0.836 | 0.305 | 0.993 | 1 245 | 47.8 |
| Findex final | 642 | **0.984** | **1.000** | **0.991** | **0.970** | 0.994 | 0.016 | **1.000** | 1 392 | 26.0 |
| Findex final, held-out only | 190 | 0.974 | 1.000 | 0.985 | 0.947 | 0.989 | 0.026 | 1.000 | 1 388 | 27.8 |
| `findex_find_symbol` (exact only) | 280 | 0.971 | 0.979 | 0.974 | 0.964 | 0.976 | 0.007 | n/a | 442 | 4.1 |

By query type (final vs start, R@1): exact symbol 0.986 vs 0.789;
identifier words 0.979 vs 0.561; line-term (non-definition) 0.992 vs 0.740.
Stretch targets (R@10 ≥ 0.95, MRR ≥ 0.90) are met on this dataset; the
dataset's limits are in `REMAINING_LIMITATIONS.md` (#1, #2).
Regression: mean payload +11.8 % (more `terms`-driven hits carry full
excerpts; `match_kind` field), still ~126× smaller than grep output.

Semantic edges (ripgrep, rust-analyzer SCIP, 60 target files): 316 direct
`findex_impact` edges, 316 verified on disk (257 are module/crate `use`
references, 3 resolved through `use … as …` aliases); 834 indirect edges, all
labelled `potential-transitive-file-impact`. Recall proxy vs `git grep`
callers of crate-unique functions: 0.82 (157 expected files; 7 of 60 targets
hit the 32-result cap).

## 10. Token A/B measurements

Run `20261012T001625Z-bf4967d90524`: Claude Code 2.1.281, `--model haiku`,
default effort, 6 tasks × 2 arms × 3 repetitions, ABBA order, $0.40
per-session cap, estimated spend $2.24. A = built-in Read/Grep/Glob/Edit;
B = same + Findex MCP (connected in all 18 B sessions).

| Task | Success A / B | Total tokens A | Total tokens B | Δ | Findex calls/session (B) |
|---|---|---|---|---|---|
| t1 find a definition (ripgrep) | 3/3 / 3/3 | 97 067 ± 15 209 | 79 589 ± 96 | −18.0 % | 0.7 |
| t2 list callers (tokio) | 3/3 / 3/3 | 129 175 ± 42 294 | 203 509 ± 59 210 | +57.5 % | 0.7 |
| t3 explain a rule (requests) | 3/3 / 2/3 | 138 093 ± 32 174 | 133 529 ± 87 752 | −3.3 % | 0 |
| t4 cross-file trace (findex) | 3/3 / 3/3 | 275 443 ± 60 243 | 227 346 ± 24 339 | −17.5 % | 0 |
| t5 bug fix (findex) | 3/3 / 3/3 | 258 919 ± 4 093 | 270 423 ± 48 940 | +4.4 % | 0 |
| t6 feature + test (findex) | 3/3 / 3/3 | 215 852 ± 43 651 | 261 555 ± 59 357 | +21.2 % | 0 |
| **All** | **18/18 / 17/18** | 185 758 | 195 992 | **+5.5 %** | 0.2 |

Mean ± sample standard deviation over 3 repetitions. Paired B − A total
tokens: +10 234, 95 % half-width ±36 783 (normal approximation): no
significant difference. Estimated cost +4.0 %. The single B failure (t3)
named `rebuild_auth` (which applies the rule) instead of `should_strip_auth`
(which decides it); the scripted judge is strict by design. One t6 B session
was re-judged after a harness bug (case-sensitive `kdbx` check); see the run's
`NOTE.md`. **Conclusion: with Haiku choosing freely, Findex was rarely used
and did not reduce tokens on these tasks.** Where it was used for lookup
(t1), tokens fell 18 %; on the caller-listing task its use coincided with
more tool calls and more tokens. These results do not extend to other
models, efforts or larger tasks.

## 11. CPU and RAM measurements

Run `validation/results/20261012T005742Z-746f47e6bb69`, release builds, 3 cold repetitions (p50), warm MCP
queries over stdio, 4 concurrent MCP clients sharing one DB. Baseline =
start build `44f1a38`.

| Corpus (files) | Build | Cold index s | Cold CPU s | Read during cold index | No-op reconcile s | Search p50 / p95 ms | Symbol p95 ms | 4-client rps | Peak WS MiB | DB MiB |
|---|---|---|---|---|---|---|---|---|---|---|
| ripgrep (195) | start | 0.62 | 0.52 | 115 MB | 0.110 | 13.0 / 35.3 | 2.8 | 182 | 17.1 | 4.5 |
| ripgrep (195) | final | 0.54 | 0.44 | 8.5 MB | 0.091 | 9.9 / 21.5 | 3.0 | 314 | 17.5 | 4.8 |
| tokio (763) | start | 2.29 | 2.17 | 1.39 GB | 0.192 | 25.4 / 40.4 | 2.7 | 142 | 15.3 | 8.5 |
| tokio (763) | final | 1.23 | 1.12 | 24 MB | 0.131 | 13.5 / 27.4 | 2.7 | 218 | 15.2 | 9.6 |
| vscode (6 877) | start | 251.3 | 249.8 | 262 GB | 1.455 | 3.3 / 13.8 | 2.9 | 96 | 24.7 | 118.2 |
| vscode (6 877) | final | 15.5 | 15.3 | 0.93 GB | 0.884 | 3.3 / 6.1 | 3.1 | 142 | 30.9 | 131.2 |

* Idle CPU of a running MCP server: 0.0 CPU-seconds per 10 s on every
  corpus and build. MCP server after the query workload: 8–10 MiB RSS,
  2–4 MiB private bytes.
* Real watcher (ripgrep, final build): edits 25/25, creates 10/10, atomic
  renames 10/10, deletes 10/10 reflected correctly; p50 latency 22 / 40 /
  39 / 38 ms (start build: 29 / 60 / 60 / 55 ms).
* Flagged regressions (> 10 %): vscode peak working set during cold
  indexing +25 % (+6.2 MiB; not root-caused, plausibly larger transactions
  now that indexing no longer serializes on I/O); DB size +11–13 % on tokio
  and vscode (the `terms` FTS column and the identifier expression index).
  Retrieval latency p95 on the `symbol_exact/dev` subset +15.7 % (6.8 →
  7.9 ms) while the overall p95 fell from 41.2 to 21.9 ms.
* The concurrency regression measured in run 2 (vscode 42 rps vs 71 at
  baseline) is gone after `dd16d92`: 142 rps vs 96. Run-to-run
  noise is large: the unchanged baseline build measured 71 rps in run 2
  and 96 rps in run 3.
* Release binary 12.8 MB (start 12.7 MB).

## 12. Security findings

30/30 black-box checks pass (run 3). Covered: 2 MiB frame, invalid
JSON, 5000-deep nesting, batch rejection; 10 path-escape forms; credential
redaction; `.env` never indexed; prompt-injection text only returned as JSON
data and server instructions mark results untrusted; `server/discover` and
`-32022`; no sockets during indexing or serving (10 ms sampling); file
symlink to outside not indexed (Developer Mode available); random-bytes DB
rebuilt; 200 random SCIP and 200 mutated LSP inputs without crash; indexer
killed at 0.2/0.6/1.5 s then recovered with `integrity_check = ok`.
`cargo audit` 0.22.2: no vulnerabilities or warnings. No unresolved
critical/high finding. Not done: coverage-guided fuzzing, external audit.

## 13. CI and build provenance

CI (`.github/workflows/ci.yml`) now also: generates a CycloneDX 1.5 SBOM
(`validation/harness/sbom.py`; 76 crates, all with licence metadata, 70 with
registry SHA-256) into the release ZIP; runs `cargo audit` in a separate job;
attests build provenance of the ZIP with `actions/attest-build-provenance`
v4.2.2 on pushes to `main` and manual runs. CI run 38191592325 on `main` (`493627f`): format/clippy/tests/release
build, Rust 1.85 MSRV and RustSec audit jobs all succeeded; `gh attestation
verify` on the downloaded `findex-windows-x64.zip` passed (signer
`ci.yml@refs/heads/main`, source digest `493627f`).

## 14. Known limitations

See `REMAINING_LIMITATIONS.md` (20 items with severity, cause, missing
evidence and action).

## 15. Outstanding risks

* Token benefit is unproven; claims of savings must not be made.
* Golden set labelled by the same agent that tuned ranking (held-out split
  mitigates, does not remove, this).
* Single machine; no Windows 10, OneDrive or network-volume evidence.
* Binaries unsigned.

## 16. Reproduction commands

```powershell
py -3.12 -m pip install psutil
.\validation\scripts\run-all.ps1 -TotalBudgetUsd 6     # everything, incl. real Claude sessions
.\validation\scripts\run-all.ps1 -Quick -Offline       # no network, no model usage
.\validation\scripts\generate-report.ps1 -OutDir validation\results\<run>
```

## 17. Evidence directories

| Run | Commit | Contents |
|---|---|---|
| `validation/results/20261011T210034Z-6730f7a14f26` | `6730f7a` | Pre-fix: documents the O(n²) defect; Claude sessions plan-limited (BLOCKED) |
| `validation/results/20261012T001625Z-bf4967d90524` | `bf4967d` | All components incl. integration and the A/B study |
| `validation/results/20261012T005742Z-746f47e6bb69` | `746f47e` (code `8525c50`) | Unit (clippy FAIL), retrieval, relations, performance, security, integration |
| `validation/results/20261012T012018Z-71a4b9c8209e` | `71a4b9c` (code `7566f89`) | Static checks and tests at the final code: all pass |

Each directory has `summary.md`, `manifest.json`, `environment.json`,
`commands.jsonl`, component JSON and `checksums.sha256` (verifiable with
`sha256sum -c` after checkout).

## 18. Ten-dimension scorecard

Scores are judgements against `acceptance-criteria.md`; a 10 would mean
the criterion is met with margin, on more than one machine, with
independent review. No dimension reaches that.

| # | Dimension | Gate | Score | Basis |
|---|---|---|---|---|
| 1 | Correctness | PASS (run 4) | 8 | All checks pass at `7566f89`; run 3 caught a clippy failure that slipped through a commit, so the gate held only because it was re-run |
| 2 | Retrieval quality | PASS; stretch met | 8 | R@1 0.695 → 0.984, held-out 0.974; labels not human-reviewed; no concept queries; payload +12 % |
| 3 | Semantic relationship precision | PASS | 7 | 316/316 direct edges verified, indirect edges labelled; one Rust corpus; recall only a proxy (0.82) |
| 4 | Performance | PASS | 8 | Search p95 lower on all 3 corpora; cold index 1.2–16× faster; one subset latency +15.7 %; single laptop |
| 5 | Resource efficiency | **not met** | 6 | Idle CPU 0; DB growth explained; vscode cold-index peak working set +25 % not root-caused |
| 6 | Reliability | PASS | 7 | Concurrency, kill/recover, corrupt DB, real watcher all pass; no mass-change, OneDrive or network-volume tests |
| 7 | Security | PASS | 7 | 30/30 black-box, audit clean, no sockets; no coverage-guided fuzzing, no external review, unsigned binaries |
| 8 | Real Claude Code compatibility | PASS | 8 | 12/12 via `--mcp-config`, `--plugin-dir`, `findex claude`; one client version, legacy protocol only |
| 9 | Measured token efficiency | PASS (measured, honestly reported); stretch **not met** | 3 | +5.5 % tokens, not significant; Findex used in 3/18 sessions; 17/18 vs 18/18 success |
| 10 | Maintainability and reproducibility | PASS | 7 | One-command runs, pinned corpora, checksummed results; run 3's environment step was started outside the scripts and recorded the wrong toolchain |

**Release gates.** Passed: format/lint/build/tests (at `7566f89`); no
known unresolved critical/high security defect; no known reproducible
correctness failure in core workflows; evidence never silently truncated
(truncation at 32 impact hits is reported); real MCP integration on Claude
Code 2.1.281; Windows workflows on the tested machine; no measured
regression concealed (all listed in §9 and §11). Not passed: the resource
dimension (#5), and any claim of token savings (#9 stretch) — Findex must
not be described as reducing Claude Code token usage on the basis of this
study.
