# Retrieval integrity and bounded watcher work

Goal: continue from 28842c5 with reproducible fixes to existing contracts,
using Findex itself during diagnosis. Execute inline in the current checkout.

Design: preserve the existing lexical ranking, source validation and crate
boundaries. Export the existing ranked-file list to JSON; retain the old JSON
fallback in the localization harness. Propagate semantic-site truncation and
stale counts. Filter safely excluded filesystem events before queue admission,
while preserving errors, rescan flags, Git changes and mixed/outside paths.
No dependencies, model calls, credentials, imported graph rebuild or release.

Alternatives: embeddings/compiler-backed context require new evaluation and
architecture decisions; rank-weight tuning on the exposed issue set risks
overfitting. Neither is needed to fix these observable correctness defects.

- [x] Reproduce JSON ranking loss, hidden semantic truncation/staleness and
  ignored build-event queue overflow with focused failing regressions.
- [x] Fix each responsible boundary; verify exact identities, stale rejection,
  overflow recovery and conservative event handling remain intact.
- [x] Run native fmt, Clippy, workspace tests and release build. Run offline
  Python tests, real compiler-backed semantic fixtures, paired historical
  localization and native scaling where prerequisites are available.
- [x] Record binary identities, measurements and limitations; refresh Graphify
  and update the handoff so old milestones are not mistaken for HEAD evidence.

Acceptance: no silent missing semantic evidence; excluded build storms cannot
crowd out a real source update; clients receive the existing ranking explicitly;
no unexplained regression on frozen navigation/localization controls. Synthetic
scale and authored compiler cases do not prove arbitrary monorepo/language
coverage. Existing untracked validation runs must remain untouched.
