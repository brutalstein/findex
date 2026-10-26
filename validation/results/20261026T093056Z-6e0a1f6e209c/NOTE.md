# Live check after owner-qualified call sites (code 6e0a1f6)

Small confirmation run, not a new study: task `t2_identify_callers` (tokio,
the callers task the external-crate rule affects), arms A (built-ins) and D
(Findex incl. `findex_context`, current build), 2 repetitions, model `haiku`
(claude-haiku-4-5), subscription only (every session's init reported
`apiKeySource: "none"`; see `../20261026-live-canary/`).

| Arm | rep 0 tokens | rep 1 tokens | Correct |
|---|---|---|---|
| A | 75.4k | 102.6k | 2/2 |
| D | 104.7k | 50.6k | 2/2 |

Tokens = input + output + cache write + cache read. Mean D − A = −11.4k
(−12.7 %), but with n = 2 and one D session above both A sessions the run
supports no saving claim. The expensive D session (rep 0) never called a
Findex tool (Grep, Grep, Read, Read); the cheap one made a single
`findex_context` call and answered. Tool adoption, not result quality,
decided the difference here. It confirms that D stays correct with the new build
(recall 1.00, precision 0.80 and 1.00). The token claim remains Run 4
(`validation/campaigns/vnext/CLAUDE_AB_RESULTS.md`, 108 sessions).
