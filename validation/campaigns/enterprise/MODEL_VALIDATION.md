# Model validation

| Item | Status |
|---|---|
| New live Claude sessions in this campaign | **0** (NOT_RUN) |
| Paid API calls, cloud instances, paid inference | 0 |
| LLM-as-judge for routine correctness | not used: compilation, `git grep`, SCIP and frozen labels decide |

## Gate

`validation/harness/claude_session.live_model_gate` must pass before any
`claude` process starts. It requires all of: `--enable-live-model-tests`;
`FINDEX_LIVE_MODEL_APPROVAL="I approve Claude subscription usage"`; not CI;
none of `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`,
`CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX`, `CLAUDE_CODE_USE_FOUNDRY`,
`AWS_BEARER_TOKEN_BEDROCK`; `claude auth status` reporting a logged-in
first-party `claude.ai` subscription with no API-key source. On the
development machine the API-key source is `apiKeyHelper`, so the gate would
report BLOCKED even with approval.

`test_live_gate.py` (11 tests, run in CI) blocks `subprocess`/`os.system`
launches of `claude` and a PATH shim catches PowerShell launches; the
default `run-all.ps1`, `-Steps integration,claude-ab` and every harness
entry point were run without starting a client.

## Proposed P5 smoke study (not started; needs operator approval)

At most 12 sessions: 2 arms (without / with Findex) × 6 tasks
(2 callers EN, 1 callers TR, 1 tests, 1 file overview, 1 concept), one
model, `--max-sessions 12`, resume enabled, stop on the first gate failure.
Success criteria fixed before running: task correctness by the existing
deterministic judges; tokens and turns reported per session; no aggregate
claim from 6 tasks. The earlier studies (120-session A–E, negative; 108-session
A/D/F, positive on navigation) are unchanged and are not re-interpreted here.
