# Run at commit bf4967d

All components executed at `bf4967d`. Release-gate evidence for the Claude
Code integration and the A/B token study comes from this run.

Post-hoc correction (recorded per session under `post_hoc_rejudge`): the
t6 judge looked for `kdbx` case-sensitively, although the task asks for
case-insensitive handling. One B session that asserted `"passwords.KDBX"`
was re-judged with the fixed judge against its preserved workspace (hidden
acceptance test and the findex-core suite both passed) and counts as correct.
`comparisons.json`, `summary.md` and checksums were regenerated after it.

Commits after this run (`dd16d92`, `8525c50`) changed only reconcile
locking; the next run directory holds non-Claude evidence at that commit.
