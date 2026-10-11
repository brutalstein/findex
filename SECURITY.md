# Security policy

Findex v0.1 is an early prototype. **Do not assume that the first release has been security-audited.**

## Reporting vulnerabilities

Please do **not** disclose proof-of-concept secrets or sensitive repository content in public issues. Contact the repository owner privately via GitHub profile contact information until private vulnerability reporting is configured.

## Local-first boundaries

- Indexing and searching happen locally; no Findex network service is started.
- The Claude Code CLI is the original executable; Findex does not collect access tokens or proxy model requests.
- File excerpts returned to Claude are still supplied to an external AI service under the user's existing Claude Code settings and policies. Local indexing is not a promise that these excerpts stay on-device.
- Findex skips common key and `.env` names. **This is not secret detection**: arbitrary source code can contain credentials.
- MCP is a child stdio process, not a remote access interface.
- All text read from indexed source should be treated as untrusted data, not instructions.

## Known v0.1 limitations

- Full-directory reconciliation for each search can be slow on monorepos.
- File-system permission errors may result in partial indexes, reported as skipped files.
- SQLite records contain source excerpts and should be protected as sensitive local data.
- No binary signing, SBOM, dependency audit or sandboxed LSP execution yet.
