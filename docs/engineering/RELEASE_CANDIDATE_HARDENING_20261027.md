# Findex 0.2.0 preview candidate — 2026-10-27

Prepared locally on `codex/token-efficiency-hardening`, from 88fd423. No new
subagents, paid API calls, published tag or public release were used in this
continuation. This is a tested **unsigned Windows preview candidate**, not an
enterprise certification or a claim of superiority on all coding tasks.

## Changes and preserved behavior

- MCP `findex_search` and `findex_find_symbol` now accept optional compact JSON.
  Hit columns are named once; all-zero per-lookup synchronization counters use
  `sync: 0`. Nonzero work retains the full object. Default JSON is unchanged
  except the additive exact-symbol `truncated` diagnostic. Source strings,
  syntax flags, stale-result counts and truncation semantics are preserved.
- Excerpts redact before truncating. A regression reproduced a credential
  leaking when the closing quote fell just beyond the 240-character excerpt.
  ASCII and Unicode cases now pass through real indexed retrieval.
- Exact-symbol verification remains bounded; a sentinel row reports remaining
  candidates. Four stale candidates may still consume a limit-one verification
  window, but now zero hits explicitly carry `truncated=true`. This is not
  pagination or a guarantee that the symbol is absent.
- Evaluation now freezes source/tool identities, prevents overwriting attempted
  measurements, journals a live attempt before spawn, stops on interrupted or
  failed calls, validates real source anchors, and fixes nDCG duplicate ranks.
  Offline regressions run in CI rather than only the live-gate tests.
- Windows MSVC builds link the CRT statically. The plugin/package check uses
  only Windows system directories on PATH; the original local GNU/LLVM binary
  reproduced a missing `libunwind.dll` failure under that test. The final MSVC
  binary passes, and PE imports contain only Windows system DLLs.
- Packages include the changelog, security policy and documentation. README
  now distinguishes local retrieval from the assistant's provider transmission,
  and name-matched references from type-resolved calls. Architecture docs no
  longer incorrectly say corrupt caches are automatically rebuilt.
- Full synchronization now queues at most eight native file reads, rechecks
  admission before opening and writes SQLite rows in the original order.
  Thread-start failure falls back to serial reads. Targeted refresh uses the
  same bounded reader. No reader threads remain after synchronization.
- Full-text retrieval sorts bounded path/rank/hash metadata, then fetches bodies
  only within the existing candidate window. The metadata cursor pins the read
  snapshot; reference tests compare old/new SQL and a concurrent writer.
- Codex installation uses verified immutable runtime paths by SHA-256. Native
  regressions cover a locked legacy executable, locked repeated installation
  and a corrupt cached runtime that must preserve the prior MCP configuration.

No ranking, schema migration, language coverage, authentication provider or
source-admission boundary was changed. No new runtime dependency was added.

## Fresh evidence

Authoritative local run: `validation/results/20261027-performance-release/`.
`20261027-release-candidate/` is the passing predecessor before native I/O/SQL
optimization; it is retained with its original identities and counts.

| Check | Result |
|---|---|
| MSVC Rust 1.97.0 fmt, Clippy `-D warnings`, all-target tests, optimized build | PASS; 153 tests |
| Declared MSRV Rust 1.85.0, MSVC/static CRT | PASS; 153 tests |
| Offline Python evidence/gate/adapter/locked-installer regressions | PASS; 34 tests |
| RustSec advisory scan and cargo-deny license/bans/sources | PASS |
| Quick native black-box security/recovery/fuzz | 29 PASS, 0 FAIL, 0 skipped; 40 random SCIP + 40 mutated LSP inputs |
| Sensitive-path privacy | 17 secret-path shapes excluded; normal source still found |
| Corrupt-cache preservation, explicit recovery, project-scoped forget, private MCP errors | PASS |
| Cloud Files provider | No Findex-triggered downloads; local/cloud transitions verified |
| Loopback SMB | CLI and MCP edit/delete/create freshness PASS |
| 10k-file stress | 130 edits, 80 deletes, 60 renames, two concurrent reconciles; integrity PASS |
| Claude official plugin validator and bundled MCP launcher | PASS; Windows-system-only PATH |
| Native installed Codex plugin via app-server | Eight tools, exact fixture source, final compact/zero-sync response; zero model turns |
| Final executable navigation equivalence | 115/115 hit lists unchanged; 115/115 complete compact roundtrips |

The final comparison's binary SHA-256 is checked against the packaged executable.
Raw local test logs and transcripts remain ignored; structured results and
checksums are retained. Earlier GNU runs and failed environment/adapter setup
attempts remain labelled as intermediate evidence, not passing release gates.

## Comparative outcome

[`20261027-performance-comparison`](../../validation/results/20261027-performance-comparison/NOTE.md):
55 queries new relative to the historical comparison plus 60 regression controls.
All 115: Findex compact averages 263.83 raw output tokens, codebase tree 266.37,
Graphify code-only MCP 2274.87. File hit@1 is 112/115, 85/115 and 87/115.
The 55-query subset is 222.07 / 240.31 / 2663.47 tokens and 54/55 / 47/55 /
48/55 correct files. Findex's own JSON shrinks 28.5% overall without changing
source-hit lists or losing diagnostics.

The raw-token advantage over codebase is narrow overall and **does not hold on
three of four individual corpora or the historical 60 subset**. Both dev-selected
format revisions used the same evaluation queries for confirmation; no second
blind replication is claimed. Corpus-level failures are explicit in the saved
gates. No competitor cold-speed, RAM, idle-CPU or disk superiority was measured.

[`20261027-codex-live`](../../validation/results/20261027-codex-live/NOTE.md):
six real sequential Codex subscription sessions, two navigation tasks per tool,
all answers correct. Client-reported input+output tokens: Findex 149527,
Graphify 217280, codebase 592385. Cache differences are separate. The pilot
used the earlier matrix-only GNU build; it is not re-labelled as a live study
of the final MSVC executable. Final installed-client and source-hit equivalence
checks establish the compatibility bridge. Cumulative starts including the
earlier stopped study are twelve; no additional sessions were started.

## Native scaling and query work

[`20261027-native-scale-final`](../../validation/results/20261027-native-scale-final/NOTE.md)
freezes the previous and final MSVC executables. Each build/repetition gets its
own identical synthetic Rust directory and fresh database, with alternating
build order. OS/antivirus caches are **not** flushed. Three paired repetitions
cover 200/2,000/10,000 files; 100k is candidate-only, one repetition.

| Files | Previous initial index, median s | Final, median s | Final max sampled peak MiB |
|---|---:|---:|---:|
| 200 | 0.374 | 0.251 | 8.75 |
| 2,000 | 5.619 | 1.968 | 10.17 |
| 10,000 | 49.007 | 10.212 | 12.46 |
| 100,000 | Not measured | 106.159 | 23.39 |

The 10k improvement is 4.80x on this machine. The first study reused source
directories across builds; its asymmetric cache conditions must not be used to
claim a larger speedup. The corrected v2 precedes the final rebuild and has a
different executable hash; both predecessor studies remain labelled separately.
20-ms sampling may miss the final sub-20-ms resource interval. Files are tiny
synthetic Rust sources; this is a file-count test, not a large-byte-volume study.

All 245 warm-query checks return expected path/line anchors; all 105 paired
query hit lists match. At 100k, exact-symbol p95 is 3.47 ms,
unique lexical p95 4.18 ms and broad common-term p95
83.44 ms, with truncation explicit. First MCP startup/reconciliation
there takes 5.09 s; warm timings exclude startup.
Warm queries report zero traversed files. Every five-second idle sample records
0.0 CPU seconds at OS-counter resolution. All post-query thread counts are four.
The 100k warmed process RSS is 9.34 MiB,
with 39.37 MiB lifetime peak during
startup/queries; the table measures the separate CLI indexing process.

After nine edits plus a 100,001st file, the transaction fails, preserves all
nine old hashes and 100k rows, passes integrity, then admits all nine edits
after removing the extra file. The 100k file cap and 512 KiB source cap remain
deliberate limits; arbitrary project sizes and nonblocking cold scans are not
claimed.

[`20261027-wide-query-v2`](../../validation/results/20261027-wide-query-v2/NOTE.md)
uses 1,200 sources of 64 KiB each and ten alternating warm common-term calls
per build. All 20 full expanded responses match, including source flags and
truncation. Median native MCP latency falls 140.97 → 16.12 ms (8.74x);
p95 is 146.12 → 21.25 ms. Lifetime process peak is 14.39 → 13.24 MiB.
This is a targeted wide-body workload, not an average across real projects.
The first attempt stopped on a harness lifecycle mistake before timed
responses and is retained separately; the correction reuses existing close hooks.

## Review scope and remaining work

Reviewed core/index admission, excerpt and symbol retrieval, watcher freshness,
MCP boundaries, CLI integration, relation import trust checks, evaluation,
CI and packaging. Graphify scoped queries plus direct source/tests supplied
evidence; the parent codebase-memory MCP transport was unavailable. This was a
bounded engineering review, not an exhaustive independent audit of every line
or native dependency.

Before broad performance/enterprise claims: independently authored unseen tasks,
repeated live code-edit/architecture studies, wider language/compiler oracles,
Windows 10 and real remote SMB, larger mixed-project/soak measurements, and
publisher signing. Existing 100k-file / 512-KiB-file caps, lexical call matching
and known-shape-only secret redaction remain. The preview is explicitly unsigned;
verify SHA-256. Production/enterprise readiness and universal superiority remain
**not established**. Publishing/tagging and CI on the final commit are separate
from the completed local checks.

## Reproduce the Windows build

Use an x64 Visual Studio Developer terminal. If another Rust distribution is
ahead of rustup on PATH, select the actual MSVC binaries explicitly for this shell:

```powershell
$env:RUSTC = (& rustup which --toolchain stable rustc)
$env:PATH = "$(Split-Path -Parent $env:RUSTC);$env:PATH"
rustc -vV # must say host: x86_64-pc-windows-msvc
python validation/harness/unit.py --out validation/results/NEW-RELEASE-CHECK
python -m unittest discover -s validation/harness -p 'test_*.py'
python validation/harness/plugin_check.py --plugin plugin --marketplace .claude-plugin/marketplace.json --binary target/release/findex.exe
```

For MSRV, select `1.85.0` for both cargo/rustc (not a standalone compiler still
on PATH), then run `cargo test --locked --workspace --all-targets`. Native client
checks and the comparison have their own commands and saved identities.
