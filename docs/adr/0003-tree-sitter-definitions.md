# ADR-0003: Bounded on-demand Tree-sitter definition extraction

**Status:** experimental, first language-aware slice.

Use the official Tree-sitter Rust parser and published grammar crates.
Start with Rust, Python, TypeScript/TSX and C++ (JavaScript-family syntax is
parsed by the TypeScript grammar). This is definition discovery only:
`start_line` and `end_line` are one-based source ranges, and `kind` is
syntactic, not an LSP or compiler-derived semantic fact.

Important boundaries:

- No AST model network calls; read an explicit file under the canonical root.
- Reject absolute and traversal paths, symlinked final files, oversized
  files, binary files, non-UTF-8 source and common credential filenames.
- Maximum 512 KiB source and 2048 reported definitions per request.
- Return syntax-error and truncation flags. Unknown extensions are supported
  by lexical fallback, not made-up language results.
- Use deterministic source-order output; avoid deep recursion in traversal.
- No full-workspace AST pass on every query. Persisted content-hash-keyed
  symbol indexing and cross-file graphs require separate acceptance tests.
- One file can have syntax errors but still yield useful definitions.
- Test four language fixtures, partial syntax and path permissions on Windows.

Grammar sources: <https://tree-sitter.github.io/tree-sitter/>.
