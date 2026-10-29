# Claude Haiku pilot (2026-10-30): inconclusive, stopped after two sessions

Subscription sign-in (claude.ai Pro), model `haiku`, no paid API. Continuation of
`../20261030-claude-live-haiku/` after an ACL fault; Findex state is held outside
the `target` tree. Task: `fmt:parse_arg_id:exact` (frozen golden:
`include/fmt/compile.h:300` or `test/base-test.cc:500`). Two of six authorized
starts used here (three Claude starts in total with the first attempt).

| Arm | Valid | Correct | Successful MCP calls | Turns | Notes |
|---|---|---|---:|---:|---|
| Findex | yes | no | 1 | 3 | `findex_find_symbol` (compact) returned every definition, including `compile.h:300`; Haiku answered a list of definitions instead of the single `{path, line}` object the prompt required, so the grader marks it wrong |
| Graphify | no | no | 0 | 3 | MCP server connected, but Haiku never called it (Grep, then Read); answered `base.h:2312`, which is not in the golden |

The harness stops after the first invalid trial by design (no retries, no
fallback), so codebase-memory and the second task did not run.

What this does and does not show: nothing about tool superiority. The task is
ambiguous (`parse_arg_id` is defined in several places and the golden lists two
of them); the Findex "miss" is an answer-format failure of the model, and the
Graphify trial is a model that skipped the supplied tool. Raw transcripts stay
in `raw/live/` (ignored). Remaining budget: four starts; continuing needs the
harness to tolerate an invalid trial instead of halting, which is a design
change and is not made here.
