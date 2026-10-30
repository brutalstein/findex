# Portable plugin runtime check (2026-10-30)

Runtime: `target/msvc-stable/release/findex.exe` built with the MSVC toolchain (static CRT from `.cargo/config.toml`).
SHA-256: `61dd3cc3d5069e3085c7c84e0b35092ae93feb6b383aa8facd588bb030ce2997`
Non-API-set DLL imports: bcryptprimitives.dll, kernel32.dll, ntdll.dll, userenv.dll

- `plugin-check.json`: `plugin_check.py` on a trimmed System32-only PATH (PASS).
- `codex-integration.json`: `install-codex.ps1` into an isolated CODEX_HOME, then native Codex app-server loaded the plugin, listed 12 tools and returned fixture source (PASS, zero model calls).
- `claude-integration.json`: `claude_integration.py --enable-live-model-tests --model haiku` (subscription sign-in, no API key): MCP config, `--plugin-dir` and `findex claude` launcher sessions all connected, called `findex_find_symbol` and answered `crates/findex-watch/src/lib.rs:91` (PASS).

The previous failure: a gnullvm-built `findex.exe` imports `libunwind.dll` (present only on the build machine), so the
plugin launcher exited 3221225781 (0xC0000135, STATUS_DLL_NOT_FOUND) on a machine without that toolchain.
`plugin_check.py` now rejects any runtime that imports a DLL missing from System32.
