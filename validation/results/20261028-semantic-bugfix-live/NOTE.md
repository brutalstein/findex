# Primary subscription bugfix study

Exactly six new official Codex subscription starts were attempted, sequentially,
with `gpt-6.1-sol`, high reasoning and priority service. The user renewed live
testing; the announced cap was six. No paid APIs or subagents were used. The
previous 17 starts, including five blocked attempts, remain separate.

| Tool | Regression passes / starts | Model wall time, Requests / ripgrep | Preparation total |
|---|---:|---:|---:|
| Findex | 1/2 | 68.85 s / timeout at 300 s | 42.09 s |
| Graphify | 2/2 | 54.05 s / 43.18 s | 30.23 s |
| codebase-memory | 2/2 | 61.68 s / 55.59 s | 36.01 s |

The timeout produces no patch and has no complete final usage record. It remains
in the attempted-task denominator. **Findex does not win this primary bugfix
study.** Its two-task token usage is incomplete and must not be ranked against
two completed competitor sessions.

On the single fully matched Requests task:

| Tool | Total input + output | Cached input | Uncached input | Output |
|---|---:|---:|---:|---:|
| Findex | 191,315 | 160,768 | 28,809 | 1,738 |
| Graphify | 288,152 | 250,752 | 35,697 | 1,703 |
| codebase-memory | 340,850 | 295,040 | 44,267 | 1,543 |

Findex uses 33.6% / 43.9% fewer total tokens on this one task than Graphify /
codebase-memory, with equal regression outcome but longer model wall time.
Repeated cached input is included in totals; subscription quota weighting or
currency cost cannot be inferred. This is one public authored task, not a
general or statistically powered savings claim.

## Execution and grading

All arms use the same frozen source tree per task, unchanged public reproductions
and equivalent workspace-write MXC permissions. Models edit application source;
the independent native Windows host runs `repro.py` after source/fixture guards.
MXC private-temp permissions and MSVC linking were unsupported, so model prompts
explicitly delegate regression execution to the host. This does not claim
model-side test execution or a full upstream/hidden-test suite. Model wall time
excludes preparation and grading; each successful record includes grader time.

Findex/Rust additionally receives SCIP generated from the same buggy source by
rust-analyzer; its cost is included in preparation. Graphify uses code-only AST
extraction. Tool binaries, source trees, reproduction hashes, model profile and
ordering are frozen in `pilot-manifest.json`. The fish oracle accepts equivalent
single/double/unquoted long flags; red/green upstream checks were rerun before
any model starts. Read-only tool sets differ by each product's supported surface.

`patches/` publishes all five passing primary source diffs; `patches.json` binds
their hashes and source-only guard results. Three additional Requests behavior
controls pass for each Requests patch; these are labelled post-campaign,
exploratory checks and do not change primary scoring. Raw model streams stay
local under `raw/`.

## Timeout and separate diagnostic

The initial subprocess wrapper lost partial stdout/stderr on timeout. Its
original journal/command error is retained locally under `raw/`; the published
error is path-free. The capture wrapper now writes streams directly to files,
with a regression test for interrupted output preservation. No primary model
attempt is rerun or replaced.

Direct post-timeout native MCP probes return in 4–88 ms, but are not a replay of
the lost model trace and do not establish its cause. One additional diagnostic
session was explicitly approved afterwards; its successful 34.10-second result
is recorded in [a separate study](../20261028-semantic-bugfix-diagnostic/NOTE.md).
The timeout cause remains undetermined. These results do not guarantee broad
bugfix, quality, latency or cost superiority, nor production readiness.
