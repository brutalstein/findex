# Final release-candidate comparison — 2026-10-27

Actual Windows MSVC/static-CRT release binary, 115 frozen queries, six native
MCP modes, 690 responses. All 115 compact/default payloads expand identically;
all 115 source-hit lists equal the pre-hardening binary. Codebase tree/JSON
adapters also agree on every source-hit list. No tool errors were recorded.

| 55 queries new relative to the original study | File hit@1 | Definition span@1 | Mean raw output tokens | Warm median ms |
|---|---:|---:|---:|---:|
| Findex JSON | 54/55 | 54/55 | 312.96 | 3.12 |
| Findex compact | 54/55 | 54/55 | 222.07 | 2.90 |
| Codebase-memory JSON | 47/55 | 40/55 | 271.89 | 13.66 |
| Codebase-memory tree | 47/55 | 40/55 | 240.31 | 27.75 |
| Graphify code-only MCP | 48/55 | 42/55 | 2663.47 | 25.50 |

Compact reduces Findex's own current JSON by **29.0%** on this subset and
**28.5%** across all 115 (369.01 → 263.83). The primary subset passes the
strict local aggregate token/file/span gate against both competitors. Across
all 115, Findex is 112/115 on file and span@1, codebase 85/115 and 72/115,
Graphify 87/115 and 76/115. Mean raw tokens are 263.83, 266.37 and 2274.87.

**Losses and limits remain:** the all-115 token advantage over codebase tree
is only 0.95%; on the historical 60 controls Findex uses **more** tokens
(302.12 vs 290.27). Per-corpus mean raw tokens across all queries:

| Corpus | Findex compact | Codebase tree |
|---|---:|---:|
| requests | 230.33 | 207.44 |
| zod | 287.77 | 278.69 |
| fmt | 265.44 | 313.22 |
| ripgrep | 270.66 | 266.54 |

Codebase remains smaller on three of four corpora. `summary.json` exposes
gates per subset and corpus, including these failures. Neither a successful
aggregate gate nor the separate two-task live pilot proves superiority on
every repository, architecture question, code change, or resource dimension.

The format was developed on 48 separate dev queries. After the initial
matrix-only comparison, one further general encoding was prototyped on those
48 dev queries: compact `sync: 0` represents the full all-zero synchronization
counter object, saving 38 tokens per idle response. Nonzero work preserves
the full object. Source evidence, stale-result counts and truncation flags are
unchanged; JSON remains the default. Thus the final run is a regression
confirmation on a reused evaluation set, not a second blind held-out study.
No ranking or query selection was tuned between format revisions.

Method: pinned, clean four-corpus content hashes and graph/binary hashes in
`manifest.json`, checked again before run; 10 candidates; o200k_base; rotating
mode order; persistent warm native stdio MCP. Exact codebase queries use
anchored regex; other queries use BM25. Codebase 0.10.8 is measured in both
tree and JSON. Graphify 0.9.61 uses its code-only graph, depth=2 and
token_budget=2000. Accuracy uses a 2000-token visible prefix with only complete
source anchors; the raw column counts all output. 23/55 Graphify responses
exceed that budget; no Findex/codebase response does. Timings are one Windows
11 workstation and fluctuate; cold build/index speed, idle CPU, peak RAM and
disk cost are not established by this comparison. The author-created golden
also limits external validity.

Earlier attempts remain available: parser-stop `../20261027-payload-cost/`,
matrix-only GNU `../20261027-payload-cost-v2/`, and matrix-only MSVC
`../20261027-payload-cost-msvc/`. The real Codex pilot used the GNU matrix-only
build, before the final portable build and zero-sync encoding. Its usage is
not relabelled as usage measured on the final executable. Final native Codex
installation tests and all 115 hit/roundtrip checks bridge compatibility;
there were no additional model calls after the twelve-start cumulative cap.

Reproduce with a new empty output directory; preserve the saved 88fd423
baseline as `target/token-baseline/findex.exe` and the pinned corpora/graphs
and codebase projects from the original study:

```powershell
$paths = @('--out', 'validation/results/NEW-UNUSED-DIRECTORY',
  '--cbm', 'C:\Tools\codebase-memory-mcp.exe',
  '--graphify', 'C:\Python312\Scripts\graphify.exe',
  '--python', 'C:\Python312\python.exe')
python validation/harness/payload_cost.py prepare @paths
python validation/harness/payload_cost.py run @paths
```

Harness Python needs psutil and tiktoken; the specified Python needs Graphify.
Raw responses remain local in ignored `raw/`. All aggregate losses and source
anchors are retained in the committed records. No model/API is used here.
