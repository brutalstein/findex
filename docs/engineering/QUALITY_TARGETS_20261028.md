# Quality checkpoint — 2026-10-28

This is a verified source checkpoint, not a public release or a declaration of
universal superiority. Product sources are frozen at `3e9d4d2`; the measured
candidate executable SHA-256 is
`ccb6299d315c18d7d65c06d86b640f9e54100f1092f928e51bd65d2638243c73`.
The paired predecessor executable is
`1e49405dd0dd90b55cff28eb8af0cef56ac3e330ae0a72b14bd51513e51993ca`.
Later changes repair experiment preflight/classification and documentation;
they do not change the measured product engine.

## Implemented behavior

Conceptual task retrieval uses bounded BM25 candidates and source/test-role
ranking. Exact symbol and public lexical search retain their previous behavior.
Missing definitions broaden to task words while retaining references. Rust call
classification checks explicit local receiver annotations and shadowing; aliases,
generic dispatch and unresolved owners remain possible evidence. Every context
report states its admitted-file, hash-verification and syntax-only scope.
Receiver inspection is capped at 2,048 hints and 200,000 scope-walk steps per
source; the existing 512-KiB source and 100,000-file admission caps remain.

## Real issue localization

65 historical issue titles are evaluated at their closing commit's parent.
Gold files are non-test source files changed by the closing commit. These are
reused regression tasks, not a new independent blind study. Both binaries receive
the same task, source and rendered budget; failed commands cannot score as empty
successful retrievals.

| Metric | Paired predecessor | Candidate | Acceptance target |
|---|---:|---:|---:|
| Hit@1 | 30.77% | 40.00% | >=60% — unmet |
| Gold recall@10 | 66.15% | 79.23% | >=90% — unmet |
| MRR, 8,000-token budget | 0.4283 | 0.5446 | Improvement |
| MRR, 1,200-token budget | 0.4283 | 0.5429 | Improvement |

Hit@1 and gold recall@10 are unchanged between those two budgets. The budget
uses the harness's rendered-character estimate, not actual model usage; JSON
metadata is outside the rendered text. These results do not measure whole-session
tokens or successful edits.
[8,000-budget results](../../validation/results/20261028-quality-localization-final/localization.json),
[1,200-budget results](../../validation/results/20261028-quality-localization-1200/localization.json).

## Rust call evidence: precision versus coverage

Each row uses 80 historical heldout functions and a regenerated rust-analyzer
SCIP oracle. The same oracle is used for both binaries; it is not imported into
Findex. Its macro/configuration limitations remain. The historical screenshot's
Tokio 56.7% is not the baseline for this regenerated paired experiment.

| Corpus / metric | Predecessor | Candidate |
|---|---:|---:|
| Tokio conditional macro call precision | 63.97% | 93.40% |
| Tokio micro call precision | 33.12% | 88.10% |
| Tokio macro all-evidence site recall | 91.34% | 91.91% |
| Tokio functions with confident calls | 74/80 | 31/80 |
| Tokio confident-site recall | 72.30% | 17.37% |
| ripgrep conditional macro call precision | 70.04% | 97.76% |
| ripgrep micro call precision | 62.15% | 94.93% |
| ripgrep macro all-evidence site recall | 98.75% | 98.71% |
| ripgrep functions with confident calls | 75/80 | 39/80 |
| ripgrep confident-site recall | 95.53% | 29.31% |

Conditional macro precision excludes functions without confident calls. Many
retained sites move to `possible_call`; confident-site recall and caller-file
recall fall substantially. Tokio micro precision is below 90%. The complete
quality target is **not passed**. Syntax annotations are not type resolution;
even the candidate has false positives in these real samples. Passing targeted
ambiguity regressions does not prove a general zero-false-certainty rate.

Paired raw results:
[Tokio baseline](../../validation/results/20261028-quality-calls-tokio-heldout-baseline/callsites.json),
[Tokio candidate](../../validation/results/20261028-quality-calls-tokio-heldout-candidate/callsites.json),
[ripgrep baseline](../../validation/results/20261028-quality-calls-ripgrep-heldout-baseline/callsites.json),
[ripgrep candidate](../../validation/results/20261028-quality-calls-ripgrep-heldout-candidate/callsites.json).
Seed-7 dev and disjoint seed-11 heldout results are retained separately.

## Navigation and live scope

All 115 public search/symbol hit lists match the previous binary; all 115
compact roundtrips and competitor parser equivalence checks pass. The reused
55-query subset remains at 98.18% hit@1; Findex compact averages 222 output
tokens against codebase-memory tree's 240. The historical subset and individual
corpora still include token losses. These are tool outputs, not billed money or
whole-session usage. No general superiority follows.
[Fresh frozen comparison](../../validation/results/20261028-quality-navigation/summary.json).

The six-session subscription bugfix pilot started five sessions. Four completed
with read-only environment reports and no edits; the fifth was interrupted to
stop spending the remaining quota. The sixth did not start. All five are excluded
from quality/efficiency comparisons, with the original journal retained. No new
live sessions were authorized after the user declined a replacement campaign.

Preflight now removes the parent app's execution context, explicitly selects the
native MXC sandbox and checks effective workspace-write permissions, an actual
write/read/delete, command execution, rejection of writes outside the workspace,
and selected MCP tools in all six arms. This invokes no model. The Codex app also
updated its executable; the frozen pilot manifest refuses that tool drift. A new
campaign must freeze new identities and obtain new session authorization.
[Pilot audit](../../validation/results/20261028-bugfix-live/NOTE.md).

## Verification and remaining work

Native Windows stable format, Clippy with warnings denied, all-target tests and
release build pass; 167 Rust tests also pass on MSRV 1.85.0. The black-box security
run passes 29 checks with no skips. Navigation, budget, coverage and offline
harness checks also pass, including the eight-tool plugin contract, sensitive
paths, corrupt-cache/MCP privacy and project overview. Evidence is linked in the
[integration record](../../validation/results/20261028-quality-integration/NOTE.md).

No fresh cloud-files/SMB/100k-scale timing campaign, official Claude live session,
public packaging, signing or release publication is claimed for this engine.
Prior packages and broad deployment measurements remain tied to their earlier
binary. Issue ranking targets, general call resolution and meaningful downstream
bugfix improvement remain open engineering goals.
