//! SQLite/FTS5-backed *lexical* code discovery. No embeddings or model calls.
//! CLI searches reconcile file contents first; the MCP server refreshes through `findex-watch`.

mod context;
mod overview;
mod refresh;
mod symbols;

pub use context::{context, context_in_db, ContextReport, Evidence};
pub use overview::{overview, InventoryCount, ProjectOverview};
pub use refresh::{refresh_known_paths, refresh_known_paths_into, RefreshReport};
pub use symbols::{
    inspect_indexed_file, inspect_indexed_file_in_db, search_symbols, search_symbols_in_db,
    SymbolHit, SymbolSearchReport,
};

use findex_core::{is_sensitive_filename, storage_dir, MAX_FILES_PER_SYNC, MAX_SOURCE_BYTES};
use ignore::WalkBuilder;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Default, Clone, Serialize)]
pub struct SyncStats {
    pub examined: usize,
    pub indexed: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub skipped: usize,
    pub symbols_parsed: usize,
    pub symbols_reused: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    /// Workspace-relative path, not an arbitrary disk path.
    pub path: String,
    pub line: usize,
    pub excerpt: String,
    /// Lower BM25 numbers mean more relevant lexical matches; not a probability.
    pub bm25_score: f64,
    pub match_kind: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SearchReport {
    pub hits: Vec<SearchHit>,
    pub sync: SyncStats,
    pub stale_results_discarded: usize,
    /// True when a bounded candidate window may have omitted lower-ranked matches.
    pub truncated: bool,
    pub retrieval: &'static str,
}

pub fn synchronize(project: &Path) -> Result<SyncStats> {
    let directory = storage_dir(project)?;
    fs::create_dir_all(&directory)?;
    synchronize_into(project, &directory.join("index.sqlite3"))
}

pub fn search(project: &Path, query: &str, limit: usize) -> Result<SearchReport> {
    let sync = synchronize(project)?;
    let directory = storage_dir(project)?;
    search_in_db(
        project,
        &directory.join("index.sqlite3"),
        query,
        limit,
        sync,
    )
}

/// Lexical schema revision kept in `PRAGMA user_version`.
/// 2: FTS `terms` column with camelCase sub-words and path words.
/// 3: `docs.rowid` = `files.id`, so updates never scan the FTS table by path.
/// 4: FTS5 `secure-delete`, so removed text leaves the index segments.
const LEXICAL_SCHEMA: i64 = 4;

/// A corrupt cache must not be automatically deleted or regenerated:
/// other Findex processes may be using its WAL and automatic deletion can
/// silently erase forensic evidence. Fail closed and let an operator decide
/// when and how to back up and recover this derived database.
fn connect(db: &Path) -> Result<Connection> {
    if let Some(parent) = db.parent() {
        fs::create_dir_all(parent)?;
    }
    match open_schema(db) {
        Err(err) if is_corrupt(err.as_ref()) => Err(
            "Findex index cache is corrupt; original database preserved. Use `findex index-check` and back up the cache before explicit recovery."
                .into(),
        ),
        other => other,
    }
}

fn is_corrupt(err: &(dyn Error + Send + Sync + 'static)) -> bool {
    matches!(
        err.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::SqliteFailure(failure, _))
            if matches!(
                failure.code,
                rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt
            )
    )
}

/// Schema setup can race other processes creating the same new database
/// (WAL switch, CREATE TABLE). SQLite reports such lock-order conflicts as
/// immediate SQLITE_BUSY without consulting the busy handler, so retry.
fn open_schema(db: &Path) -> Result<Connection> {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match try_open_schema(db) {
            Err(err) if is_busy(err.as_ref()) && std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            other => return other,
        }
    }
}

fn is_busy(err: &(dyn Error + Send + Sync + 'static)) -> bool {
    matches!(
        err.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::SqliteFailure(failure, _))
            if matches!(failure.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
}

fn try_open_schema(db: &Path) -> Result<Connection> {
    let mut conn = Connection::open(db)?;
    conn.busy_timeout(Duration::from_secs(30))?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > LEXICAL_SCHEMA {
        // Written by a newer Findex: its layout is unknown here, so neither
        // read nor rewrite it.
        return Err(format!(
            "index cache schema {version} is newer than this Findex supports ({LEXICAL_SCHEMA}); upgrade Findex or delete the index cache"
        )
        .into());
    }
    // Freed pages are zeroed: text of removed or excluded files must not
    // stay recoverable from the cache.
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA secure_delete=ON;")?;
    if version < LEXICAL_SCHEMA {
        // Rebuild lexical rows under a write lock; forgetting file hashes
        // makes the next reconciliation re-ingest every admitted source.
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < LEXICAL_SCHEMA {
            tx.execute_batch(&format!(
                "DROP TABLE IF EXISTS docs;
                 DROP TABLE IF EXISTS files;
                 CREATE TABLE files(
                     id INTEGER PRIMARY KEY,
                     path TEXT UNIQUE NOT NULL,
                     hash TEXT NOT NULL,
                     size_bytes INTEGER NOT NULL
                 );
                 CREATE VIRTUAL TABLE docs USING fts5(
                     path UNINDEXED,
                     body,
                     terms,
                     tokenize='unicode61'
                 );
                 INSERT INTO docs(docs, rank) VALUES('secure-delete', 1);
                 PRAGMA user_version = {LEXICAL_SCHEMA};"
            ))?;
        }
        tx.commit()?;
    }
    symbols::create_schema(&conn)?;
    Ok(conn)
}

/// Explicit operator health check: no implicit reindex, repair, or schema
/// migration. Checks SQLite b-trees, FTS5 segment consistency and the 1:1
/// rowid contract between source metadata and full-text documents.
///
/// This maintenance operation may briefly acquire a write lock for the FTS5
/// integrity-check command. It must not run on every ordinary search.
#[derive(Debug, Serialize)]
pub struct IndexHealth {
    pub schema: i64,
    pub files: usize,
    pub documents: usize,
    pub status: &'static str,
}

pub fn verify_index(project: &Path) -> Result<IndexHealth> {
    let db = storage_dir(project)?.join("index.sqlite3");
    verify_index_into(&db)
}

pub fn verify_index_into(database: &Path) -> Result<IndexHealth> {
    if !database.is_file() {
        return Err("index database absent; run findex index".into());
    }
    // Do not call connect(): a verification tool must not silently replace a
    // damaged cache before it can report what was wrong.
    let conn = Connection::open(database)?;
    conn.busy_timeout(Duration::from_secs(30))?;
    let schema: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if schema != LEXICAL_SCHEMA {
        return Err(format!(
            "expected lexical schema {LEXICAL_SCHEMA}; observed {schema}; no changes made"
        )
        .into());
    }
    let check: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if check != "ok" {
        return Err("SQLite structural integrity check failed".into());
    }
    conn.execute("INSERT INTO docs(docs) VALUES('integrity-check')", [])?;
    let files: i64 = conn.query_row("SELECT count(*) FROM files", [], |row| row.get(0))?;
    let documents: i64 = conn.query_row("SELECT count(*) FROM docs", [], |row| row.get(0))?;
    let dangling: i64 = conn.query_row(
        "SELECT count(*) FROM files f
         LEFT JOIN docs d ON d.rowid=f.id WHERE d.rowid IS NULL",
        [],
        |row| row.get(0),
    )?;
    let orphan: i64 = conn.query_row(
        "SELECT count(*) FROM docs d
         LEFT JOIN files f ON f.id=d.rowid WHERE f.id IS NULL",
        [],
        |row| row.get(0),
    )?;
    if files != documents || dangling != 0 || orphan != 0 {
        return Err("FTS5 documents and source-file metadata disagree".into());
    }
    Ok(IndexHealth {
        schema,
        files: usize::try_from(files)?,
        documents: usize::try_from(documents)?,
        status: "verified-database-structure-and-fts5",
    })
}

/// Extra searchable words that `unicode61` cannot derive on its own:
/// camelCase sub-words of identifiers and the words of the file path.
fn identifier_terms(path: &str, body: &str) -> String {
    let mut words = std::collections::BTreeSet::new();
    for token in body.split(|c: char| !c.is_alphanumeric()).filter(|token| {
        token.chars().any(char::is_uppercase) && token.chars().any(char::is_lowercase)
    }) {
        let parts = camel_parts(token);
        // A single-part identifier is already a body token.
        if parts.len() > 1 {
            words.extend(parts.into_iter().filter(|part| part.chars().count() > 1));
        }
    }
    for token in path
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
    {
        words.insert(token.to_lowercase());
        words.extend(camel_parts(token));
    }
    words.into_iter().collect::<Vec<_>>().join(" ")
}

/// `parseHTTPResponse2` -> ["parse", "http", "response2"].
fn camel_parts(identifier: &str) -> Vec<String> {
    let chars: Vec<char> = identifier.chars().collect();
    let mut parts = Vec::new();
    let mut start = 0;
    for index in 1..chars.len() {
        let boundary = chars[index].is_uppercase()
            && (chars[index - 1].is_lowercase()
                || chars[index - 1].is_ascii_digit()
                || (chars[index - 1].is_uppercase()
                    && chars.get(index + 1).is_some_and(|next| next.is_lowercase())));
        if boundary {
            parts.push(
                chars[start..index]
                    .iter()
                    .collect::<String>()
                    .to_lowercase(),
            );
            start = index;
        }
    }
    parts.push(chars[start..].iter().collect::<String>().to_lowercase());
    parts
}

/// Atomic reconciliation against project files. Paths outside `project` are never indexed.
/// If the file limit is reached, the transaction is rolled back rather than deleting unseen files.
pub fn synchronize_into(project: &Path, database: &Path) -> Result<SyncStats> {
    let project = project.canonicalize()?;
    let mut conn = connect(database)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        // A deferred transaction keeps an unchanged reconcile read-only, so
        // concurrent servers do not queue behind each other. When it must write
        // after another writer committed, SQLite returns SQLITE_BUSY at once
        // (stale snapshot); the whole pass is then retried on a fresh snapshot.
        match synchronize_once(&project, &mut conn) {
            Err(err) if is_busy(err.as_ref()) && std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            other => return other,
        }
    }
}

fn synchronize_once(project: &Path, conn: &mut Connection) -> Result<SyncStats> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)?;
    let existing: HashMap<String, String> = {
        let mut statement = tx.prepare("SELECT path, hash FROM files")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<std::result::Result<HashMap<_, _>, _>>()?
    };
    let mut seen = HashSet::new();
    // Paths that passed every admission check in this walk. Anything else that
    // was indexed before is removed, without re-reading files a second time.
    let mut admitted = HashSet::new();
    let mut stats = SyncStats::default();
    let walker = WalkBuilder::new(project)
        .hidden(true)
        .git_ignore(true)
        // Apply .gitignore rules also in non-Git workspaces (e.g. ZIP projects).
        // Without this, files the user excludes could silently be indexed.
        .require_git(false)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .follow_links(false)
        .filter_entry(|entry| !findex_core::is_excluded_scan_entry(entry.file_name()))
        .build();

    for item in walker {
        // A failed directory walk means an entire subtree may have been hidden.
        // Treat the snapshot as incomplete and roll back: removing all the
        // unseen paths would otherwise silently destroy still-valid index rows.
        let entry = item.map_err(|_| {
            std::io::Error::other("incomplete workspace traversal; previous index preserved")
        })?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if is_sensitive_filename(entry.path()) {
            stats.skipped += 1;
            continue;
        }
        let rel = entry.path().strip_prefix(project)?;
        let rel_string = rel.to_string_lossy().replace('\\', "/");
        if !seen.insert(rel_string.clone()) {
            continue;
        }
        stats.examined += 1;
        if stats.examined > MAX_FILES_PER_SYNC {
            return Err(format!(
                "project exceeds the {}-file safety budget; index transaction cancelled",
                MAX_FILES_PER_SYNC
            )
            .into());
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(err)
                if err
                    .io_error()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
            {
                stats.skipped += 1;
                continue;
            }
            Err(_) if existing.contains_key(&rel_string) => {
                // NTFS/cloud-sync sharing violations are transient. Preserve
                // the cached row, while all consumers still validate its hash.
                admitted.insert(rel_string);
                stats.skipped += 1;
                continue;
            }
            Err(_) => {
                stats.skipped += 1;
                continue;
            }
        };
        if metadata.len() > MAX_SOURCE_BYTES {
            stats.skipped += 1;
            continue;
        }
        let bytes = match fs::read(entry.path()) {
            Ok(b) => b,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound && existing.contains_key(&rel_string)
                {
                    // Locked mid-save: keep the entry; hits re-verify the hash.
                    admitted.insert(rel_string);
                }
                stats.skipped += 1;
                continue;
            }
        };
        if bytes.contains(&0) || bytes.len() as u64 > MAX_SOURCE_BYTES {
            stats.skipped += 1;
            continue;
        }
        let Ok(contents) = std::str::from_utf8(&bytes) else {
            stats.skipped += 1;
            continue;
        };
        admitted.insert(rel_string.clone());
        let digest = blake3::hash(&bytes).to_hex().to_string();
        if existing
            .get(&rel_string)
            .is_some_and(|old| old.as_str() == digest.as_str())
        {
            if symbols::upsert(&tx, &rel_string, &digest, contents)? {
                stats.symbols_parsed += 1;
            } else if findex_syntax::supports_path(rel) {
                stats.symbols_reused += 1;
            }
            stats.unchanged += 1;
            continue;
        }
        if symbols::upsert(&tx, &rel_string, &digest, contents)? {
            stats.symbols_parsed += 1;
        }
        store_document(&tx, &rel_string, contents, &digest, bytes.len() as i64)?;
        stats.indexed += 1;
    }

    // Skipped but previously indexed files must be removed to prevent leaking obsolete results.
    for old_path in existing.keys() {
        if !admitted.contains(old_path) {
            remove_document(&tx, old_path)?;
            symbols::clear(&tx, old_path)?;
            stats.removed += 1;
        }
    }
    tx.commit()?;
    Ok(stats)
}

/// Upsert one file's metadata and lexical row, including derived identifier
/// words. The FTS row is addressed by `files.id`: FTS5 cannot look up its
/// UNINDEXED `path` column without reading every stored document.
pub(crate) fn store_document(
    tx: &rusqlite::Transaction<'_>,
    path: &str,
    contents: &str,
    hash: &str,
    size_bytes: i64,
) -> Result<()> {
    let id: i64 = tx.query_row(
        "INSERT INTO files(path, hash, size_bytes) VALUES(?1, ?2, ?3)
         ON CONFLICT(path) DO UPDATE SET hash=excluded.hash, size_bytes=excluded.size_bytes
         RETURNING id",
        params![path, hash, size_bytes],
        |row| row.get(0),
    )?;
    tx.execute("DELETE FROM docs WHERE rowid = ?1", params![id])?;
    tx.execute(
        "INSERT INTO docs(rowid, path, body, terms) VALUES(?1, ?2, ?3, ?4)",
        params![id, path, contents, identifier_terms(path, contents)],
    )?;
    Ok(())
}

/// Remove one file's metadata and lexical row.
pub(crate) fn remove_document(tx: &rusqlite::Transaction<'_>, path: &str) -> Result<()> {
    let id: Option<i64> = tx
        .query_row(
            "SELECT id FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = id {
        tx.execute("DELETE FROM docs WHERE rowid = ?1", params![id])?;
        tx.execute("DELETE FROM files WHERE id = ?1", params![id])?;
    }
    Ok(())
}

/// Constructs an FTS expression without allowing user input to act as query operators.
fn fts_query(query: &str) -> Option<(String, Vec<String>)> {
    let terms: Vec<String> = query
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .take(12)
        .map(str::to_lowercase)
        .collect();
    if terms.is_empty() {
        return None;
    }
    let fts = terms
        .iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ");
    Some((fts, terms))
}

pub fn search_in_db(
    project: &Path,
    database: &Path,
    query: &str,
    limit: usize,
    sync: SyncStats,
) -> Result<SearchReport> {
    let mut report = SearchReport {
        hits: Vec::new(),
        sync,
        stale_results_discarded: 0,
        truncated: false,
        retrieval: "sqlite-fts5-bm25",
    };
    let Some((expression, terms)) = fts_query(query) else {
        return Ok(report);
    };
    let conn = connect(database)?;
    let canonical_root = project.canonicalize()?;
    let count = limit.clamp(1, 32);
    let mut seen_paths = HashSet::new();

    // Stage 1: verified definitions whose identifier equals the whole query
    // (`Engine`, `parse config` -> parseConfig/parse_config). Definitions name
    // the place a reader needs first; usages follow as lexical evidence.
    let identifier = terms.concat().replace('_', "");
    if identifier.chars().count() >= 3 && identifier.len() <= 256 {
        let mut rows = symbols::definitions_by_identifier(&conn, &identifier, 64)?;
        let exact = query.trim();
        rows.sort_by(|left, right| {
            (
                left.name != exact,
                definition_rank(&left.kind),
                &left.path,
                left.start_line,
            )
                .cmp(&(
                    right.name != exact,
                    definition_rank(&right.kind),
                    &right.path,
                    right.start_line,
                ))
        });
        for row in rows {
            if report.hits.len() >= count {
                report.truncated = true;
                break;
            }
            if seen_paths.contains(&row.path) {
                continue;
            }
            let Some(text) = live_source(&canonical_root, &row.path, &row.hash) else {
                // Marked seen so the lexical stage does not count it twice.
                seen_paths.insert(row.path);
                report.stale_results_discarded += 1;
                continue;
            };
            let excerpt = text
                .lines()
                .nth(row.start_line.saturating_sub(1))
                .map(excerpt_of)
                .unwrap_or_default();
            seen_paths.insert(row.path.clone());
            report.hits.push(SearchHit {
                path: row.path,
                line: row.start_line,
                excerpt,
                bm25_score: 0.0,
                match_kind: "definition",
            });
        }
    }
    let remaining = count - report.hits.len();
    if remaining == 0 {
        return Ok(report);
    }

    // Stage 2: BM25 candidates over source text and identifier/path words.
    let mut stmt = conn.prepare(
        "SELECT docs.path, docs.body, docs.rank, files.hash
         FROM docs JOIN files ON files.id = docs.rowid
         WHERE docs MATCH ?1 ORDER BY docs.rank ASC, docs.path ASC LIMIT ?2",
    )?;
    // Collect a bounded but generous window before reranking. An extra
    // candidate makes truncation explicit rather than silently claiming
    // completeness after stale-result rejection or multi-term reranking.
    let candidate_budget = (count * 16).clamp(32, 512);
    let result_rows =
        stmt.query_map(params![expression, (candidate_budget + 1) as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
    let mut candidates = Vec::new();
    for (candidate_index, row) in result_rows.enumerate() {
        if candidate_index >= candidate_budget {
            report.truncated = true;
            break;
        }
        let (path, body, score, indexed_hash) = row?;
        if seen_paths.contains(&path) {
            continue;
        }
        let folded_body = body.to_lowercase();
        let coverage = terms
            .iter()
            .filter(|term| folded_body.contains(term.as_str()))
            .count();
        // A snippet from the first matching line often shows the wrong
        // function. Prefer the line covering the most query terms.
        // Without any matching line, cite line 1 with its real text.
        let mut best = (
            0_usize,
            1_usize,
            excerpt_of(body.lines().next().unwrap_or("")),
        );
        for (line_index, line) in body.lines().enumerate() {
            let folded_line = line.to_lowercase();
            let matches = terms
                .iter()
                .filter(|term| line_has_term(&folded_line, term))
                .count();
            if matches > best.0 {
                best = (matches, line_index + 1, excerpt_of(line));
            }
        }
        candidates.push((
            (coverage, best.0),
            SearchHit {
                path,
                line: best.1,
                excerpt: best.2,
                bm25_score: score,
                match_kind: "lexical",
            },
            indexed_hash,
        ));
    }
    // File coverage first, then same-line co-occurrence (proximity), then BM25.
    candidates.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.bm25_score.total_cmp(&right.1.bm25_score))
            .then_with(|| left.1.path.cmp(&right.1.path))
    });
    // Verify only what is returned: hashing every ranked candidate costs one
    // file read per candidate. A stale candidate is skipped, never returned.
    for (_, hit, indexed_hash) in candidates {
        if report.hits.len() == count {
            report.truncated = true;
            break;
        }
        if live_source(&canonical_root, &hit.path, &indexed_hash).is_none() {
            report.stale_results_discarded += 1;
            continue;
        }
        report.hits.push(hit);
    }
    Ok(report)
}

/// Literal match, or the phrase FTS matched: `to_regex` is the tokens `to regex`.
fn line_has_term(folded_line: &str, term: &str) -> bool {
    if folded_line.contains(term) {
        return true;
    }
    if term.chars().all(char::is_alphanumeric) {
        return false; // a single token: the literal check was exact
    }
    let tokenize = |text: &str| -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let phrase = tokenize(term);
    phrase.len() > 1
        && tokenize(folded_line)
            .windows(phrase.len())
            .any(|window| window == phrase.as_slice())
}

/// Types and free functions first; trait/impl members and impl blocks after.
fn definition_rank(kind: &str) -> u8 {
    match kind {
        "type" | "class" => 0,
        "function" | "module" | "macro" | "constant" => 1,
        "method" => 2,
        _ => 3,
    }
}

/// Trimmed, length-bounded and secret-masked evidence line.
fn excerpt_of(line: &str) -> String {
    let trimmed: String = line.trim().chars().take(240).collect();
    findex_core::redact_secrets(&trimmed).into_owned()
}

/// Current file text, only if it is a regular in-root file whose bytes still
/// hash to the indexed digest. Symlinks, junction escapes and edits fail closed.
fn live_source(root: &Path, relative: &str, indexed_hash: &str) -> Option<String> {
    let full_path: PathBuf = root.join(relative);
    let metadata = fs::symlink_metadata(&full_path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_SOURCE_BYTES {
        return None;
    }
    if !full_path.canonicalize().ok()?.starts_with(root) {
        return None;
    }
    let bytes = fs::read(&full_path).ok()?;
    if blake3::hash(&bytes).to_hex().as_str() != indexed_hash {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn finds_code_and_handles_edits_and_removal() {
        let tmp = tempdir().expect("temp dir");
        let src = tmp.path().join("src");
        fs::create_dir(&src).expect("make src");
        let target = src.join("auth.rs");
        fs::write(&target, "fn validate_token() {}\n").expect("write");
        let state = tempdir().expect("state dir");
        let db = state.path().join("db.sqlite3");
        let first = synchronize_into(tmp.path(), &db).expect("sync");
        assert!(first.indexed >= 1);
        let found = search_in_db(tmp.path(), &db, "validate_token", 5, SyncStats::default())
            .expect("search");
        assert_eq!(found.hits.len(), 1);
        assert_eq!(found.hits[0].line, 1);
        fs::write(&target, "fn verify_identity() {}\n").expect("edit");
        synchronize_into(tmp.path(), &db).expect("resync");
        let stale = search_in_db(tmp.path(), &db, "validate_token", 5, SyncStats::default())
            .expect("search");
        assert!(stale.hits.is_empty());
        fs::remove_file(&target).expect("remove");
        let after = synchronize_into(tmp.path(), &db).expect("clean");
        assert!(after.removed >= 1);
    }

    #[test]
    fn excludes_secrets_and_binary_files() {
        let tmp = tempdir().expect("temp dir");
        fs::write(tmp.path().join(".env"), "password_secret_marker").expect("env");
        fs::write(tmp.path().join("binary.bin"), [65, 0, 66]).expect("bin");
        fs::write(tmp.path().join("src.txt"), "nonsecretmarker").expect("src");
        let state = tempdir().expect("state dir");
        let db = state.path().join("index.sqlite3");
        synchronize_into(tmp.path(), &db).expect("sync");
        assert!(search_in_db(
            tmp.path(),
            &db,
            "password_secret_marker",
            5,
            SyncStats::default()
        )
        .expect("search")
        .hits
        .is_empty());
        assert_eq!(
            search_in_db(tmp.path(), &db, "nonsecretmarker", 5, SyncStats::default())
                .expect("search")
                .hits
                .len(),
            1
        );
    }

    #[test]
    fn prefers_files_and_excerpts_covering_more_query_terms() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        fs::write(project.path().join("one.rs"), "fn alpha() {}\n").expect("first");
        fs::write(
            project.path().join("both.rs"),
            "fn alpha() {}\nfn alpha_beta() {}\n",
        )
        .expect("second");
        synchronize_into(project.path(), &db).expect("sync");
        let found = search_in_db(project.path(), &db, "alpha beta", 2, SyncStats::default())
            .expect("search");
        assert_eq!(found.hits[0].path, "both.rs");
        assert_eq!(found.hits[0].line, 2);
        assert!(found.hits[0].excerpt.contains("alpha_beta"));
    }

    #[test]
    fn advertises_when_bounded_candidate_window_is_truncated() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        for number in 0..35 {
            fs::write(
                project.path().join(format!("file_{number}.rs")),
                "fn marker() {}\n",
            )
            .expect("fixture");
        }
        synchronize_into(project.path(), &db).expect("sync");
        let found =
            search_in_db(project.path(), &db, "marker", 1, SyncStats::default()).expect("search");
        assert_eq!(found.hits.len(), 1);
        assert!(found.truncated);
    }

    fn indexed(files: &[(&str, &str)]) -> (tempfile::TempDir, tempfile::TempDir, PathBuf) {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        for (name, body) in files {
            let path = project.path().join(name);
            fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
            fs::write(path, body).expect("fixture");
        }
        let db = state.path().join("index.sqlite3");
        synchronize_into(project.path(), &db).expect("sync");
        (project, state, db)
    }

    #[test]
    fn camel_case_identifiers_match_split_query_words() {
        let (project, _state, db) = indexed(&[
            (
                "config.ts",
                "// loader\nexport function parseConfigFile() {}\n",
            ),
            ("other.ts", "export const value = 1;\n"),
        ]);
        let found = search_in_db(
            project.path(),
            &db,
            "parse config file",
            5,
            SyncStats::default(),
        )
        .expect("search");
        assert_eq!(found.hits[0].path, "config.ts");
        assert_eq!(found.hits[0].line, 2);
    }

    #[test]
    fn definition_outranks_usages_and_points_at_definition_line() {
        let usage = "use crate::Engine;\nfn a(e: Engine) -> Engine { e }\nfn b(e: Engine) -> Engine { e }\n";
        let (project, _state, db) = indexed(&[
            (
                "src/engine.rs",
                "// The core engine.\n// It runs.\npub struct Engine {\n    id: u32,\n}\n",
            ),
            ("src/a.rs", usage),
            ("src/b.rs", usage),
            ("src/c.rs", usage),
        ]);
        let found =
            search_in_db(project.path(), &db, "Engine", 5, SyncStats::default()).expect("search");
        assert_eq!(found.hits[0].path, "src/engine.rs");
        assert_eq!(found.hits[0].line, 3);
        assert_eq!(found.hits[0].match_kind, "definition");
        assert_eq!(
            found.hits.len(),
            4,
            "usages are still returned as lexical evidence"
        );
        assert!(found.hits[1..]
            .iter()
            .all(|hit| hit.match_kind == "lexical"));
    }

    #[test]
    fn word_query_finds_snake_case_definition_before_comment_mentions() {
        let (project, _state, db) = indexed(&[
            (
                "notes.py",
                "# load user profile\n# load user profile again\n# load user profile\n",
            ),
            (
                "users.py",
                "import os\n\n\ndef load_user_profile(uid):\n    return uid\n",
            ),
        ]);
        let found = search_in_db(
            project.path(),
            &db,
            "load user profile",
            5,
            SyncStats::default(),
        )
        .expect("search");
        assert_eq!(found.hits[0].path, "users.py");
        assert_eq!(found.hits[0].line, 4);
    }

    #[test]
    fn same_line_co_occurrence_breaks_file_coverage_ties() {
        let filler = "let unrelated = 1;\n".repeat(40);
        let (project, _state, db) = indexed(&[
            ("spread.rs", "// retry\n// budget\n// exhausted\n"),
            (
                "together.rs",
                &format!("{filler}// retry budget exhausted here\n{filler}"),
            ),
        ]);
        let found = search_in_db(
            project.path(),
            &db,
            "retry budget exhausted",
            5,
            SyncStats::default(),
        )
        .expect("search");
        assert_eq!(found.hits[0].path, "together.rs");
        assert_eq!(found.hits[0].line, 41);
    }

    #[test]
    fn index_integrity_detects_missing_fts_rows_without_repairing_them() {
        let (project, _state, db) = indexed(&[
            ("a.rs", "fn keep_me() {}\n"),
            ("b.rs", "fn call_me() { keep_me(); }\n"),
        ]);
        let clean = verify_index_into(&db).expect("healthy index");
        assert_eq!(clean.files, 2);
        assert_eq!(clean.documents, 2);
        let conn = Connection::open(&db).expect("db");
        let id: i64 = conn
            .query_row("SELECT id FROM files WHERE path='a.rs'", [], |row| {
                row.get(0)
            })
            .expect("file id");
        conn.execute("DELETE FROM docs WHERE rowid=?1", params![id])
            .expect("remove one FTS record");
        drop(conn);
        assert!(verify_index_into(&db).is_err());
        // Verification never rewrites, rebuilds or hides the corruption.
        let raw = Connection::open(&db).expect("original preserved");
        let remaining: i64 = raw
            .query_row("SELECT count(*) FROM docs", [], |row| row.get(0))
            .expect("count");
        assert_eq!(remaining, 1);
        drop(raw);
        synchronize_into(project.path(), &db).expect("reconcile");
        // An unchanged reconcile alone does not repair a missing FTS row.
        assert!(verify_index_into(&db).is_err());
    }

    #[test]
    fn corrupt_index_database_is_preserved_and_fails_closed() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        fs::write(project.path().join("lib.rs"), "fn recovered_marker() {}\n").expect("source");
        let db = state.path().join("index.sqlite3");
        let original = b"this is not a sqlite database, it is garbage".repeat(200);
        fs::write(&db, &original).expect("garbage");
        let err = synchronize_into(project.path(), &db).expect_err("must fail closed");
        assert!(
            err.to_string().contains("original database preserved"),
            "{err}"
        );
        assert_eq!(fs::read(&db).expect("preserved"), original);
        assert!(
            verify_index_into(&db).is_err(),
            "corrupt cache must remain visible"
        );
    }

    /// Text of a deleted file must not stay recoverable from the cache files
    /// (SQLite free pages, FTS5 segments, WAL frames).
    #[test]
    fn removed_file_text_is_not_left_in_index_bytes() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        let marker = "zq_removed_marker_8841";
        fs::write(project.path().join("keep.rs"), "fn kept() {}\n").expect("keep");
        let secret = project.path().join("secret.rs");
        fs::write(
            &secret,
            format!("fn {marker}() {{}}\n// {marker} body text\n"),
        )
        .expect("secret");
        synchronize_into(project.path(), &db).expect("sync");
        fs::remove_file(&secret).expect("delete");
        let stats = synchronize_into(project.path(), &db).expect("sync after delete");
        assert_eq!(stats.removed, 1);
        for suffix in ["", "-wal"] {
            let mut path = db.as_os_str().to_owned();
            path.push(suffix);
            let bytes = fs::read(PathBuf::from(path)).unwrap_or_default();
            assert!(
                !bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
                "marker left in index{suffix}"
            );
        }
    }

    /// After a downgrade, an index written by a newer Findex is refused
    /// instead of being read with an older idea of its schema.
    #[test]
    fn newer_index_schema_is_refused_and_left_intact() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        fs::write(project.path().join("lib.rs"), "fn marker() {}\n").expect("source");
        let db = state.path().join("index.sqlite3");
        Connection::open(&db)
            .expect("db")
            .execute_batch(&format!("PRAGMA user_version = {};", LEXICAL_SCHEMA + 1))
            .expect("newer version");
        let err = synchronize_into(project.path(), &db).expect_err("newer schema refused");
        assert!(err.to_string().contains("newer"), "{err}");
        let version: i64 = Connection::open(&db)
            .expect("db")
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, LEXICAL_SCHEMA + 1);
    }

    #[test]
    fn legacy_lexical_schema_is_migrated_and_reindexed() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        fs::write(
            project.path().join("a.ts"),
            "function loadUserProfile() {}\n",
        )
        .expect("source");
        let db = state.path().join("index.sqlite3");
        {
            let conn = Connection::open(&db).expect("legacy db");
            conn.execute_batch(
                "CREATE TABLE files(path TEXT PRIMARY KEY NOT NULL, hash TEXT NOT NULL, size_bytes INTEGER NOT NULL);
                 CREATE VIRTUAL TABLE docs USING fts5(path UNINDEXED, body, tokenize='unicode61');",
            )
            .expect("legacy schema");
        }
        synchronize_into(project.path(), &db).expect("migrate");
        let found = search_in_db(project.path(), &db, "user profile", 5, SyncStats::default())
            .expect("search");
        assert_eq!(found.hits.len(), 1);
    }

    /// FTS5 cannot look up its UNINDEXED `path` column, so deleting a document
    /// by path scans every stored body: quadratic I/O on a cold index. Rows are
    /// therefore keyed by `files.id`; this pins that invariant through edits,
    /// targeted refreshes and removals.
    #[test]
    fn lexical_rows_are_keyed_by_file_id_through_edits_refreshes_and_removals() {
        let (project, _state, db) = indexed(&[
            ("a.rs", "fn alpha_marker() {}\n"),
            ("b.rs", "fn beta_marker() {}\n"),
            ("c.rs", "fn gamma_marker() {}\n"),
        ]);
        fs::write(project.path().join("a.rs"), "fn alpha_edited() {}\n").expect("edit");
        fs::remove_file(project.path().join("b.rs")).expect("remove");
        synchronize_into(project.path(), &db).expect("resync");
        fs::write(project.path().join("c.rs"), "fn gamma_refreshed() {}\n").expect("edit");
        refresh_known_paths_into(project.path(), &db, &[PathBuf::from("c.rs")]).expect("refresh");

        let conn = Connection::open(&db).expect("db");
        let orphans: i64 = conn
            .query_row(
                "SELECT count(*) FROM docs WHERE rowid NOT IN (SELECT id FROM files)",
                [],
                |row| row.get(0),
            )
            .expect("orphans");
        let (docs, files): (i64, i64) = conn
            .query_row(
                "SELECT (SELECT count(*) FROM docs), (SELECT count(*) FROM files)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("counts");
        assert_eq!((orphans, docs, files), (0, 2, 2));
        for (query, path) in [("alpha edited", "a.rs"), ("gamma refreshed", "c.rs")] {
            let found =
                search_in_db(project.path(), &db, query, 5, SyncStats::default()).expect("search");
            assert_eq!(found.hits.first().map(|h| h.path.as_str()), Some(path));
        }
        let gone = search_in_db(project.path(), &db, "beta marker", 5, SyncStats::default())
            .expect("search");
        assert!(gone.hits.is_empty(), "{:?}", gone.hits);
    }

    /// Several MCP servers reconcile the same index on start-up. An unchanged
    /// project must not queue behind another connection's write lock.
    #[test]
    fn unchanged_reconcile_does_not_wait_for_another_writer() {
        let (project, _state, db) = indexed(&[("a.rs", "fn alpha() {}\n")]);
        let mut holder = Connection::open(&db).expect("holder");
        let lock = holder
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("write lock");
        let started = std::time::Instant::now();
        let stats = synchronize_into(project.path(), &db).expect("read-only reconcile");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        assert_eq!((stats.unchanged, stats.indexed), (1, 0));
        drop(lock);
    }

    /// FTS splits `to_regex` into `to` + `regex`; a file matching only those
    /// tokens must still cite a real line, never line 1 with an empty excerpt.
    #[test]
    fn token_only_matches_cite_the_line_holding_the_tokens() {
        let (project, _state, db) = indexed(&[(
            "glob.rs",
            "//! Globs.\n\nfn build() {}\n\n// convert glob to regex here\nfn convert() {}\n",
        )]);
        let found =
            search_in_db(project.path(), &db, "to_regex", 5, SyncStats::default()).expect("search");
        let hit = found.hits.first().expect("hit");
        assert_eq!(
            (hit.line, hit.excerpt.as_str()),
            (5, "// convert glob to regex here")
        );
    }

    #[test]
    fn excerpts_redact_common_secret_shapes() {
        let (project, _state, db) = indexed(&[(
            "settings.py",
            "API_KEY = \"AKIAIOSFODNN7EXAMPLE\"\npassword = \"hunter2hunter2\"\n",
        )]);
        let found =
            search_in_db(project.path(), &db, "API_KEY", 5, SyncStats::default()).expect("search");
        assert!(
            found.hits[0].excerpt.contains("[REDACTED"),
            "{}",
            found.hits[0].excerpt
        );
        assert!(!found.hits[0].excerpt.contains("AKIAIOSFODNN7EXAMPLE"));
        let found =
            search_in_db(project.path(), &db, "password", 5, SyncStats::default()).expect("search");
        assert!(!found.hits[0].excerpt.contains("hunter2hunter2"));
    }

    #[test]
    fn neutralizes_fts_operators() {
        let parsed = fts_query("hello OR world* (secret)").expect("terms");
        assert!(!parsed.0.contains('*'));
        assert!(parsed.0.contains("\"or\""));
    }
}
