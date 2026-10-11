# ADR-0005: Locked Rust resolution and Windows provenance

**Status:** Windows CI-enforced; binary bit-for-bit reproducibility and signing
are not yet demonstrated.

## Requirements

- Track the **Cargo-generated**, SHA-256-verifiable `Cargo.lock` in Git;
  never edit package hashes or dependency entries manually.
- Keep workspace MSRV `rust-version = "1.85"` and use Cargo's rust-version-aware
  dependency fallback. A separate Windows job compiles **all workspace targets**
  with Rust **1.85.0**, not just current stable.
- Run `cargo metadata --locked`, Clippy, all tests and Release builds with
  `--locked`. Manifest changes must deliberately update the lockfile.
- Build Windows native x64 with the existing official CLI unchanged.
- Include a copy of `Cargo.lock` and the observed Rust and Cargo versions with
  the distributable, and publish an SHA-256 digest of the final ZIP artifact.
- Continue to pin GitHub Actions to reviewed immutable commit SHAs.

## Limitations

A lockfile fixes resolved package versions; it does **not** guarantee
bit-identical EXEs across compilers, environment variables, paths, generated
timestamps, or native build-tool versions. A SHA-256 digest detects changes
to the artifact but is **not** a signature, supply-chain attestation or SBOM.
No external service, binary download helper, source upload or credential
interception is introduced.

## Dependency policy

- Dependabot PRs are proposals, not automatic merges. The 1.85 job must
  pass on new resolutions or the project must explicitly change its MSRV.
- Windows CI tests behavior with the locked dependency graph.
- Follow up with SBOM, license audits, security advisory checks, pinned
  compiler/toolchain and signed release provenance. Monitor upstream
  vulnerability advisories; do not treat passing unit tests as a security audit.

References:
- https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html
- https://doc.rust-lang.org/cargo/reference/resolver.html
- https://doc.rust-lang.org/cargo/commands/cargo-generate-lockfile.html

## MSRV finding

The initial Windows Rust 1.85 CI test rejected `ignore 0.4.30`, which
uses let-chain syntax not accepted by Rust 1.85. The scanner dependency is
therefore constrained to the compatible 0.4.25 release and validated by
explicit MSRV tests. Any future upgrade must preserve this minimum or
update the advertised toolchain support explicitly.
