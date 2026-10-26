# Owner-qualified call sites (`Type::name`, `module::name`)

Oracle: rust-analyzer SCIP references (`validation/harness/callsites.py`).
Task: ``callers of `Owner::name` `` (`--qualified`). Two disjoint 80-function
samples per corpus: `dev` (seed 7, used while developing) and `heldout`
(seed 11, sampled from the remaining functions). The baseline for both
splits was recorded with the 0b9f6c2 build **before** any owner rule was
written (`../20261026-callsites-baseline-0b9f6c2/`). The held-out split
was run once, at the end, with no change made after it.

Macro means (per function). `call` = confident calls only; `call+possible`
adds the `possible call` tier (receiver type unknown, ranked last).

| corpus / split | call precision | call+possible precision | site recall | caller-file precision | caller-file recall |
|---|---|---|---|---|---|
| ripgrep dev, baseline | 0.525 | — | 0.951 | 0.603 | 0.896 |
| ripgrep dev, this change | 0.711 | 0.673 | 1.000 | 0.805 | 0.910 |
| ripgrep held-out, baseline | 0.523 | — | 0.957 | 0.614 | 0.940 |
| ripgrep held-out, this change | **0.714** | 0.689 | **0.981** | 0.860 | 0.955 |
| tokio dev, baseline | 0.391 | — | 0.878 | 0.441 | 0.814 |
| tokio dev, this change | 0.644 | 0.588 | 1.000 | 0.690 | 0.877 |
| tokio held-out, baseline | 0.376 | — | 0.829 | 0.440 | 0.763 |
| tokio held-out, this change | **0.567** | 0.548 | **0.947** | 0.683 | 0.762 |

Unqualified tasks (``callers of `name` ``, dev) also gain from the
external-crate rule with no recall change: ripgrep call precision
0.519 -> 0.520, tokio 0.391 -> 0.418 (site recall 0.951 / 0.878 unchanged).

## What decides a line

Text rules, not type resolution; every exclusion is counted under `missing`.

- `Owner::name`, `Self::name` / `self.name` inside the owner's impl: call.
  Inside another impl that defines `name` itself: excluded.
- A path through another owner, a call form the target cannot take (`.f(`
  for a free function, bare `f(` for a method), or a Rust file naming only
  other owners: excluded.
- A path rooted in an external crate (`std`/`core`/`alloc` or a Cargo
  dependency no project manifest names as its own package), or a bare call
  imported only from one: excluded, also for unqualified tasks.
- Receiver named after the owner (`rt_handle` for `Handle`): call. Unknown
  receiver: call where the file names the owner, else `possible call`.
  Receiver named after another owner in a file naming this one: `possible call`.
- A name inside a string literal is no call evidence.

A tried variant that made every unknown receiver a `possible call` raised
dev call precision (ripgrep 0.711 -> 0.802) but cut caller-file recall
(0.910 -> 0.657) and was rejected.

## False negatives (held-out, not tuned on)

Sites the baseline found and this change drops:

- ripgrep `Match::zero`: called as `Range::zero` through `use ... Match as Range`;
  import aliases are not followed.
- tokio `Init::init`: a trait method called through an implementing type
  (`OsExtraData::init()`); the owner is not known to be a trait.
- tokio `OwnedTasks::num_alive_tasks` (1 of 2 sites): receiver is a field
  chain (`self.shared.owned`) in a file that names only another owner.

Caller-file recall also drops where a site moved to the `possible call`
tier (it counts confident calls only), for example tokio
`raw::try_read_output` and `LocalKey::sync_scope`.

## Regression checks on the same build

- `coverage_check.py`: 279/279 exhaustiveness claims hold on 7 corpora.
  Lines excluded through an external crate are reported and counted like
  method-syntax exclusions.
- `localization.py` (65 tasks), findex-context: hit@1 0.308 -> 0.308,
  MRR 0.426 -> 0.428, gold recall@10 0.646 -> 0.662.
- `budget_check.py`: 0 reports over budget.
