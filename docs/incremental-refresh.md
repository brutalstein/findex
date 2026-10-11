# Incremental refresh safety contract

The initial path-refresh primitive in `findex-index` is **not** a native
filesystem watcher. It is designed to be called by a future Windows watcher.

- `synchronize`: full filesystem reconciliation. Required at startup, after
  lost/overflowed events, for newly created files, and after branch/worktree
  switches. This method applies the project's ignore policies.
- `refresh_known_paths`: transactional, targeted content refresh for paths
  already admitted to the index by a full scan. Unknown paths cannot be
  indexed through this method, even if they exist. They are counted separately.
- The targeted refresh rejects absolute paths, traversal, and non-normal path
  components; it never follows symlinks or junctions outside the canonical
  workspace. Invalid batches roll back atomically.
- Changed, removed, binary, unreadable or oversized files invalidate their old
  indexed body; unchanged files avoid unnecessary FTS writes.
- Search still hashes candidate files before showing evidence, protecting
  against stale code between a watcher event and a database update.
- A future watcher MUST reconcile on event loss, new paths, root replacement,
  branch switches and restart. It MUST NOT treat file metadata as proof of
  unchanged content, or advertise complete freshness without reconciliation.

No native watcher, real token accounting, or AST/LSP graph is claimed here.
