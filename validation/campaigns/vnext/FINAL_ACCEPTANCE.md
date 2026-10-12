# Final acceptance — Findex vNext

**RELEASE_READY = FALSE**

## Blockers (exact)

1. **The Claude Code study shows no benefit; it shows a cost.** Run 3
   (`20261012T105254Z-397bf78730ba`, 120/120 sessions, Haiku 4.5): the
   `findex_context` arm used +22.7k tokens per session vs built-ins only
   (+31.1 %, 95 % CI [+8.7k, +37.6k]) and +1.33 turns, with 23/24 vs 24/24
   correct. The model called Findex but then re-verified with Grep. The
   product goal "make Claude use Findex beneficially" is not met for this
   model.
2. **Typed multi-language relationship graph not built; cross-language edge
   precision unmeasured** (spec P2). Only Rust has a compiler oracle.
3. **Windows reliability items not exercised**: OneDrive/SMB volumes, mass
   branch switch under the watcher, Windows 10 (REMAINING_LIMITATIONS #8, #9,
   #16).

## Gates that pass (native Windows, this machine)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace --all-targets` | **107 passed, 0 failed, 0 ignored** |
| `cargo build --release -p findex` | pass |
| Black-box security suite | 30/30 PASS |
| `cargo audit` | 0 vulnerabilities |
| MCP backward compatibility (4 original tools unchanged, `findex_search` first) | MCP tests pass |
| Held-out issue localization: `findex_context` vs `findex search` | MRR +0.169 [+0.083, +0.263]; hit@1 +0.179 [+0.036, +0.321]; hit@10 −0.036 [−0.143, +0.071] |
| Call sites vs rust-analyzer SCIP (unique names), tokio held-out | call precision 0.787, site recall 1.000 |
| Cold-index peak memory regression (#17) | root-caused (transient, bounded) |
| Relations DB concurrent open (#6) | reproduced and fixed |

CI: PR brutalstein/findex#25, run 38237215005 — Format/Clippy/Tests/Release
Build, RustSec audit and Rust 1.85 minimum toolchain all pass (at `fc6c4e7`).

## What can be claimed

- `findex_context` returns definitions and call sites with enclosing
  functions in one call, and ranks the right file higher than the previous
  `findex search` on held-out real issues.
- Its call-site evidence is reliable for uniquely named functions and
  explicitly flagged as unreliable for shared names.

## What cannot be claimed

- Token savings or higher task success with Claude Code. For Haiku 4.5 the
  measured effect is the opposite (+31 % tokens).
- Any result for models other than Haiku 4.5.
- Type-resolved call graphs.
