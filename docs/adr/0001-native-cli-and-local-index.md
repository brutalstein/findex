# ADR-0001: Official Claude CLI + Local Rust Engine

Status: Accepted for initial source scaffold (not yet Windows-CI validated)
Date: 2026-10-11

## Context

Findex must improve Claude Code project navigation without replacing the official client or requiring a separate API subscription. It must remain native on Windows and function without a remote Findex service.

## Decision

Use a Rust multi-crate workspace and invoke unchanged official `claude.exe` with inherited console and forwarded user arguments. Supply Findex retrieval through a sidecar Claude Code plugin and stdio MCP. Keep data in a per-user local SQLite index with explicit validity checks. Use lexical BM25 as a measurable baseline before adding structural/semantic algorithms.

## Rejected alternatives

- Re-implementing Claude Code's terminal: duplication of UI, auth, permissions and breaking changes.
- API-token proxy or client impersonation: privacy, policy and maintainability risks.
- Mandatory local LLM/embeddings: expensive on student laptops and lacks proven incremental gain.
- Shipping a single monolithic core before measuring: makes debugging and precision evaluation harder.

## Consequences

Early Findex behavior is restricted to lexical retrieval; complex cross-file reasoning still belongs to Claude until Findex's graph layer exists. Plugin capability and Windows native CLI compatibility require integration tests. Index scanning cost is currently O(total selected source bytes) per query. This is knowingly a *bootstrap* performance limitation.

## Path safety follow-up

Resolve `claude.exe` explicitly from absolute `PATH` directories; never rely on Windows process search order that may execute a project-local file called `claude.exe`. Refuse executables inside the indexed project. This is a basic shadowing defense, **not** a code-signature/Authenticode verification; signature trust checks belong to the production hardening milestone.
