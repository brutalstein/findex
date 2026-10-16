# Architecture (enterprise campaign view)

The module map, data flow and threat boundaries are in
[docs/architecture.md](../../../docs/architecture.md); the threat analysis is
in [THREAT_MODEL.md](THREAT_MODEL.md). This file lists what this campaign
changed at each boundary.

| Module | Change | Commit |
|---|---|---|
| `validation/harness` | `live_model_gate` before any `claude` launch; offline runner by default; `--max-sessions`; CI spawn test | `622f044` |
| `findex-index/context.rs` | per-language comment/string lexer (C-like with Rust/Swift/Kotlin/Scala nesting, `#` languages, prose); multi-line and raw strings, char literals | `cd2b397`, `7068bff`, `fbc73ad` |
| `findex-index/context.rs` | router: Turkish stems and more English forms, `file` intent (named file → its definitions or head), callers/impact without an identifier → concept | `fbc73ad` |
| `findex-index/context.rs` | budget counts the whole report | `d60e34b` |
| `findex-index/lib.rs` | schema 4: `secure_delete`, FTS5 `secure-delete`; newer schema refused; locked files kept in full reconciliation | `edd41bf` |
| `findex-index/refresh.rs` | locked known files kept; `RefreshReport.unreadable` | `edd41bf` |
| `findex-index/symbols.rs` | `definitions_in_file` for the file intent | `fbc73ad` |
| docs | README, SECURITY, architecture, doctor text | `01ff624` |

Unchanged: MCP tool set and descriptions, `findex-watch`, `findex-relations`,
`findex-syntax`, launcher, CI release pipeline. No new dependency;
`Cargo.lock` is byte-identical to the baseline.

Not built (planned phases): typed relationship graph (P1), relations schema
versioning, SCIP-backed context, semantic (embedding) retrieval.
