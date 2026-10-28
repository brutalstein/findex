# Interrupted subscription bugfix pilot

The user authorized six new Codex subscription sessions after coding, with no
paid APIs and no subagents. Five starts were recorded: four completed with usage,
one was interrupted, and `ripgrep-2659:findex` never started. All completed sessions
reported read-only execution policy, made no source changes and ran no regression
test. **Zero trials are evaluable as bugfix successes or failures.**

`live-results-as-recorded.json` preserves the initial journal, whose `valid=true`
flags checked usage/MCP activity but failed to check editability. Those flags are
incorrect for a bugfix experiment. `live-results.json` corrects the classification,
sets outcomes to null and stops the study. Partial usage is retained for audit,
not a tool-efficiency comparison. The interrupted session's usage is unavailable.

The original `native-preflight.json` checked MCP availability only. The
`permission-probe.json` diagnostic also did not establish workspace isolation and
is superseded. `native-preflight-v2.json` checks effective workspace-write, an
actual write/read/delete, command execution, rejection of an outside write and
MCP tools across all six arms, with **zero model sessions**. The harness removes
inherited parent app context and explicitly selects native MXC; no global config,
credentials or authentication files are modified or copied.

The Codex desktop update replaced the executable from the frozen manifest.
The successful new preflight is not a rerun of the original frozen live client;
the live runner rejects executable drift. New paired measurements need a new
manifest and authorization. On 2026-10-28 the user explicitly chose **no new
sessions; integrate verified source and documentation**. No retry or sixth start
was performed.
