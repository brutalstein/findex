# Defects — found, fixed, open

## Fixed in vNext (each reproduced by a failing test or measurement first)

| # | Defect | Fix | Evidence |
|---|---|---|---|
| F1 | Findex tools deferred in Claude Code: first use costs a `ToolSearch` turn | `findex_context` with `anthropic/alwaysLoad` | MCP test; pilot sessions D/E: 2 turns vs 3–6 |
| F2 | Tree-sitter-hidden definitions (macro-generated) misclassified as calls | lexical definition detection | `definitions_hidden_from_tree_sitter_are_found_lexically` |
| F3 | All-caps acronyms (`SQLite`) treated as identifiers; prose listed as calls | lower→upper camel rule; `mention` kind for non-code files | `acronyms_are_words_and_prose_mentions_are_not_references` |
| F4 | Rust free-function callers included `.name(` method calls of other types | excluded and reported | `rust_free_function_callers_exclude_method_syntax` |
| F5 | Same-name definitions presented without warning | ambiguity disclosure | `shared_names_are_disclosed_as_ambiguous` |
| F6 | Unknown identifier in an issue title → empty result (dev 3/37) | text-search fallback | `unmatched_identifier_falls_back_to_the_task_words`; dev empty rate 0.081 → 0 |
| F7 | Vendored test frameworks (gtest) outranked source on concept tasks | tests after source | `concept_task_ranks_test_code_after_source` |
| F8 | Repository text could forge report lines | indentation + untrusted label | `repository_text_cannot_forge_report_lines` |
| F9 | Relations DB first open: "database is locked" under concurrency (REMAINING_LIMITATIONS #6) | BUSY/LOCKED retry | `concurrent_first_opens_of_a_new_database_all_succeed` |
| F10 | A/B sessions loaded the operator's CLAUDE.md and skills | `CLAUDE_CODE_DISABLE_CLAUDE_MDS=1`, Skill denied | probe; CLAUDE_AB_RESULTS |
| F11 | Expired sign-in sessions scored as wrong answers | `blocked_session()` → BLOCKED, run stops | harness self-checks; run `96295ee75261` reclassified |
| F12 | Study results written only at the end (lost on interruption) | save after every session, `--resume` | `claude_ab.py` |

## Root-caused, not changed

| # | Item | Finding |
|---|---|---|
| R1 | +25 % vscode cold-index peak memory (#17) | transient per-file peak from `terms` + Tree-sitter on files near 512 KiB; bounded, not repo-size growth (PERFORMANCE_RESULTS) |

## Open

| # | Severity | Defect | Why open |
|---|---|---|---|
| O1 | medium | Call sites of shared names are mostly wrong (precision 0.06–0.15 for 4+ definitions) | needs type resolution; disclosed in every report |
| O2 | medium | Vendored packages inside a project (`requests/packages/urllib3`) outrank the project's code | no reliable language-independent vendored-code signal without false negatives |
| O3 | medium | hit@10 on held-out localization not improved (0.750 vs 0.786 main) | concept fallback ranks by lexical coverage; top-10 breadth unchanged |
| O4 | low | `findex context --json` output is not budgeted (max 41.8 KB); the MCP text output is | CLI JSON is for tooling |
| O5 | medium | Claude A/B study incomplete | BLOCKED on sign-in and plan limits |
| O6 | medium | Cross-language edge precision unmeasured | SCIP indexers for Python/TS/C++ not installed |
