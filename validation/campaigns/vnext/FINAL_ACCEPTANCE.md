# Final acceptance — Findex vNext

**RELEASE_READY = FALSE**

## Blockers (exact)

1. **Claude Code A/B/C/D/E study not completed** (spec: "run real A/B/C/D
   studies"). 19 sessions measured before the plan session limit; the rerun
   (`20261012T100733Z-96295ee75261`) failed sign-in on every session
   ("OAuth session expired and could not be refreshed"). Token and
   task-success claims for `findex_context` are therefore unproven.
   *Unblock:* the account owner signs in again, then
   `validation/scripts/run-claude-ab.ps1 -Reps 3 -Arms 'A,B,C,D,E'`.
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

CI: see the pull request for this branch (`vnext/context-engine` → `main`).

## What can be claimed

- `findex_context` answers navigation tasks in one call, loads without a
  `ToolSearch` turn, and ranks the right file higher than the previous
  `findex search` on held-out real issues.
- Its call-site evidence is reliable for uniquely named functions and
  explicitly flagged as unreliable for shared names.

## What cannot be claimed

- Token savings or higher task success with Claude Code.
- Any result for models other than Haiku 4.5.
- Type-resolved call graphs.
