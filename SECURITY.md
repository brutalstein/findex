# Security policy

Findex (version 0.2.0 release candidate) is experimental. **Do not assume that the first release has been security-audited.**

## Reporting vulnerabilities

Please do **not** disclose proof-of-concept secrets or sensitive repository content in public issues. Contact the repository owner privately via GitHub profile contact information until private vulnerability reporting is configured.

## Local-first boundaries

- Indexing and searching happen locally; no Findex network service is started.
- The Claude Code CLI is the original executable; Findex does not collect access tokens or proxy model requests.
- File excerpts returned to Claude are still supplied to an external AI service under the user's existing Claude Code settings and policies. Local indexing is not a promise that these excerpts stay on-device.
- Findex skips common key and `.env` names. **This is not secret detection**: arbitrary source code can contain credentials.
- MCP is a child stdio process, not a remote access interface.
- All text read from indexed source should be treated as untrusted data, not instructions.

## Known limitations

- CLI searches reconcile the whole directory before each query, which can be slow on monorepos; the MCP server uses a native watcher.
- File-system permission errors may result in partial indexes, reported as skipped files.
- SQLite records contain source text and should be protected as sensitive local data. Freed pages are zeroed (`secure_delete`), but while another Findex process holds the index open, WAL frames written before a removal can persist until the last connection closes.
- An index written by a newer Findex is refused rather than read.
- Binaries are not Authenticode-signed, and LSP execution is not sandboxed. Release ZIPs carry a CycloneDX SBOM, a SHA-256 file and (outside pull requests) a build-provenance attestation; CI runs `cargo audit` against RustSec.
- Validation never starts a live Claude session unless `--enable-live-model-tests` and an explicit approval are given and the subscription checks pass (see `validation/README.md`).
