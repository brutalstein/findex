# Baseline (frozen before enterprise work)

Baseline commit: `247576e49eb5d42999d30dbce55c78e28c9e6397` (`main`, 2026-10-16).
Branch: `enterprise/p0-safety`. Archived results are listed with SHA-256 in
[baseline_manifest.json](baseline_manifest.json); their recorded checksum
files were verified (67/67) and no archived result was modified.

## Environment

| Item | Value |
|---|---|
| OS | Windows 11 Pro 10.0.26200 (native, not WSL) |
| CPU / RAM | AMD Ryzen 9 8945HX, 32 logical CPUs / 31 GiB |
| Toolchain | rustc 1.96.1 (31fca3adb 2026-06-26), cargo 1.96.1, MSVC target |
| `Cargo.lock` SHA-256 | `5056b814c15372b586d1502e4d5f7155363f2f36fed5e38b5ba92273150df2b9` (unchanged by this branch) |
| Disk location | OneDrive-synchronised working copy (locally present files) |

## Corpus pins (`validation/golden/symbol_navigation.jsonl`)

| Corpus | Commit |
|---|---|
| findex | `44f1a387a010b83f3ddf026091224363328b2a02` |
| ripgrep | `4649aa9700619f94cf9c66876e9549d83420e16c` |
| requests | `0e322af87745eff34caffe4df68456ebc20d9068` |
| zod | `ca42965df46b2f7e2747db29c40a26bcb32a51d5` |
| fmt | `0c9fce2ffefecfdce794e1859584e25877b7b592` |
| pybind11 | `a2e59f0e7065404b44dfe92a28aca47ba1378dc4` |
| tokio | `ea6d652a102dee3f22b490db70545b7f66a23fb7` |

The issue-localization set (65 real fixed issues) evaluates each task at the
parent of its fix commit (`validation/golden/issue_localization.jsonl`).

## Baseline measurements

| Measurement | Value at baseline | Source |
|---|---|---|
| Symbol retrieval, 642 queries | file R@1 0.9844, MRR 0.991 | `results/20261016T105926Z-fb1b000c2bef` |
| Black-box security suite | 30/30 PASS | same run |
| Coverage claim, 279 names, *independent* checker | 268/279 hold (frozen main binary) | `results/20261016T170902Z-7068bff0db43/coverage-main-247576e` |
| Coverage claim, earlier shared-heuristic checker | 279/279 (not independent, see audit A1) | `results/20261016T105620Z-ac4ea21dd692` |
| Issue localization, `findex context` | hit@1 0.292, hit@10 0.662, MRR 0.417 | `results/20261012T101245Z-3a2e7de81241` |
| Router, labeled dev set (260) | 178/260 | emulation of the pre-change rules, `CONTEXT_ENGINE_RESULTS.md` |
| Router, held-out set (60) | 30/60 | same |
| Live model sessions started by default runner | possible (audit A10) | `run-all.ps1` before `622f044` |

The frozen pre-fix release binary of `247576e` was built once and is used
via `CARGO_TARGET_DIR` for before/after comparisons; it is not committed.
