# Final native wide-source query profile

PASS: 1200 synthetic 64-KiB Rust sources, ten alternating warm common-term requests per immutable build. All twenty expanded payloads are equal, with eight expected path/line anchors and explicit truncation. Initial indexing excluded; OS caches not flushed. See manifest hashes and results for latency and lifetime process peak. No model/API calls.

Reproduce: `python validation/harness/query_profile.py --baseline target/scale-baseline/findex.exe --out validation/results/NEW-WIDE`. The first attempt is retained separately.
