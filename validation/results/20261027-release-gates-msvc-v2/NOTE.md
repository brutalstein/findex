# Intermediate MSVC build

Stable and MSRV passed 150 tests on the matrix-only implementation, and the
bundled executable passed the Windows-system-only PATH check. The later zero-sync
format adds another regression and changes the release binary. Use the final
`../20261027-release-candidate/` run (151 Rust tests) for release decisions.
