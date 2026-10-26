# Lossless payload and native MCP comparison — 2026-10-27

115 frozen golden queries across pinned requests, zod, fmt and ripgrep sources.
55 queries had not been used by the historical comparison; its 60 queries are
regression controls. The compact representation was chosen on 48 separate dev
queries (`../20261026-token-hardening/`). No ranking change was made.

| New 55 queries | File hit@1 | Definition span@1 | Mean raw output tokens | Warm median ms |
|---|---:|---:|---:|---:|
| Previous Findex JSON | 54/55 | 54/55 | 310.05 | 3.24 |
| Current Findex JSON | 54/55 | 54/55 | 312.96 | 3.02 |
| Current Findex compact | 54/55 | 54/55 | 260.07 | 2.99 |
| Codebase-memory JSON | 47/55 | 40/55 | 271.89 | 14.68 |
| Codebase-memory tree | 47/55 | 40/55 | 240.31 | 17.90 |
| Graphify code-only MCP | 48/55 | 42/55 | 2663.47 | 24.85 |

Compact uses 16.9% fewer tokens than current JSON on the new set; across all
115, 369.01 becomes 301.83 (18.2% fewer). The new `truncated` field accounts
for a small increase in default JSON over the previous binary. All 115 default
and compact payloads expand identically, including source strings, flags and
sync diagnostics. All 115 hit lists match the previous binary. Codebase's tree
and JSON adapters produce identical source-hit lists for every query.

**Strict superiority gate fails:** compact still sends 8.2% more tokens than
codebase tree on the new set (13.3% more across all 115). Accuracy and latency
are better in this test; general superiority, billed savings and code-fix
success are not established. See `summary.json` for the explicit gate.

Method: persistent native stdio MCP for all tools, ten candidates, rotating
query/mode order, warm indexes, `o200k_base` tokens. Codebase 0.10.8 uses an
anchored `name_pattern` for exact symbols and BM25 for other queries; both
JSON and tree modes are measured. This is a stronger exact-symbol baseline
than the historical all-BM25 comparison. Graphify 0.9.61 uses its existing
code-only graph, native `query_graph` with depth 2 and token_budget 2000.
Findex 0.2.0 uses exact symbols or lexical search with its existing ranking.

Accuracy sees a maximum 2000-token prefix, including any complete Graphify
source anchors in that prefix. 23/55 Graphify outputs exceed this bound;
the raw token column counts **everything returned**, not just the visible
prefix. Findex and codebase do not exceed the bound. No output truncation
is hidden. These tokenizer counts are tool text, not entire model sessions.

The author-created golden and four-language navigation scope limit external
validity. Historical controls are not independent confirmation. Warm timing
on one Windows 11 workstation is not a cold-index/resource/scale benchmark.
No statistical claim about the population of coding tasks is made.

The first attempt (`../20261027-payload-cost/`) stopped when a parser assertion
detected quoted C++ template names in codebase tree output. The adapter was
fixed with a failing-then-passing regression; the same query hash was preserved
for this complete run. No Findex code or ranking was tuned from that failure.
Failed-attempt data is retained, not silently replaced.

Reproduction (native Windows; pinned corpora and prebuilt graph/CBM indexes
from the original study must be present):

```powershell
$common = @('--out', 'validation/results/NEW-UNUSED-DIRECTORY',
  '--cbm', 'C:\Tools\codebase-memory-mcp.exe',
  '--graphify', 'C:\Python312\Scripts\graphify.exe',
  '--python', 'C:\Python312\python.exe')
python validation/harness/payload_cost.py prepare @common
python validation/harness/payload_cost.py run @common
```

Requires psutil and tiktoken in the harness Python; Graphify in the specified
Python. `--baseline` selects the saved pre-hardening binary (default
`target/token-baseline/findex.exe`, built from 88fd423). Identity/graph/source
hashes are in `manifest.json` and checked again before run. Results may not be
overwritten. Raw public-source responses remain local in ignored `raw/`.
