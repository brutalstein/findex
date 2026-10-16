//! Persistent, revision-verified syntax definitions; no guessed references.
//!
//! Definition extraction and lexical FTS updates share one SQLite transaction.
//! An extractor version plus BLAKE3 content hash controls cache invalidation.

use crate::{connect, synchronize, Result, SyncStats};
use findex_core::{is_disallowed_project_path, storage_dir, MAX_SOURCE_BYTES};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use std::fs;
use std::path::{Component, Path};

// 2: method/impl/macro/constant kinds, C++ forward-declaration filtering, JS via TSX grammar.
const EXTRACTOR_REVISION: i64 = 2;
const MAX_RESULTS: usize = 32;

#[derive(Debug, Clone, Serialize)]
pub struct SymbolHit {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub language: String,
    pub start_line: usize,
    pub end_line: usize,
    pub has_syntax_errors: bool,
    pub truncated: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct SymbolSearchReport {
    /// Reconciliation work performed before this query, if any.
    pub sync: SyncStats,
    pub hits: Vec<SymbolHit>,
    pub stale_results_discarded: usize,
    pub retrieval: &'static str,
}

/// Additive schema migration for projects with a pre-existing lexical index.
pub(crate) fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS symbol_state(
            path TEXT PRIMARY KEY NOT NULL,
            hash TEXT NOT NULL,
            extractor_revision INTEGER NOT NULL,
            language TEXT NOT NULL,
            has_syntax_errors INTEGER NOT NULL,
            truncated INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS symbols(
            path TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            PRIMARY KEY(path, ordinal)
        );
        CREATE INDEX IF NOT EXISTS symbols_by_name
            ON symbols(name COLLATE NOCASE, path);
        CREATE INDEX IF NOT EXISTS symbols_by_identifier
            ON symbols(lower(replace(name, '_', '')), path);",
    )?;
    Ok(())
}

/// A cached definition whose file hash still matches the lexical index.
pub(crate) struct DefinitionRow {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
    pub hash: String,
}

/// Definitions whose name equals `identifier` after case folding and removing
/// underscores, so `parse config` finds `parseConfig` and `parse_config`.
/// Callers must still verify the live file hash before returning evidence.
pub(crate) fn definitions_by_identifier(
    conn: &Connection,
    identifier: &str,
    limit: usize,
) -> Result<Vec<DefinitionRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT s.path, s.name, s.kind, s.start_line, s.end_line, st.hash
         FROM symbols AS s
         JOIN symbol_state AS st ON st.path = s.path AND st.extractor_revision = ?2
         JOIN files AS f ON f.path = s.path AND f.hash = st.hash
         WHERE lower(replace(s.name, '_', '')) = ?1
         ORDER BY s.path ASC, s.start_line ASC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(
        params![identifier, EXTRACTOR_REVISION, limit as i64],
        definition_row,
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Definitions cached for `path` at content `hash`, in source order.
pub(crate) fn definitions_in_file(
    conn: &Connection,
    path: &str,
    hash: &str,
    limit: usize,
) -> Result<Vec<DefinitionRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT s.path, s.name, s.kind, s.start_line, s.end_line, st.hash
         FROM symbols AS s
         JOIN symbol_state AS st ON st.path = s.path AND st.extractor_revision = ?3
         WHERE s.path = ?1 AND st.hash = ?2
         ORDER BY s.start_line ASC, s.end_line DESC
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        params![path, hash, EXTRACTOR_REVISION, limit as i64],
        definition_row,
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn definition_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DefinitionRow> {
    Ok(DefinitionRow {
        path: row.get(0)?,
        name: row.get(1)?,
        kind: row.get(2)?,
        start_line: usize::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
        end_line: usize::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
        hash: row.get(5)?,
    })
}

/// Definitions in `path` whose span contains `line`, innermost first. Only
/// rows extracted from the given file hash count, so stale spans never apply.
pub(crate) fn containing(
    conn: &Connection,
    path: &str,
    hash: &str,
    line: usize,
) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT s.name, s.kind
         FROM symbols AS s
         JOIN symbol_state AS st ON st.path = s.path AND st.extractor_revision = ?3
         WHERE s.path = ?1 AND st.hash = ?2 AND s.start_line <= ?4 AND s.end_line >= ?4
         ORDER BY s.end_line - s.start_line ASC, s.start_line DESC",
    )?;
    let rows = stmt.query_map(
        params![path, hash, EXTRACTOR_REVISION, line as i64],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub(crate) fn clear(tx: &Transaction<'_>, path: &str) -> Result<()> {
    tx.execute("DELETE FROM symbols WHERE path = ?1", params![path])?;
    tx.execute("DELETE FROM symbol_state WHERE path = ?1", params![path])?;
    Ok(())
}

/// Returns true only when the supported file was actually parsed.
/// Unchanged hashes and extractor revisions bypass Tree-sitter entirely.
pub(crate) fn upsert(tx: &Transaction<'_>, path: &str, hash: &str, source: &str) -> Result<bool> {
    // Unsupported files cannot have symbol rows: a renamed source changes
    // its path and is separately removed by reconciliation. Avoid two DELETE
    // statements for every Markdown/config file on every scan.
    if !findex_syntax::supports_path(Path::new(path)) {
        return Ok(false);
    }
    let cached: Option<(String, i64)> = tx
        .query_row(
            "SELECT hash, extractor_revision FROM symbol_state WHERE path = ?1",
            params![path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if cached.is_some_and(|(old_hash, revision)| old_hash == hash && revision == EXTRACTOR_REVISION)
    {
        return Ok(false);
    }

    let analysis = findex_syntax::analyze(Path::new(path), source)?
        .ok_or("supported language unexpectedly missing its grammar")?;
    clear(tx, path)?;
    tx.execute(
        "INSERT INTO symbol_state(
            path, hash, extractor_revision, language, has_syntax_errors, truncated
        ) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            path,
            hash,
            EXTRACTOR_REVISION,
            analysis.language,
            i64::from(analysis.has_syntax_errors),
            i64::from(analysis.truncated),
        ],
    )?;
    let mut stmt = tx.prepare(
        "INSERT INTO symbols(path, ordinal, name, kind, start_line, end_line)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (ordinal, symbol) in analysis.symbols.iter().enumerate() {
        stmt.execute(params![
            path,
            ordinal as i64,
            symbol.name,
            symbol.kind,
            symbol.start_line as i64,
            symbol.end_line as i64,
        ])?;
    }
    Ok(true)
}

/// Full reconciliation first: safe for standalone CLI and index migrations.
pub fn search_symbols(project: &Path, name: &str, limit: usize) -> Result<SymbolSearchReport> {
    let sync = synchronize(project)?;
    let db = storage_dir(project)?.join("index.sqlite3");
    let mut report = search_symbols_in_db(project, &db, name, limit)?;
    report.sync = sync;
    Ok(report)
}

/// Read a single syntax file *only* after applying the exact same ignore
/// and admission policy as the repository scanner.
///
/// This standalone entry point reconciles first. Watcher-backed callers may
/// use `inspect_indexed_file_in_db` after they have reconciled pending events.
pub fn inspect_indexed_file(
    project: &Path,
    relative: &Path,
) -> Result<Option<findex_syntax::Analysis>> {
    synchronize(project)?;
    let db = storage_dir(project)?.join("index.sqlite3");
    inspect_indexed_file_in_db(project, &db, relative)
}

/// Refuse paths not admitted into the current index. Validate the live digest
/// using the same bytes that are subsequently parsed to avoid stale evidence.
pub fn inspect_indexed_file_in_db(
    project: &Path,
    database: &Path,
    relative: &Path,
) -> Result<Option<findex_syntax::Analysis>> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("expected a normal project-relative file path".into());
    }
    let root = project.canonicalize()?;
    let absolute = root.join(relative);
    if is_disallowed_project_path(relative) || !findex_syntax::supports_path(relative) {
        return Ok(None);
    }
    let key = findex_core::normalized_relative_key(relative)?;
    let conn = connect(database)?;
    let expected: Option<String> = conn
        .query_row(
            "SELECT hash FROM files WHERE path = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;
    let Some(expected) = expected else {
        return Ok(None);
    };
    let metadata = fs::symlink_metadata(&absolute)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_SOURCE_BYTES {
        return Ok(None);
    }
    let resolved = absolute.canonicalize()?;
    if !resolved
        .strip_prefix(&root)
        .ok()
        .is_some_and(|path| !is_disallowed_project_path(path))
    {
        return Ok(None);
    }
    let bytes = fs::read(absolute)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES
        || bytes.contains(&0)
        || blake3::hash(&bytes).to_hex().as_str() != expected
    {
        return Ok(None);
    }
    let Ok(source) = std::str::from_utf8(&bytes) else {
        return Ok(None);
    };
    findex_syntax::analyze(relative, source).map_err(Into::into)
}

/// Cache-only search, used after watcher-backed incremental reconciliation.
/// Even if the watcher misses events, stale results are not returned.
pub fn search_symbols_in_db(
    project: &Path,
    database: &Path,
    name: &str,
    limit: usize,
) -> Result<SymbolSearchReport> {
    let mut report = SymbolSearchReport {
        retrieval: "sqlite-symbol-exact-hash-verified",
        ..SymbolSearchReport::default()
    };
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Ok(report);
    }
    let project = project.canonicalize()?;
    let conn = connect(database)?;
    let mut stmt = conn.prepare(
        "SELECT s.path, s.name, s.kind, st.language,
                s.start_line, s.end_line, st.has_syntax_errors, st.truncated, st.hash
         FROM symbols AS s
         JOIN symbol_state AS st ON st.path = s.path
         JOIN files AS f ON f.path = s.path AND f.hash = st.hash
         WHERE s.name = ?1 COLLATE NOCASE AND st.extractor_revision = ?2
         ORDER BY s.path ASC, s.start_line ASC
         LIMIT ?3",
    )?;
    let max = limit.clamp(1, MAX_RESULTS);
    let rows = stmt.query_map(params![name, EXTRACTOR_REVISION, (max * 4) as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;
    for row in rows {
        let (path, symbol, kind, language, start, end, errors, truncated, digest) = row?;
        if is_disallowed_project_path(Path::new(&path)) {
            report.stale_results_discarded += 1;
            continue;
        }
        let candidate = project.join(&path);
        // Defensive path checking also covers databases from earlier versions.
        let meta = fs::symlink_metadata(&candidate).ok();
        let inside = candidate.canonicalize().ok().is_some_and(|p| {
            p.strip_prefix(&project)
                .ok()
                .is_some_and(|path| !is_disallowed_project_path(path))
        });
        if !meta.is_some_and(|m| m.file_type().is_file() && m.len() <= MAX_SOURCE_BYTES) || !inside
        {
            report.stale_results_discarded += 1;
            continue;
        }
        let Ok(bytes) = fs::read(candidate) else {
            report.stale_results_discarded += 1;
            continue;
        };
        if bytes.len() as u64 > MAX_SOURCE_BYTES || blake3::hash(&bytes).to_hex().as_str() != digest
        {
            report.stale_results_discarded += 1;
            continue;
        }
        report.hits.push(SymbolHit {
            path,
            name: symbol,
            kind,
            language,
            start_line: usize::try_from(start)?,
            end_line: usize::try_from(end)?,
            has_syntax_errors: errors != 0,
            truncated: truncated != 0,
        });
        if report.hits.len() >= max {
            break;
        }
    }
    Ok(report)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{refresh_known_paths_into, synchronize_into};
    use tempfile::tempdir;

    #[test]
    fn file_inspection_obeys_ignore_admission_and_detects_stale_edits() {
        let workspace = tempdir().expect("workspace");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        fs::write(workspace.path().join(".gitignore"), "ignored.rs\n").expect("ignore");
        fs::write(workspace.path().join("ignored.rs"), "fn ignore_me() {}").expect("ignored");
        fs::write(workspace.path().join("allowed.rs"), "fn allowed() {}").expect("allowed");
        synchronize_into(workspace.path(), &db).expect("index");
        assert!(
            inspect_indexed_file_in_db(workspace.path(), &db, Path::new("ignored.rs"))
                .expect("ignored inspection")
                .is_none()
        );
        assert_eq!(
            inspect_indexed_file_in_db(workspace.path(), &db, Path::new("allowed.rs"))
                .expect("allowed inspection")
                .expect("analysis")
                .symbols[0]
                .name,
            "allowed"
        );
        fs::write(workspace.path().join("allowed.rs"), "fn changed() {}").expect("edit");
        assert!(
            inspect_indexed_file_in_db(workspace.path(), &db, Path::new("allowed.rs"))
                .expect("stale inspection")
                .is_none()
        );
        assert!(
            inspect_indexed_file_in_db(workspace.path(), &db, Path::new("../outside.rs")).is_err()
        );
        assert!(
            inspect_indexed_file_in_db(workspace.path(), &db, Path::new("C:\\outside.rs")).is_err()
        );
    }

    #[test]
    fn caches_definitions_and_reuses_identical_source() {
        let workspace = tempdir().expect("workspace");
        let state = tempdir().expect("state");
        let db = state.path().join("db.sqlite3");
        fs::write(
            workspace.path().join("engine.rs"),
            "struct Engine;\nfn ready() {}\n",
        )
        .expect("write");
        let first = synchronize_into(workspace.path(), &db).expect("first");
        assert_eq!(first.symbols_parsed, 1);
        let second = synchronize_into(workspace.path(), &db).expect("second");
        assert_eq!(second.symbols_parsed, 0);
        assert_eq!(second.symbols_reused, 1);
        let hits = search_symbols_in_db(workspace.path(), &db, "ready", 8).expect("lookup");
        assert_eq!(hits.hits.len(), 1);
        assert_eq!(hits.hits[0].start_line, 2);
        assert_eq!(hits.hits[0].kind, "function");
    }

    #[test]
    fn refresh_and_delete_invalidate_definitions() {
        let workspace = tempdir().expect("workspace");
        let state = tempdir().expect("state");
        let db = state.path().join("db.sqlite3");
        let path = workspace.path().join("app.py");
        fs::write(&path, "def original():\n    pass\n").expect("initial");
        synchronize_into(workspace.path(), &db).expect("scan");
        fs::write(&path, "def replacement():\n    pass\n").expect("edit");
        assert!(search_symbols_in_db(workspace.path(), &db, "original", 8)
            .expect("stale")
            .hits
            .is_empty());
        let refreshed =
            refresh_known_paths_into(workspace.path(), &db, &[std::path::PathBuf::from("app.py")])
                .expect("refresh");
        assert_eq!(refreshed.sync.symbols_parsed, 1);
        assert!(search_symbols_in_db(workspace.path(), &db, "original", 8)
            .expect("old")
            .hits
            .is_empty());
        assert_eq!(
            search_symbols_in_db(workspace.path(), &db, "replacement", 8)
                .expect("new")
                .hits
                .len(),
            1
        );
        fs::remove_file(path).expect("delete");
        refresh_known_paths_into(workspace.path(), &db, &[std::path::PathBuf::from("app.py")])
            .expect("refresh deletion");
        assert!(
            search_symbols_in_db(workspace.path(), &db, "replacement", 8)
                .expect("deleted")
                .hits
                .is_empty()
        );
    }

    #[test]
    fn migrates_existing_lexical_index_without_changing_file_hash() {
        let workspace = tempdir().expect("workspace");
        let state = tempdir().expect("state");
        let db = state.path().join("db.sqlite3");
        fs::write(workspace.path().join("legacy.rs"), "fn migrated() {}").expect("source");
        synchronize_into(workspace.path(), &db).expect("initialize");
        let conn = connect(&db).expect("connection");
        conn.execute("DELETE FROM symbol_state", []).expect("state");
        conn.execute("DELETE FROM symbols", []).expect("symbols");
        drop(conn);
        let report = synchronize_into(workspace.path(), &db).expect("backfill");
        assert_eq!(report.symbols_parsed, 1);
        assert_eq!(
            search_symbols_in_db(workspace.path(), &db, "migrated", 8)
                .expect("lookup")
                .hits
                .len(),
            1
        );
    }

    #[test]
    fn never_indexes_ignored_or_secret_named_files() {
        let workspace = tempdir().expect("workspace");
        let state = tempdir().expect("state");
        let db = state.path().join("db.sqlite3");
        fs::write(workspace.path().join(".gitignore"), "ignored.rs\n").expect("ignore");
        fs::write(workspace.path().join("ignored.rs"), "fn blocked() {}").expect("ignored");
        fs::write(
            workspace.path().join("secrets.key"),
            "fn unsafe_symbol() {}",
        )
        .expect("secret");
        fs::write(workspace.path().join("allowed.rs"), "fn allowed() {}").expect("allowed");
        synchronize_into(workspace.path(), &db).expect("scan");
        assert!(search_symbols_in_db(workspace.path(), &db, "blocked", 8)
            .expect("ignored")
            .hits
            .is_empty());
        assert!(
            search_symbols_in_db(workspace.path(), &db, "unsafe_symbol", 8)
                .expect("secret")
                .hits
                .is_empty()
        );
        assert_eq!(
            search_symbols_in_db(workspace.path(), &db, "allowed", 8)
                .expect("allowed")
                .hits
                .len(),
            1
        );
    }
}
