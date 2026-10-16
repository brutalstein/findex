# Limitations of vNext and of its evidence

1. **Claude Code savings are task-dependent.** Run 3 (`397bf78`): D +31 %
   tokens vs A. Run 4 (`ac4ea21`, coverage line + trust rule): D −23.7 % on
   held-out navigation tasks and −15.9 % overall, but t3 (concept question)
   +14.0k tokens, and no significant change on cross-file traces or code
   changes. 3 repetitions per cell; arm A varies widely between runs, so only
   within-run paired comparisons are valid.
2. **One model.** All sessions used Haiku 4.5. Nothing is claimed for
   Sonnet/Opus.
3. **Small held-out localization set** (28 tasks, 4 corpora; ripgrep and fmt
   are 23 of 28). MRR and hit@1 gains have CIs that exclude zero; hit@5 is
   borderline; hit@10 is not improved.
4. **Name-based relations.** `call`/`reference`/`import` are name matches.
   For uniquely named Rust functions, call precision is 0.79–0.82 and site
   recall 1.0; shared names are unreliable (GRAPH_RESULTS). Only Rust has a
   compiler oracle in this campaign.
5. **Spec items not built:** typed multi-language relationship graph (P2),
   project knowledge/memory layer, DB size reduction. Reasons are in
   ARCHITECTURE_DECISIONS and GRAPH_RESULTS; none is claimed.
6. **Windows reliability items not exercised:** OneDrive cloud-only
   placeholders, SMB shares, Windows 10. Mass branch switch under the watcher
   and a locally present OneDrive folder pass (`results/20261016T105742Z-384393cfdee2`).
7. **Intent routing is a word list** (English). Non-English tasks fall back
   to concept search over their words.
8. **Same author.** The engine, harness and documents were written by the
   same agent. The localization labels (issue titles, fix commits), the
   SCIP oracle and `git grep` (coverage check, held-out callers truth) are
   independent of it; the A/B task judges are not.
9. The previous campaign's limitations in `validation/REMAINING_LIMITATIONS.md`
   still apply except #6 (fixed) and #17 (root-caused).
