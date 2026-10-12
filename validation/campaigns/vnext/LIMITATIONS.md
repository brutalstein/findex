# Limitations of vNext and of its evidence

1. **Token savings not demonstrated.** The A–E study ran 19 sessions (one
   per cell) before a plan limit, and the rerun was blocked by an expired
   sign-in. Single-session observations favour D/E on t1/t2 (2 turns vs 3–6),
   but there are no repetitions, no CI and no data for t3–t6.
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
6. **Windows reliability items not exercised:** OneDrive placeholders and SMB
   shares (#9), mass branch switches under the watcher (#8), Windows 10.
7. **Intent routing is a word list** (English). Non-English tasks fall back
   to concept search over their words.
8. **Same author.** The engine, harness and documents were written by the
   same agent. The localization labels (issue titles, fix commits) and the
   SCIP oracle are independent of it; the A/B task judges are not.
9. The previous campaign's limitations in `validation/REMAINING_LIMITATIONS.md`
   still apply except #6 (fixed) and #17 (root-caused).
