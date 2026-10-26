# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions
follow [Semantic Versioning](https://semver.org/); before 1.0, a minor version
may change CLI output, MCP tool output and the index schema. An index written
by a newer Findex is refused, not misread; `findex forget` rebuilds it.
Measured claims and their limits are listed in [docs/EVIDENCE.md](docs/EVIDENCE.md).

## [0.2.0] — unreleased

First release candidate. No earlier version was published.

### Added
- `findex_context` / `findex context`: one call returns a name's definition,
  call sites with their enclosing function, or its tests, with a coverage line
  checked against `git grep`. Intent routing in English and Turkish.
- Owner-qualified call sites (`Type::method`, `self.method`, generics), with
  external-crate exclusion for Rust.
- Native Claude Code plugin (marketplace, skills, bundled MCP runtime).
- Codex plugin: the same MCP server and skills (`plugin/.codex-plugin`,
  `plugin/scripts/install-codex.ps1`).
- Persistent Tree-sitter definitions (Rust, Python, TypeScript/JavaScript, C++),
  SCIP and captured-LSP reference imports, bounded file impact.
- Watcher-backed MCP server (ReadDirectoryChangesW) with safe fallbacks.
- `findex forget`: deletes this project's cached indexes; imported SCIP/LSP
  evidence must be imported again.
- Release ZIPs with a CycloneDX SBOM, SHA-256 files and build-provenance
  attestation.

### Fixed
- Cloud-only OneDrive (Cloud Files) placeholders are no longer read, so
  indexing no longer downloads them. `findex index` reports how many were left
  out.
- MCP tool errors no longer contain engine text or local paths.
- Removed, excluded or access-denied file text no longer survives in SQLite
  free pages, FTS5 segments or the WAL.
- A corrupt or newer-schema index fails closed instead of being rebuilt or
  misread.

### Security
- Offline-by-default validation; live model tests need explicit approval and
  bill only the Claude subscription.
- No symlink or junction following; sensitive file names excluded from
  indexing, reads and imports.

### Known limitations
- Binaries are not Authenticode-signed; certificate signing is not configured.
- Call sites outside Rust are name-based, not type-resolved.
- Cloud-only files are not searchable until made available offline.
- Tested on Windows 11 and Windows Server CI images; not on Windows 10.
