# Final local native release checks

PASS: 153 Rust tests on MSVC stable 1.97 and MSRV 1.85; 34 offline Python tests; fmt, Clippy, optimized build, RustSec and cargo-deny; privacy, corrupt-cache, overview, Cloud Files (13 local plus 3 placeholders, no Findex downloads), loopback SMB, 10k mutation/concurrent-writer stress, Claude strict validator, Windows-system-only PATH plugin check and native Codex installed eight-tool source evidence. No model/API calls.

Initial security run stopped on missing corpus under an isolated validation root. The failure is retained in commands.jsonl; security-corrected/security.json passes 29 checks after explicitly selecting pinned target/tool-comparison/corpora. MSRV/Python raw count logs remain local/ignored. Source identity hashes link final runtime/harness sources with the binary and package. CI on this final commit, signing, Windows 10 and real remote SMB remain unverified. No tag/release published.
