# Claude Haiku pilot, first attempt (2026-10-30): invalid, environment fault

Subscription sign-in (claude.ai Pro), model `haiku`, no paid API. The first
session (Findex, fmt `parse_arg_id`) connected to the Findex MCP server but its
lookup failed with "Findex symbol lookup unavailable", so Haiku fell back to
Grep and answered correctly from source. Cause: the `target` tree carries an
inherited `Everyone: DENY delete-child` ACE (not present on 2026-10-27), so
SQLite could not create its journal in `target/tool-comparison/state/`. This is
an invalid trial for the comparison (the supplied tool never succeeded), not a
tool result. The harness stopped without retry. The study continues in
`../20261030-claude-live-haiku-b/` with the Findex state held outside that tree
(`FINDEX_LIVE_FINDEX_STATE`). One start is used here.
