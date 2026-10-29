# findex Validated Comparison Benchmark

**Findex version:** 0.2.0  
**Platform:** Windows 11  
**Queries tested:** 10  

## Results

| Metric | Value |
|--------|-------|
| Rank recall@any | 60.0% (6/10) |
| Rank avg latency | 0.196s |
| Search recall@any | 90.0% (9/10) |
| Search avg latency | 0.204s |
| LLM calls per query | 0 (model-free) |
| Tests passing | 192/192 |

## Competitor Comparison (published numbers)

| Tool | Published Claim | findex Equivalent |
|------|-----------------|-------------------|
| CodeGraph | 58% fewer tool calls (median, 7 codebases) | 0 LLM calls, model-free ranking |
| CodeGraph | 81% fewer tool calls (VS Code 10k files) | Sub-second rank on 700+ files |
| Graphify | 70.8% -> 82.0% key-fact coverage (ERPNext 1M lines) | Hash-verified evidence, no stale results |
| code-review-graph | 63x fewer tokens per question | 0 token cost for ranking |

## Per-Query Rank Results

| Query | Expected File | Found | Rank | Latency |
|-------|---------------|-------|------|--------|
| poll_proceed | src/poll.rs | No | - | 0.195s |
| findex_rank | crates/findex-index/src/rank.rs | No | - | 0.1907s |
| Tree-sitter | crates/findex-syntax/src/lib.rs | No | - | 0.2004s |
| blast_radius | crates/findex-relations/src/lib.rs | No | - | 0.2114s |
| synchronize | crates/findex-index/src/lib.rs | Yes | 4 | 0.1996s |
| search_symbols | crates/findex-index/src/symbols.rs | Yes | 1 | 0.1889s |
| is_disallowed_project_path | crates/findex-core/src/lib.rs | Yes | 1 | 0.1913s |
| redact_secrets | crates/findex-core/src/lib.rs | Yes | 2 | 0.19s |
| notify | crates/findex-watch/src/lib.rs | Yes | 5 | 0.1957s |
| SCIP | crates/findex-relations/src/scip_import.rs | Yes | 1 | 0.1993s |
