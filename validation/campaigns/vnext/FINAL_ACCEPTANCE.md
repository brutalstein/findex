# Final acceptance — Findex vNext

**RELEASE_READY = FALSE** for the full vNext specification.

The context engine (`findex_context`, coverage line, trust rule) meets its own
acceptance evidence and can be merged as scoped work; the decision is FALSE
because spec items below were not built or not exercised.

## Blockers (exact)

1. **Typed multi-language relationship graph not built; cross-language edge
   precision unmeasured** (spec P2). Only Rust has a compiler oracle
   (rust-analyzer SCIP); other languages use name-based call sites.
2. **Windows reliability items not exercised**: Windows 10, SMB/network
   volumes, OneDrive cloud-only (dehydrated) placeholders
   (REMAINING_LIMITATIONS #8 partly, #16). Mass branch switch under the
   watcher and a locally present OneDrive folder now pass (#8, #9).
3. **Claude benefit is shown for one model and one task class.** Run 4 used
   Haiku 4.5 only. Savings are demonstrated for navigation tasks
   (definitions, callers); not for concept questions (t3: +14.0k tokens),
   cross-file traces or code changes.

Resolved since the previous decision (`397bf78`): the Run 3 cost (+31 %
tokens) is gone. Run 4 (`results/20261016T105620Z-ac4ea21dd692`, 108/108
sessions) — D vs built-ins only: held-out −23.7 % tokens, −15.1k per session
[−21.4k, −8.7k], −1.5 turns [−2.67, −0.58], 12/12 correct; all 12 tasks
−15.9 % [−31.2k, −4.0k], 36/36 correct.

## Gates that pass (native Windows 11, this machine)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace --all-targets` | **109 passed, 0 failed, 0 ignored** |
| `cargo build --release -p findex` | pass |
| Black-box security suite (at `fb1b000`) | 30/30 PASS |
| `cargo audit` | 0 vulnerabilities |
| MCP backward compatibility (4 original tools unchanged, `findex_search` first) | MCP tests pass |
| Symbol retrieval regression, 642 queries (at `fb1b000`) | unchanged: R@1 0.984, MRR 0.991, validity 1.000 |
| Coverage claim vs `git grep -w`, 7 corpora | 279/279 names hold |
| Held-out issue localization: `findex_context` vs `findex search` | MRR +0.169 [+0.083, +0.263]; hit@1 +0.179 [+0.036, +0.321]; hit@10 −0.036 [−0.143, +0.071] |
| Call sites vs rust-analyzer SCIP (unique names), tokio held-out | call precision 0.787, site recall 1.000 |
| Branch switch 14.1.1 ↔ 13.0.0 under running MCP watcher (148 files) | pass, correct within ~1.1 s |
| Index of a locally present OneDrive folder | pass, 0 stale results |
| Cold-index peak memory regression (#17) | root-caused (transient, bounded) |
| Relations DB concurrent open (#6) | reproduced and fixed |

CI: PR brutalstein/findex#25 — see the PR checks for the latest pushed commit.

## What can be claimed

- `findex_context` returns definitions and call sites with enclosing
  functions in one call, and ranks the right file higher than the previous
  `findex search` on held-out real issues.
- When a report is not truncated, its coverage line is exhaustive for that
  name in the indexed files (verified 279/279 against `git grep -w`).
- With Haiku 4.5 on navigation tasks, the Findex arm uses fewer tokens and
  turns than built-ins only, with equal correctness, including on held-out
  tasks in C++, Python and TypeScript.

## What cannot be claimed

- Token savings for concept questions, multi-file traces or code changes.
- Any result for models other than Haiku 4.5.
- Type-resolved or cross-language call graphs.
- Behaviour on Windows 10, network volumes or cloud-only OneDrive files.
