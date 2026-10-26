# Live canary: subscription-only routing

One session, `haiku`, one turn, no tools, prompt "Reply with the single word: ok",
started through `claude_session.run_claude` after `live_model_gate` passed.

- `claude auth status`: claude.ai sign-in, first-party provider, `pro` subscription.
  It also reports `apiKeySource: apiKeyHelper` from the operator's user settings,
  which route to a local proxy. Sessions load `--setting-sources project,local`
  only and receive no billing or routing variables (`session_env`), so that
  helper never applies to them.
- The session's own init event: `apiKeySource: "none"`; model
  `claude-haiku-4-5-20251001`; answer `ok`. `run_claude` now stops the study
  when a session reports any other key source.
- The local proxy port (127.0.0.1:3456) was not listening during the run.
- `total_cost_usd` (0.047) is Claude Code's equivalent-price estimate, not a charge.
