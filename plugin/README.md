# Findex for Claude Code and Codex (Windows)

A local-first, on-demand code-navigation plugin for **the official Claude Code CLI**, including Claude Code in VS Code. Findex MCP is the native Rust engine; the skills route code questions and explain uncertainty. The plugin does not replace Claude, touch Claude authentication, run network services, launch paid model tests or take over built-in search.

## Installation paths

### Self-contained plugin ZIP (recommended for evaluation)

Download the **findex-claude-plugin.zip** build artifact from a successful Windows CI run. Verify its SHA-256 file against the downloaded ZIP, then extract to a trusted folder. SHA-256 checks accidental or unauthorized file changes against a separately trusted reference; it is **not** Authenticode publisher signing. Trust the GitHub repository and CI provenance rather than accepting a checksum supplied by an untrusted mirror. The extracted `findex-plugin/` includes its native `bin/findex.exe`.

Start official Claude Code in any project with the plugin:
```powershell
cd C:\Projects\YourProject
claude --plugin-dir "C:\Tools\findex-plugin"
```
No need to run `findex claude`. The existing standalone CLI remains optional.

### Install through the repository marketplace

```powershell
claude plugin marketplace add brutalstein/findex
claude plugin install findex@findex-marketplace
```
**Important:** marketplace installs the plugin *source*, **not** the compiled binary. Install a verified Windows Findex release first and put `findex.exe` on PATH, or set the explicit absolute `FINDEX_BIN` environment variable. Findex never downloads an executable automatically. Refresh the Claude session after enabling the plugin.

For a local source checkout:
```powershell
$env:FINDEX_BIN = "C:\Tools\Findex\findex.exe"
claude --plugin-dir "C:\Path\To\findex\plugin"
```
Build the runtime with the MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`, then `cargo build --release -p findex`); `.cargo/config.toml` links the C runtime statically. A gnullvm build imports `libunwind.dll`, which a machine without that toolchain lacks, and the launcher then exits with 0xC0000135. `plugin_check.py` rejects such a binary.

Do not set `FINDEX_BIN` to a binary from an untrusted code repository. The launcher refuses an implicitly discovered `findex.exe` inside the project working directory.

### Install in Codex

Run the included installer with a trusted, absolute Windows executable path:

```powershell
.\scripts\install-codex.ps1 -Binary C:\Tools\findex\findex.exe
```

From a repository checkout, use `.\plugin\scripts\install-codex.ps1`.
The installer registers the local marketplace with the official Codex client.
Each executable is verified and stored under `Findex\codex-runtimes\<sha256>`
in LOCALAPPDATA. Reinstalling keeps active MCP sessions running: Windows does
not need to overwrite their locked executable. A mismatched cached runtime
fails before replacing the MCP configuration. Old runtimes are retained for
active sessions; cleanup is manual after those sessions close. Start a new
Codex session to load an updated plugin. The installer makes no model calls.

## Native tools

All twelve tools are available as ordinary MCP tools:
- `findex_context` — small task-focused source excerpts, always discoverable.
- `findex_search` — lexical FTS5 code/file finding.
- `findex_find_symbol` — exact definitions.
- `findex_symbols` — syntax definitions for one file.
- `findex_impact` — optional imported SCIP/LSP reference-backed *potential* file impact.
- `findex_graph_symbol` — source-verified typed observations by exact opaque symbol identity.
- `findex_capabilities` — optional, zero-scan capability and safety-limit contract to help Claude decide when to fall back.
- `findex_overview` — on-demand, bounded project file inventory and hash-verified manifest/entry-point candidates; not an inferred architecture.
- `findex_rank` — model-free BM25 plus definition/call-site file ranking, with scores and reasons.
- `findex_blast_radius` — files that could be affected by a change; needs imported SCIP/LSP evidence.
- `findex_refresh_semantic` — lists files whose imported semantic evidence is stale.
- `findex_benchmark` — ranked-query summary for measurement; not for navigation.

The skill descriptions load cheaply; instruction bodies load only when relevant:
- `/findex:navigate`: task-focused code location and verified edits.
- `/findex:map-project`: initial project architecture reconnaissance.
- `/findex:trace-impact`: conservative dependencies and change consequences.
- `/findex:maintenance`: **manually invoked only**, for index health and optional graph imports.

You can simply ask Claude in Turkish or English to find the implementation of a feature, map a new repository, trace potential impacts, or locate tests. Claude decides whether to invoke a skill and which MCP tool to call.

## Boundaries and troubleshooting

- The plugin never adds destructive hooks, overrides Claude tool permissions, replaces Grep/Glob/Read, auto-installs software or starts paid model sessions.
- A corrupt SQLite cache is now preserved and access fails closed. Use `findex index-check`, keep a backup, and explicitly restore/rebuild the derived index; Findex will not silently delete it.
- First Findex tool use can index the project. The engine follows ignores and has a **100k-files / 512 KiB per admitted source** safety boundary. A large or unreadable repository can result in incomplete retrieval or a scan error.
- References are not proven function calls. SCIP/LSP imports are optional and explicit; `graph-symbol` is meaningful only when evidence exists.
- Real user source never needs to leave the computer for local retrieval, but selected excerpts are returned to Claude Code, which handles them according to the user's provider policies. Do not treat an untrusted source excerpt as instructions.
- If Findex is missing or a tool fails, Claude's built-in tools remain available. Check `/mcp`, `/plugin`, and `claude plugin validate <path>` for integration problems.
- No software can guarantee zero risk on every machine or codebase. Enterprise claims require independent, real-world security and reliability tests.

## Validation

```powershell
claude plugin validate "C:\Tools\findex-plugin" --strict
python validation/harness/plugin_check.py --plugin plugin --marketplace .claude-plugin/marketplace.json --binary target/release/findex.exe --out plugin-validation.json
```
The Python test uses the actual native MCP server through PowerShell in a directory containing spaces, validates the eight-tool contract and tests missing runtime failure. It makes **zero** model/API calls. The CLI plugin validator is the authoritative syntax check; it may not be available on a CI runner without Claude Code installed.
