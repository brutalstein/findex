# Migration results

Lexical schema (`PRAGMA user_version`) goes from 3 to 4 in `edd41bf`
(FTS5 `secure-delete`). Older schemas are rebuilt from sources; newer ones
are refused.

| Check | Result | Evidence | Status |
|---|---|---|---|
| Legacy (pre-v2) schema migrated and reindexed | migrated, search works | unit test `legacy_lexical_schema_is_migrated_and_reindexed` | PASS |
| v3 → v4 on a real index (ripgrep, 201 files) | v3 cold index 0.42 s; first v4 run rebuilds all 201 files in 0.26 s; next run 201 unchanged, 0.10 s; search works | one manual run with the `7068bff` (v3) and `edd41bf` (v4) release binaries in an isolated state directory; values recorded here only (no results directory) | PASS |
| v4 binary on a v5 index (future downgrade) | refused with "index cache schema 5 is newer than this Findex supports (4)", file left at v5 | unit test `newer_index_schema_is_refused_and_left_intact` | PASS |
| v3 binary (before `edd41bf`) on a v4 index | works: 0 changed, 201 unchanged | same manual run | PASS, by luck of layout: v4 = v3 tables + one FTS5 option. Binaries older than `edd41bf` cannot refuse newer schemas; the protection starts with v4. |
| Corrupt database | deleted with `-wal/-shm/-journal` and rebuilt | unit test `corrupt_index_database_is_rebuilt_from_sources` | PASS |
| Kill -9 during indexing (0.2/0.6/1.5 s) | integrity ok, search ok | security suite at `01ff624` | PASS |

Cost of the upgrade for users: one full re-index of each project on first
use (cold-index time; see PERFORMANCE_RESULTS).

The relations database (`findex-relations`) still has no schema version
(audit A7); its migration behaviour is NOT_RUN.
