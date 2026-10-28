# Renewed subscription bugfix pilot

Six starts, two reused public tasks × three tools × one repetition. All six
produce source-only patches passing the independent native Windows regression.
No timeout, retry, paid API or subagent occurs in this campaign. This does not
explain or replace the timeout in the previous primary campaign.

The user requested a plan, implementation, then renewed live testing. The
announced six-session cap is respected. Source coding and native gates finish
before live starts. The frozen candidate is commit `3a41999`, binary SHA-256
`f70b0834d7742ae211b5ef08252a13de3c1af4a0a2e70cc7800e1403454a5833`.
Official Codex uses ChatGPT subscription login and the unchanged native
`gpt-6.1-sol` / high / priority profile. Earlier 24 starts remain separate.

## Results

| Tool | Passed / starts | Total input + output | Cached input | Uncached input | Output | Model seconds | Setup seconds | Grader seconds |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Findex | 2/2 | 494,954 | 431,360 | 60,608 | 2,986 | 80.951 | 17.828 | 3.197 |
| Graphify | 2/2 | 723,857 | 638,336 | 82,816 | 2,705 | 71.073 | 29.736 | 3.272 |
| codebase-memory | 2/2 | 419,689 | 351,616 | 65,033 | 3,040 | 70.904 | 29.708 | 3.424 |

Findex uses **31.6% fewer total tokens than Graphify**, but **17.9% more than
codebase-memory**. Findex's uncached input is 6.8% below codebase-memory; that
does not cancel its higher total count or determine subscription quota/currency
cost. It is slower during the model sessions than both alternatives. Its lower
setup time produces similar component totals (setup + model + grader: about
102.0 / 104.1 / 104.0 seconds respectively); this small difference on two tasks
is not a general latency win.

Findex has three completed MCP calls and 16 native commands; Graphify three and
10; codebase-memory nine and 10. Command totals mix source reads, editing and
verification. They indicate where to investigate follow-up work, not proof that
every additional command is redundant or the cause of the timing difference.
Per-arm counts, time and token comparisons are in `analysis.json`.

## Frozen protocol and limits

- Fresh red/green upstream-fix oracles pass before models start. Equal source
  tree and reproduction hashes, model profile, executables and tool surfaces
  are frozen in `pilot-manifest.json`.
- Semantic preparation is explicitly **not requested**. Ordinary Findex task
  navigation is lexical/syntactic. The earlier Rust SCIP preparation cost is
  removed by omitting that work, not by claiming equivalent semantics for free.
  Compiler-backed lookup still has its own native fixture evidence elsewhere.
- Models edit within native workspace-write MXC. Preflight verifies writes,
  blocked outside writes and the selected MCP tools without any model call.
- MXC private-temp/MSVC linking is unsupported. All arms explicitly leave
  reproduction execution to the independent native Windows host grader.
  These are public supplied regressions, not model-run/full upstream/hidden suites.
- No fixture, lockfile, baseline metadata or out-of-scope file may change.
  `patches/` publishes all six passing source diffs; `patches.json` binds their
  hashes and guard results. Raw streams remain local under ignored `raw/`.
- Three extra Requests behavior checks pass per patch (nine assertions), reused
  post-campaign exploratory controls. They do not change primary scoring.
- Stream diagnostics preserve pending MCP call IDs, last observed phase,
  completed/error counts and usage availability without copying source or
  arguments. All six end with complete usage and no pending MCP call.
- These are known public tasks with one repetition and differing cache states.
  No blind/statistically powered comparison, monetary savings, universal
  reliability, all-language semantics or production certification follows.

**Overall superiority is not established.** The new run ties regression success,
beats Graphify in total tokens, loses to codebase-memory in total tokens, and
does not beat either in model wall time. All historical failures remain intact.
