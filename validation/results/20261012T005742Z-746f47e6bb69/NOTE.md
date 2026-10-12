# Run at commit 746f47e (code 8525c50)

Unit, retrieval, relations, performance, security and integration executed.
The A/B token study was not repeated (plan usage); its evidence is run
`20261012T001625Z-bf4967d90524`.

* **clippy FAIL is real.** `dd16d92` changed `synchronize_once` to take
  `project: &Path`, leaving two `&project` arguments that clippy 1.97 rejects
  (`needless_borrows_for_generic_args`, `lib.rs:254` and `lib.rs:282`). The
  follow-up commit removes both borrows; fmt, clippy, 89/0 tests and the
  release build then pass on the same machine (not re-recorded as a run).
* `environment.json` lists `rustc 1.96.1 … gnullvm`: this run's environment
  step was started outside `validation/scripts/_common.ps1`, so it saw the
  standalone toolchain first on `PATH`. Every build step ran through the
  scripts, which put the rustup MSVC 1.97 toolchain first; the benchmarked
  binary ran without `libunwind.dll`, which a gnullvm build requires.
* Concurrency throughput is noisy between runs: baseline vscode measured
  71 rps in the previous run and 96 rps here, on unchanged baseline code.
