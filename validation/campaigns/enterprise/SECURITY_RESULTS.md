# Security results

## Black-box suite at `01ff624` (`results/20261016T172700Z-01ff6241d30e/security`)

`validation/harness/security.py` against the release binary: **30 PASS,
0 FAIL, 0 SKIPPED** (same 30 checks as the baseline run at `fb1b000`).
Covers traversal and Windows path forms through MCP (`..`, absolute,
`\\?\`, UNC, ADS `:stream`, `CON`, NUL byte), a file symlink to outside,
`.env` exclusion, excerpt redaction, MCP frame/JSON/nesting/batch/version
handling, no sockets opened, SCIP/LSP fuzzing (200 random SCIP payloads,
mutated LSP captures), corrupt-DB rebuild and kill -9 during indexing at
0.2/0.6/1.5 s. NTFS junctions, long paths and Unicode names are covered by
`crates/findex-index/tests/windows_fs.rs`.

## Changes in this campaign

| Finding | Change | Test | Status |
|---|---|---|---|
| A10 default runner could start paid sessions | offline by default, fail-closed gate | `test_live_gate.py` (11, in CI) | PASS |
| A9 environment probe ran `claude` | probe removed | same | PASS |
| A4 removed text recoverable from cache | `secure_delete`, FTS5 `secure-delete` | `removed_file_text_is_not_left_in_index_bytes` (reads DB bytes) | PASS for the main DB file; the test does not discriminate the FTS5 option (passes with the pragma alone); WAL frames can outlive a removal while another process holds the DB open (open) |
| A5 older binary reads newer schema | refuse `user_version > 4` | `newer_index_schema_is_refused_and_left_intact` | PASS |
| A6 locked file dropped from index | kept, still hash-verified on read | `locked_file_keeps_its_index_entry`, `exclusively_locked_file_is_kept_but_never_served_unverified` (Windows share-mode 0) | PASS |
| A1 code lines hidden as comments while claiming full coverage | per-language lexer | coverage check 279/279 | PASS |

## Prompt-injection boundary

Structural only: evidence text is indented so repository content cannot
forge `missing:`/`coverage`/`shown:` lines (`repository_text_cannot_forge_report_lines`),
and the MCP server instructions mark returned source as untrusted. Whether a
model obeys injected instructions was not tested (no live sessions):
UNVERIFIED.

## Open

- A11: MCP error texts can include absolute local paths.
- A12: index directory ACL not verified by a test.
- No Authenticode signature; no certificate purchased.
- `cargo audit` runs in CI on every push; it was not re-run locally here.
