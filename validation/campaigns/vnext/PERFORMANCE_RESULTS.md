# Performance and resources

Machine: AMD Ryzen 9 8945HX (32 logical CPUs), 31.2 GiB RAM, NVMe,
Windows 11 Pro 10.0.26200. Single machine; small differences are noise.

## Cold-index peak memory (REMAINING_LIMITATIONS #17) — root-caused

Run: `validation/results/20261012T103253Z-599cd1e65674/memory_ablation.json`.
vscode 1.94 (6,877 files), `findex index` on fresh state, 3 cold reps each.

| Build | Peak working set (MiB), 3 reps | Median | Wall (s) |
|---|---|---|---|
| baseline `44f1a38` | 24.7, 24.5, 24.4 | 24.5 | 294, 220, 244 |
| main `caf33fa` | 30.8, 31.0, 30.4 | 30.8 | 33.1, 14.9, 15.5 |
| head (vNext code) | 31.4, 30.5, 30.8 | 30.8 | 14.1, 14.5, 15.9 |
| head, `terms` text removed (ablation) | 24.7, 23.8, 24.5 | 24.5 | 12.7, 13.1, 13.7 |
| head, symbol extraction removed (ablation) | 25.5, 25.6, 25.1 | 25.5 | 4.7, 4.8, 4.9 |

Cause: removing either the derived `terms` text or Tree-sitter extraction
returns the peak to baseline, and the rise appears only on vscode, the
corpus with files near the 512 KiB admission limit. On ripgrep (largest file
238 KB) and tokio (141 KB), the earlier run measured no difference (17.1 →
17.5, 15.3 → 15.2 MiB). The peak is therefore a **transient per-file
allocation** (text + derived terms + parse tree of the largest admitted
file), bounded by `MAX_SOURCE_BYTES`, not growth with repository size.
Not changed: both features are required for the measured retrieval quality.
Symbol extraction is also ~10 s of the 14.5 s vscode cold index; it is not
optimised in this campaign.

## vNext overhead

| Metric | Value | Source |
|---|---|---|
| vscode cold index, main vs head | 14.9–33.1 s vs 14.1–15.9 s (no regression; first main rep includes a cold disk cache) | ablation run |
| `findex context` latency, 65 tasks × 4 corpora (index warm, includes process start and reconcile) | p50 0.114 s, p95 0.152 s, max 0.587 s | `20261012T101245Z-3a2e7de81241/localization.json` |
| `findex search` (main) on the same tasks | p50 0.111 s, p95 0.146 s | same |
| `git grep -w` per title word, summed | p50 0.257 s, p95 0.452 s | same |
| `findex context --json` output size, budget 8000 | p50 4.1 KB, p95 6.5 KB, max 41.8 KB (JSON fields are not budgeted; the rendered text that MCP returns is) | same |
| Release binary size | 13.02 MB (main 12.85 MB, +1.3 %) | `findex.exe` |
| Index schema / DB size | unchanged: vNext adds no tables or columns | code review of `findex-index` |

## Not measured in this campaign

- Full perf matrix (`perf.py`: warm latency, edit workload, concurrency) on
  vNext code. Indexing and search code paths did not change; the context
  query path is covered by the latency above.
- Large-repository behaviour beyond vscode; Windows 10.
