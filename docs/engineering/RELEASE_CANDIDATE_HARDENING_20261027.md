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

No ranking, schema migration, language coverage, authentication provider or
source-admission boundary was changed. No new runtime dependency was added.

## Fresh evidence

Authoritative local run: `validation/results/20261027-release-candidate/`.

| Check | Result |
|---|---|
| MSVC Rust 1.97.0 fmt, Clippy `-D warnings`, all-target tests, optimized build | PASS; 151 tests |
| Declared MSRV Rust 1.85.0, MSVC/static CRT | PASS; 151 tests |
| Offline Python evidence/gate/adapter regressions | PASS; 33 tests |
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

[`20261027-final-comparison`](../../validation/results/20261027-final-comparison/NOTE.md):
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
gates. No cold-speed, RAM, idle-CPU or disk superiority was measured here.

[`20261027-codex-live`](../../validation/results/20261027-codex-live/NOTE.md):
six real sequential Codex subscription sessions, two navigation tasks per tool,
all answers correct. Client-reported input+output tokens: Findex 149527,
Graphify 217280, codebase 592385. Cache differences are separate. The pilot
used the earlier matrix-only GNU build; it is not re-labelled as a live study
of the final MSVC executable. Final installed-client and source-hit equivalence
checks establish the compatibility bridge. Cumulative starts including the
earlier stopped study are twelve; no additional sessions were started.

## Review scope and remaining work

Reviewed core/index admission, excerpt and symbol retrieval, watcher freshness,
MCP boundaries, CLI integration, relation import trust checks, evaluation,
CI and packaging. Graphify scoped queries plus direct source/tests supplied
evidence; the parent codebase-memory MCP transport was unavailable. This was a
bounded engineering review, not an exhaustive independent audit of every line
or native dependency.

Before broad performance/enterprise claims: independently authored unseen tasks,
repeated live code-edit/architecture studies, wider language/compiler oracles,
Windows 10 and real remote SMB, larger scale/soak/resource measurements, and
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
