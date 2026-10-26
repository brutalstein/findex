# Threat model

Scope: one local Windows user running Findex next to the official Claude Code
client. Out of scope: multi-user servers, remote MCP transports, hostile
local administrators.

## Assets

1. Source text of the user's repositories, including secrets that happen to
   be in source files.
2. The index cache (`%LOCALAPPDATA%\Findex\indexes\<blake3(root)>`): raw
   source text, FTS5 segments, symbol tables.
3. The user's Claude Code installation, settings and subscription
   credentials (Findex must never read or use them).
4. The model context: excerpts returned by MCP tools.
5. The operator's money: subscription usage and any paid API.

## Trust boundaries and threats

| # | Boundary | Threat | Mitigation | Evidence | Status |
|---|---|---|---|---|---|
| T1 | repository → index | Path traversal, symlink/junction escape, secret-named files | `Component::Normal` keys, no symlink following, canonical-root check, sensitive-name filter | security suite (traversal, UNC, ADS, device names, symlink) and `tests/windows_fs.rs` (junction, long paths) | PASS (30/30 at `01ff624`) |
| T2 | repository → model | Prompt injection in source text | Evidence lines indented so file text cannot pose as report lines; server instructions mark source as untrusted data | `context.rs` test `repository_text_cannot_forge_report_lines`, security suite | PASS (structural); model obedience UNVERIFIED |
| T3 | repository → model | Credentials in excerpts | `redact_secrets` on returned text; not a secret scanner | security suite redaction checks | PASS for the listed shapes; other shapes not covered |
| T4 | index at rest | Removed/excluded file text recoverable from free pages | `secure_delete=ON`, FTS5 `secure-delete` | `removed_file_text_is_not_left_in_index_bytes`, `removed_file_text_is_not_left_in_wal_while_another_connection_is_open` (`PRAGMA wal_checkpoint(TRUNCATE)` after every sync that changed rows) | PASS |
| T5 | index at rest | Index directory readable by other local accounts | Inherits `%LOCALAPPDATA%` ACL (per-user) | `findex-core` `index_directory_is_not_readable_by_other_local_accounts`: no allow-read ACE for Everyone, Anonymous, Authenticated Users, Users or Guests; the probe proves it sees a granted Everyone ACE | PASS (Windows, inherited per-user profile ACL) |
| T6 | index version | Older binary misreads a newer schema | `user_version > LEXICAL_SCHEMA` refused, file untouched | `newer_index_schema_is_refused_and_left_intact` | PASS (applies to binaries from `edd41bf` on) |
| T7 | MCP stdio | Oversized/invalid frames, protocol confusion | 1 MiB frame cap while reading, JSON-RPC errors, stdout JSON only | MCP tests, security suite | PASS |
| T8 | MCP errors | Absolute local paths in error text | Tool errors return a fixed text per lookup, never the engine error (`72a07b6`) | `findex-mcp` `engine_errors_never_reach_tool_results` (path and SQLite text injected into every lookup; mutation-checked) | PASS |
| T9 | Findex ↔ Claude Code | Credential access, client emulation | Launcher runs official `claude.exe` only; never reads credentials | launcher tests, code review | PASS |
| T10 | validation → billing | Unapproved live model sessions or paid API use | Offline by default; gate needs flag + approval string + no CI + no billing env vars (the first-party endpoint excepted) + a claude.ai subscription login; sessions load project/local settings only, inherit no billing variables, and stop the study unless their init event reports `apiKeySource: "none"`; CI asserts no spawn | `test_live_gate.py` (14 tests, in CI); live canary `results/20261026-live-canary/` | PASS |
| T11 | imported evidence | Malicious SCIP / LSP captures | Size/count caps, hash-verified admission | fuzz: 200 SCIP payloads, mutated LSP captures | PASS |
| T12 | crash | Kill during indexing corrupts cache | Single transaction; a corrupt index is refused and preserved (fail closed), rebuilt only after the cache is deleted | kill -9 at 0.2/0.6/1.5 s | PASS |

## Not defended

- A local process running as the same user can read the index and source.
- Excerpts sent to Claude leave the machine under the user's Claude Code
  settings; Findex cannot keep them local.
- Binaries are not Authenticode-signed (no certificate was purchased).
