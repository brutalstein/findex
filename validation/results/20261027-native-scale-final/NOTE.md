# Final native scaling

PASS: final executable hash matches comparison, wide-query, installed runtime and package. Separate identical source directories and fresh indexes per build/repetition; alternating order, 3 pairs at 200/2k/10k, 100k candidate only once. OS/antivirus caches not flushed. All 245 source-anchor checks, 105 paired hit lists and 100001-file rollback/recovery assertions pass. Tiny synthetic Rust files do not establish arbitrary-project performance.

Reproduce: `python validation/harness/scale.py --baseline target/scale-baseline/findex.exe --out validation/results/NEW-SCALE` after a native release build. Baseline hash is frozen in manifest.json. No models/APIs.
