//! Bounded event-driven refresh for files already admitted by the full scanner.
//!
//! Never admit a new path here: doing so would bypass ignore rules. New files
//! are discovered by `synchronize`, while the watcher (`findex-watch`) cheaply
//! refreshes known files as filesystem events arrive.

use super::{connect, symbols, Result, SyncStats};
use findex_core::{is_sensitive_filename, storage_dir, MAX_FILES_PER_SYNC, MAX_SOURCE_BYTES};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Default, Serialize)]
pub struct RefreshReport {
    pub sync: SyncStats,
    /// Unknown paths are intentionally not indexed until a full reconciliation.
    pub unknown_paths: usize,
    /// Known files whose read failed for a reason other than absence (a
    /// Windows sharing violation during a save); their entries are kept
    /// until the next full reconciliation.
    pub unreadable: usize,
}

pub fn refresh_known_paths(project: &Path, paths: &[PathBuf]) -> Result<RefreshReport> {
    let directory = storage_dir(project)?;
    fs::create_dir_all(&directory)?;
    refresh_known_paths_into(project, &directory.join("index.sqlite3"), paths)
}

/// Refresh only existing indexed paths, without a workspace walk.
///
/// Callers must perform a full `synchronize` after a watcher overflow, after
/// discovering new files, on branch changes, and after missed notifications.
/// This operation is transactional: an invalid path rolls back the entire batch.
pub fn refresh_known_paths_into(
    project: &Path,
    database: &Path,
    paths: &[PathBuf],
) -> Result<RefreshReport> {
    if paths.len() > MAX_FILES_PER_SYNC {
        return Err("refresh path budget exceeded".into());
    }

    let project = project.canonicalize()?;
    let mut conn = connect(database)?;
    // IMMEDIATE takes the write lock up front: a deferred read-then-write
    // transaction fails with SQLITE_BUSY at once when another writer committed.
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut seen = BTreeSet::new();
    let mut report = RefreshReport::default();

    for relative in paths {
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!("unsafe relative path: {}", relative.display()).into());
        }

        let key = findex_core::normalized_relative_key(relative)?;
        if !seen.insert(key.clone()) {
            continue;
        }

        let old: Option<String> = tx
            .query_row(
                "SELECT hash FROM files WHERE path = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;

        let Some(old_hash) = old else {
            report.unknown_paths += 1;
            continue;
        };
        report.sync.examined += 1;

        let absolute = project.join(relative);
        let verified = if is_sensitive_filename(&absolute) {
            None
        } else {
            // Never follow symlinked files, junctions or paths outside this root.
            let metadata = fs::symlink_metadata(&absolute).ok();
            let inside = absolute
                .canonicalize()
                .ok()
                .is_some_and(|resolved| resolved.starts_with(&project));
            match metadata {
                Some(meta)
                    if meta.file_type().is_file() && meta.len() <= MAX_SOURCE_BYTES && inside =>
                {
                    match fs::read(&absolute) {
                        Ok(bytes) => Some(bytes).filter(|bytes| {
                            (bytes.len() as u64) <= MAX_SOURCE_BYTES
                                && !bytes.contains(&0)
                                && std::str::from_utf8(bytes).is_ok()
                        }),
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                        // Hits re-verify the hash, so the kept entry can cost
                        // a miss but never return stale text.
                        Err(_) => {
                            report.unreadable += 1;
                            continue;
                        }
                    }
                }
                _ => None,
            }
        };

        if let Some(bytes) = verified {
            let digest = blake3::hash(&bytes).to_hex().to_string();
            let contents = std::str::from_utf8(&bytes)?;
            if digest == old_hash {
                if symbols::upsert(&tx, &key, &digest, contents)? {
                    report.sync.symbols_parsed += 1;
                } else if findex_syntax::supports_path(relative) {
                    report.sync.symbols_reused += 1;
                }
                report.sync.unchanged += 1;
                continue;
            }
            if symbols::upsert(&tx, &key, &digest, contents)? {
                report.sync.symbols_parsed += 1;
            }
            super::store_document(&tx, &key, contents, &digest, bytes.len() as i64)?;
            report.sync.indexed += 1;
        } else {
            // Removal also applies to unreadable, oversized and binary sources.
            // Search verifies hashes, so a racing write cannot return stale code.
            super::remove_document(&tx, &key)?;
            symbols::clear(&tx, &key)?;
            report.sync.removed += 1;
        }
    }

    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{search_in_db, synchronize_into};
    use tempfile::tempdir;

    #[test]
    fn edits_and_deletions_do_not_require_a_full_walk() {
        let workspace = tempdir().expect("workspace");
        let database = tempdir().expect("state");
        let db = database.path().join("index.db");
        let first = workspace.path().join("first.rs");
        let second = workspace.path().join("second.rs");
        fs::write(&first, "fn old_marker() {}").expect("first file");
        fs::write(&second, "fn other_marker() {}").expect("second file");
        synchronize_into(workspace.path(), &db).expect("initial reconciliation");

        fs::write(&first, "fn new_marker() {}").expect("edit");
        let result = refresh_known_paths_into(
            workspace.path(),
            &db,
            &[PathBuf::from("first.rs"), PathBuf::from("first.rs")],
        )
        .expect("refresh");
        assert_eq!(result.sync.indexed, 1);
        assert_eq!(result.sync.examined, 1);
        assert_eq!(result.unknown_paths, 0);

        let old = search_in_db(workspace.path(), &db, "old_marker", 5, SyncStats::default())
            .expect("search old");
        assert!(old.hits.is_empty());
        let new = search_in_db(workspace.path(), &db, "new_marker", 5, SyncStats::default())
            .expect("search new");
        assert_eq!(new.hits.len(), 1);

        fs::remove_file(&first).expect("delete");
        let removed = refresh_known_paths_into(workspace.path(), &db, &[PathBuf::from("first.rs")])
            .expect("refresh deletion");
        assert_eq!(removed.sync.removed, 1);
        let unaffected = search_in_db(
            workspace.path(),
            &db,
            "other_marker",
            5,
            SyncStats::default(),
        )
        .expect("search unaffected");
        assert_eq!(unaffected.hits.len(), 1);
    }

    /// An editor holding a file open without sharing (Windows sharing
    /// violation) must not drop it from the index until the next full scan.
    #[cfg(windows)]
    #[test]
    fn locked_file_keeps_its_index_entry() {
        use std::os::windows::fs::OpenOptionsExt;
        let workspace = tempdir().expect("workspace");
        let database = tempdir().expect("state");
        let db = database.path().join("index.db");
        let file = workspace.path().join("locked.rs");
        fs::write(&file, "fn locked_marker() {}").expect("file");
        synchronize_into(workspace.path(), &db).expect("initial reconciliation");

        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file)
            .expect("exclusive open");
        let report = refresh_known_paths_into(workspace.path(), &db, &[PathBuf::from("locked.rs")])
            .expect("refresh while locked");
        let full = synchronize_into(workspace.path(), &db).expect("full scan while locked");
        drop(lock);
        assert_eq!(full.removed, 0);
        assert_eq!(report.sync.removed, 0);
        assert_eq!(report.unreadable, 1);
        let found = search_in_db(
            workspace.path(),
            &db,
            "locked_marker",
            5,
            SyncStats::default(),
        )
        .expect("search after unlock");
        assert_eq!(found.hits.len(), 1);
    }

    #[test]
    fn unknown_paths_do_not_bypass_exclusions() {
        let workspace = tempdir().expect("workspace");
        let database = tempdir().expect("state");
        let db = database.path().join("index.db");
        fs::write(workspace.path().join("known.rs"), "fn visible() {}").expect("known");
        synchronize_into(workspace.path(), &db).expect("initial reconciliation");
        fs::write(workspace.path().join(".env"), "private_token_marker").expect("secret");
        fs::write(workspace.path().join("new.rs"), "fn newly_created() {}").expect("new");

        let report = refresh_known_paths_into(
            workspace.path(),
            &db,
            &[PathBuf::from(".env"), PathBuf::from("new.rs")],
        )
        .expect("refresh");
        assert_eq!(report.unknown_paths, 2);
        assert!(search_in_db(
            workspace.path(),
            &db,
            "private_token_marker",
            5,
            SyncStats::default()
        )
        .expect("search")
        .hits
        .is_empty());
        assert!(search_in_db(
            workspace.path(),
            &db,
            "newly_created",
            5,
            SyncStats::default()
        )
        .expect("search")
        .hits
        .is_empty());

        synchronize_into(workspace.path(), &db).expect("discover new files");
        assert_eq!(
            search_in_db(
                workspace.path(),
                &db,
                "newly_created",
                5,
                SyncStats::default()
            )
            .expect("discovered")
            .hits
            .len(),
            1
        );
    }

    #[test]
    fn unsafe_paths_abort_without_modifying_the_index() {
        let workspace = tempdir().expect("workspace");
        let database = tempdir().expect("state");
        let db = database.path().join("index.db");
        fs::write(workspace.path().join("safe.rs"), "fn safe_marker() {}").expect("file");
        synchronize_into(workspace.path(), &db).expect("initial reconciliation");
        fs::write(workspace.path().join("safe.rs"), "fn changed_marker() {}").expect("edit");
        assert!(refresh_known_paths_into(
            workspace.path(),
            &db,
            &[PathBuf::from("safe.rs"), PathBuf::from("../outside")]
        )
        .is_err());
        assert!(search_in_db(
            workspace.path(),
            &db,
            "changed_marker",
            5,
            SyncStats::default()
        )
        .expect("rollback")
        .hits
        .is_empty());
        // The previous indexed snapshot is stale but never returned as current.
        assert_eq!(
            search_in_db(
                workspace.path(),
                &db,
                "safe_marker",
                5,
                SyncStats::default()
            )
            .expect("search")
            .stale_results_discarded,
            1
        );
    }

    #[test]
    fn binary_edit_revokes_previous_index_entry() {
        let workspace = tempdir().expect("workspace");
        let database = tempdir().expect("state");
        let db = database.path().join("index.db");
        fs::write(workspace.path().join("src.rs"), "fn previous_code() {}").expect("text");
        synchronize_into(workspace.path(), &db).expect("initial reconciliation");
        fs::write(workspace.path().join("src.rs"), [65, 0, 66]).expect("binary");
        let report = refresh_known_paths_into(workspace.path(), &db, &[PathBuf::from("src.rs")])
            .expect("refresh");
        assert_eq!(report.sync.removed, 1);
        assert!(search_in_db(
            workspace.path(),
            &db,
            "previous_code",
            5,
            SyncStats::default()
        )
        .expect("search")
        .hits
        .is_empty());
    }
}
