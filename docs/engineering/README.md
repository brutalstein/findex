# Findex Engineering Audit Trail

This directory is the permanent, GitHub-visible engineering record for Findex
(https://github.com/brutalstein/findex). Its purpose is to allow any reviewer —
including another AI assistant with read-only access to the public repository —
to understand what was changed, why, which version was tested, which commands
were run, which errors occurred, how they were resolved, which tests passed or
failed, what performance changed, which claims have actual evidence, which
problems remain, and what should happen next.

## Discoverable entry points

| Document | Purpose |
|---|---|
| [`STATUS.md`](STATUS.md) | Concise project dashboard: current verdict, milestone table, health summary, next actions |
| [`WORKLOG.md`](WORKLOG.md) | Append-only chronological engineering history (confirmed bugs, fixes, failures, blockers) |
| [`ISSUES.md`](ISSUES.md) | Defect registry with priorities, root causes, reproduction steps, resolutions |
| [`BENCHMARKS.md`](BENCHMARKS.md) | Reproducible measurements: IDs, commits, datasets, environments, commands, before/after |
| [`DECISIONS.md`](DECISIONS.md) | Architecture and algorithm decisions (problem, alternatives, criteria, decision, tradeoffs) |
| [`HANDOFF.md`](HANDOFF.md) | Mandatory resumable agent state (branch, commit, working tree, next command, blockers) |
| [`RELEASE_READINESS.md`](RELEASE_READINESS.md) | Release readiness checklist with evidence-backed pass/fail marks |

## Related records

- [`docs/EVIDENCE.md`](../EVIDENCE.md) — General evidence conventions and validation layout
- [`validation/`](../../validation/) — Reproducible results, harness, golden datasets, manifests
- `docs/architecture.md` — Module boundaries and design
- `FINARCH.md` — Implementation plan and acceptance criteria
- `roadmap.md` — Milestone criteria and status

## Verifying claims

Every significant engineering claim in this directory must point to:

- A commit SHA (full or short, linkable on GitHub).
- A test command and its outcome.
- An evidence directory under `validation/results/` (manifest + structured output).
- A CI run result, when GitHub CI executed it.

Never treat an untested code change as verified. If validation has not run, the
status is `IMPLEMENTED_UNVERIFIED`, not `VERIFIED`.

## How to reproduce tests

On Windows (native-first platform):

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --release -p findex
```

## How to resume development

1. Read [`HANDOFF.md`](HANDOFF.md) for branch, commit, working-tree status, and
   the exact next command.
2. Read [`STATUS.md`](STATUS.md) for the current verdict and milestone table.
3. Read [`ISSUES.md`](ISSUES.md) for confirmed open defects.
4. Run the commands above to confirm baseline health before making changes.

## Branch

Latest engineering work lives on `main` unless `HANDOFF.md` states otherwise.
