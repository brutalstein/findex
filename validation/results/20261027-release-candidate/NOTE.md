# Final local Windows preview gates — 2026-10-27

MSVC/static-CRT binary; stable 1.97.0 and declared MSRV 1.85.0 each pass 151
all-target Rust tests. Format, Clippy, optimized build, RustSec and cargo-deny
policy pass. Offline Python suite passes 33 tests. `offline-checks.json` and
component JSON retain security, corrupt-cache/privacy, Cloud Files, loopback
SMB, 10k-file stress and plugin checks; `codex-integration.json` proves the
installed final plugin's eight tools, source anchor and zero-sync format without
a model call. Raw logs are local/ignored; commands and result identities are kept.

The source/binary identities and file checksums connect these results with the
local package. Current-commit CI, signing, Windows 10, real remote SMB, broader
live code-change studies and universal superiority are **not** claimed as done.
No tag or release was published. See the full bounded review and reproduction:
`docs/engineering/RELEASE_CANDIDATE_HARDENING_20261027.md`.
