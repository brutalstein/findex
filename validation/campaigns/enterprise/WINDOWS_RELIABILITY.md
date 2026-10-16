# Windows reliability

All results are native Windows 11 (10.0.26200) on the machine in BASELINE.

| Check | Result | Evidence | Status |
|---|---|---|---|
| File locked with share mode 0 during refresh | entry kept, `unreadable` = 1, found again after unlock without re-index | `locked_file_keeps_its_index_entry` | PASS |
| File locked during full reconciliation | entry kept (`removed` = 0), never served while unverifiable | `exclusively_locked_file_is_kept_but_never_served_unverified` (replaces the earlier test that asserted removal) | PASS |
| Watcher edits / creates / renames / deletes (ripgrep) | 25/25, 10/10, 10/10, 10/10 correct | perf runs `fbc73ad` and `edd41bf` | PASS |
| 4 MCP clients sharing one index | 0 errors | perf runs | PASS |
| Kill -9 during indexing | integrity ok, recovers | security suite `01ff624` | PASS |
| Branch switch under running watcher; locally present OneDrive folder | pass (earlier run, not repeated) | `fb1b000`, vNext FINAL_ACCEPTANCE | PASS at `fb1b000`, NOT_RUN at HEAD |
| Unicode names, paths over MAX_PATH, NTFS junction, ADS / UNC / device names | pass | `tests/windows_fs.rs`, security suite | PASS |
| 8.3 short names | — | — | NOT_RUN |
| Windows 10, network volumes, cloud-only OneDrive placeholders | — | — | NOT_RUN |

A sharing violation during a save no longer creates a false negative until
the next reconciliation; a file that stays unreadable keeps its old entry,
which is never returned because every hit re-reads and re-hashes the file.
