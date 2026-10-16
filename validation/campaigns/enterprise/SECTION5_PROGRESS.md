# Section 5: enterprise indexing and durability — acceptance boundary

This report is tied to PR Section 5's actual code and zero-model CI workflow.

## Implemented in this change
1. An incomplete repository walk fails the reconciliation transaction instead
   of silently evicting existing documents belonging to inaccessible subtrees.
2. Transient metadata/canonicalization failures keep already indexed entries;
   read-path hash checks still suppress stale data.
3. `findex index-check` verifies SQLite quick_check, FTS5's internal integrity
   command, and bijective `files.id`↔`docs.rowid` invariants without silently
   repairing or overwriting a damaged cache.
4. A deterministic native Windows CLI workload creates 1,200 source files;
   verifies index admission; refreshes 130 known paths; deletes 80 and renames
   60; verifies source-level freshness and counts; and performs two concurrent
   full reconciliations. CI records JSON evidence.

## Release claim boundaries
The 1,200-file workload is a correctness smoke test, **not** a 100k-file load
benchmark, a formal latency guarantee, or proof of network-storage support.
Real 10k/100k-project tests, CPU/RAM measurements, external-content FTS5 schema
ablation, generation-based snapshot publication, disk-full fault injection,
crash/kill recovery, OneDrive cloud-only and SMB environments, and sustained
multi-process contention remain independent Section 5 release gates.

Do not label Section 5 fully completed on the strength of a passing CI alone.
Keep a maintenance check separate from the ordinary query path.

## Reproduction on Windows
- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace --all-targets`
- `cargo build --locked --release`
- `python validation/harness/section5_stress.py target/release/findex.exe --files 1200 --out section5-stress.json`
- `findex index-check` in a previously indexed project.

No live Claude sessions, model APIs, or paid benchmarking services.
