# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions
follow [Semantic Versioning](https://semver.org/); before 1.0, a minor version
may change CLI output, MCP tool output and the index schema. An index written
by a newer Findex is refused, not misread; `findex forget` rebuilds it.
Measured claims and their limits are listed in [docs/EVIDENCE.md](docs/EVIDENCE.md).

## [0.2.0] — unreleased

First release candidate. No earlier version was published.

### Performance and installation
- Bound native source reads to at most eight workers while keeping SQLite
  writes ordered and transactional; recheck cloud-only, size and link status
  immediately before each read. Fall back to serial reads if workers cannot start.
- Rank and limit full-text candidates before fetching their bodies in the same
  SQLite read snapshot; retain ranking, source verification and truncation flags.
- Install Codex runtimes by verified content hash so active Windows MCP sessions
  do not block upgrades; reject a corrupt runtime before changing configuration.
- Record native 200–100,000-file scaling and wide-source query measurements,
  with source-equivalence, idle-resource and over-budget rollback assertions.
- Serialize `bm25_score` to three decimals in MCP and CLI JSON output. Ordering
  and hits are unchanged; the 17-digit tail cost about six model tokens per hit
  (mean raw tokens per query on the 115 frozen lookups: 263.8 to 243.0).

- Natural-language (concept) ranking demotes vendored copies (×0.5) and
  docs/examples/support scripts (×0.7) on the BM25 score, so project source
  outranks them on comparable matches. Issue localization (65 tasks): hit@1
  0.446 to 0.462, recall@10 0.808 to 0.823; five tasks improved, none worsened.
  A nested `packages` directory counts as vendored; a top-level one does not.
- Concept ranking also relaxes FTS5's fixed BM25 length normalisation (b 0.75
  to 0.3), so large central files are not penalised for their size. Together
  with the path weights: hit@1 0.446 to 0.538, recall@10 0.808 to 0.869 on 65
  issues (held-out half 0.464 to 0.536); 19 tasks improved, 5 worsened by one
  to three ranks.
- Concept ranking scores a file higher when query words name its definitions
  (symbol-name field, weight 1). hit@1 0.538 to 0.585 on 65 issues (held-out
  half 0.536 to 0.607); 11 tasks improved, 2 worsened by one rank. Measured on
  the same tasks, Graphify reaches 0.123 and codebase-memory 0.246 hit@1 from the
  raw issue title. Targets 0.60 overall / 0.90 recall@10 are not yet met.
- A task that names an identifier and adds at least two descriptive words (a problem report such as
  `MissingSchema error raised when schema present`) now also runs the concept search: identifier
  evidence stays first, `ranked` lists definition files first, and the concept matches fill the remaining
  budget. Bare lookups and "where ... defined" questions are unchanged. On the 65 development tasks this
  removes four cases where the gold file was missing from the output. Fresh 55-issue holdout: gold
  recall@10 0.706 to 0.761, hit@10 0.745 to 0.800, hit@3 0.691 to 0.727, hit@1 0.364 to 0.345; the
  60% / 90% targets remain unmet (FX-DEC-20261030-004).
- Concept-task text output opens with `ranked files, most likely first: ...`
  (five files, rank order); packing groups snippets by kind, so the rank was
  not visible. The JSON report carries the same list in a `ranked` field. In a live Claude Haiku study on eight
  held-out issues, Findex-only sessions went from 4/8 to 6/8 correct and from
  420k to 143k tokens each (same issues the change was designed on; not an
  independent test).

### Added
- Optional lossless `format="compact"` for MCP lexical and exact-symbol results;
  the existing JSON object format remains the default.
- Exact-symbol `truncated` diagnostic for unexamined candidates, including
  stale candidates that exhaust the bounded verification window.
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
- Windows MSVC packages link the CRT statically and are tested with only
  Windows system directories on PATH, exposing hidden compiler-DLL dependencies.
- Excerpt redaction now precedes truncation, preventing a quoted credential
  from escaping masking when its closing quote crosses the excerpt boundary.
- Evaluation guards freeze corpus/tool identities, count interrupted live
  attempts, require successful MCP evidence and correct nDCG duplicate ranks.
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
  verified Claude or Codex subscription login. API billing routes are blocked.
- No symlink or junction following; sensitive file names excluded from
  indexing, reads and imports.

### Known limitations
- Binaries are not Authenticode-signed; certificate signing is not configured.
- Call sites outside Rust are name-based, not type-resolved.
- Cloud-only files are not searchable until made available offline.
- Tested on Windows 11 and Windows Server CI images; not on Windows 10.
