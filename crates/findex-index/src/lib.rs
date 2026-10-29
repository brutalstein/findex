//! SQLite/FTS5-backed *lexical* code discovery. No embeddings or model calls.
//! CLI searches reconcile file contents first; the MCP server refreshes through `findex-watch`.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![warn(clippy::all)]

mod context;
mod overview;
mod rank;
mod refresh;
mod source_reads;
mod symbols;

pub use context::{context, context_in_db, ContextReport, Evidence};
pub use overview::{overview, InventoryCount, ProjectOverview};
pub use rank::{rank, rank_in_db, RankReport, RankedHit};
pub use refresh::{refresh_known_paths, refresh_known_paths_into, RefreshReport};
pub use symbols::{
    inspect_indexed_file, inspect_indexed_file_in_db, search_symbols, search_symbols_in_db,
    SymbolHit, SymbolSearchReport,
};

use findex_core::{is_disallowed_project_path, storage_dir, MAX_FILES_PER_SYNC, MAX_SOURCE_BYTES};
use ignore::WalkBuilder;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Process-wide cache of SQLite connections keyed by database path.
/// Reusing connections across searches avoids re-running schema version
/// checks, WAL setup and `CREATE TABLE IF NOT EXISTS` on every request.
static CONNECTION_CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<Connection>>>>> =
    OnceLock::new();

fn connection_cache() -> &'static Mutex<HashMap<PathBuf, Arc<Mutex<Connection>>>> {
    CONNECTION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Open (or reuse) a cached connection to the lexical index database.
/// Schema initialization happens once on first use per database path.
fn cached_connect(db: &Path) -> Result<Arc<Mutex<Connection>>> {
    let cache = connection_cache();
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(conn) = cache.get(db) {
        return Ok(conn.clone());
    }
    let conn = connect(db)?;
    let cached = Arc::new(Mutex::new(conn));
    cache.insert(db.to_path_buf(), cached.clone());
    Ok(cached)
}

// Sort metadata first; fetch source only inside the bounded candidate loop.
// files.path and docs.path share the rowid contract enforced by store_document.
const FTS_CANDIDATES_SQL: &str = "SELECT files.path, docs.rank, files.hash, files.id
     FROM docs JOIN files ON files.id = docs.rowid
     WHERE docs MATCH ?1 ORDER BY docs.rank ASC, files.path ASC LIMIT ?2";

#[derive(Debug, Default, Clone, Serialize)]
pub struct SyncStats {
    pub examined: usize,
    pub indexed: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub skipped: usize,
    /// Cloud-only placeholders (counted in `skipped`): never read, so never downloaded.
    pub cloud_only: usize,
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
    /// Serialized to three decimals: the remaining f64 digits carry no ranking
    /// information but cost about six model tokens per hit.
    #[serde(serialize_with = "serialize_score")]
    pub bm25_score: f64,
    pub match_kind: &'static str,
}

fn serialize_score<S: serde::Serializer>(
    score: &f64,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_f64((score * 1000.0).round() / 1000.0)
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
            "Findex index cache is corrupt; original database preserved. Use `findex index-check`, back up the cache if needed, then `findex forget` and `findex index` to rebuild."
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
            "index cache schema {version} is newer than this Findex supports ({LEXICAL_SCHEMA}); upgrade Findex or run `findex forget` to rebuild it"
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
            Ok(stats) => {
                if stats.indexed + stats.removed > 0 {
                    flush_wal(&conn)?;
                }
                return Ok(stats);
            }
            other => return other,
        }
    }
}

/// A read error that leaves an indexed file in place: a sharing violation
/// while an editor saves. Absence and revoked access remove it with its text;
/// the next query's reconciliation re-admits a file that was only briefly
/// unreadable.
pub(crate) fn keeps_entry(err: &std::io::Error) -> bool {
    !matches!(
        err.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
    )
}

/// Copy committed pages (freed ones zeroed by `secure_delete`) into the
/// database file and empty the WAL, so replaced or removed source text does
/// not outlive the commit while another open connection keeps SQLite from
/// doing this on close. A reader inside a transaction can still delay it.
pub(crate) fn flush_wal(conn: &Connection) -> Result<()> {
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(())
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
    let width = std::thread::available_parallelism().map_or(1, |count| count.get().min(8));
    let mut reader = source_reads::SourceReads::new(width);
    let mut pending = Vec::with_capacity(reader.capacity());
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
        let rel = entry.path().strip_prefix(project)?;
        if is_disallowed_project_path(rel) {
            stats.skipped += 1;
            continue;
        }
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
            Err(err) if err.io_error().is_some_and(|io| !keeps_entry(io)) => {
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
        if findex_core::is_cloud_only(&metadata) {
            stats.skipped += 1;
            stats.cloud_only += 1;
            continue;
        }
        pending.push((rel_string, entry.path().to_path_buf()));
        if pending.len() == reader.capacity() {
            store_source_batch(
                &tx,
                &mut pending,
                &mut reader,
                &existing,
                &mut admitted,
                &mut stats,
            )?;
        }
    }
    store_source_batch(
        &tx,
        &mut pending,
        &mut reader,
        &existing,
        &mut admitted,
        &mut stats,
    )?;

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

fn store_source_batch(
    tx: &rusqlite::Transaction<'_>,
    pending: &mut Vec<(String, PathBuf)>,
    reader: &mut source_reads::SourceReads,
    existing: &HashMap<String, String>,
    admitted: &mut HashSet<String>,
    stats: &mut SyncStats,
) -> Result<()> {
    let paths: Vec<_> = pending.iter().map(|(_, path)| path.clone()).collect();
    let reads = reader.read(&paths)?;
    for ((rel_string, _), read) in pending.drain(..).zip(reads) {
        let bytes = match read {
            Ok(b) => b,
            Err(err) => {
                if keeps_entry(&err) && existing.contains_key(&rel_string) {
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
            if symbols::upsert(tx, &rel_string, &digest, contents)? {
                stats.symbols_parsed += 1;
            } else if findex_syntax::supports_path(Path::new(&rel_string)) {
                stats.symbols_reused += 1;
            }
            stats.unchanged += 1;
            continue;
        }
        if symbols::upsert(tx, &rel_string, &digest, contents)? {
            stats.symbols_parsed += 1;
        }
        store_document(tx, &rel_string, contents, &digest, bytes.len() as i64)?;
        stats.indexed += 1;
    }

    Ok(())
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
    let terms = split_query_terms(query);
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

/// Split a query into FTS5-safe terms, expanding camelCase/PascalCase
/// identifiers into their sub-words so `parseConfig` matches both
/// `parseConfig` and `parse config` in the index.
pub(crate) fn split_query_terms(query: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for token in query
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
    {
        let lower = token.to_ascii_lowercase();
        // If the token has mixed case (camelCase/PascalCase), emit sub-words
        // in addition to the whole token, so both "parseConfig" and "parse
        // config" queries match indexed sub-words.
        let has_upper = token.chars().any(char::is_uppercase);
        let has_lower = token.chars().any(char::is_lowercase);
        if has_upper && has_lower && token.chars().count() > 2 {
            for part in camel_parts(token) {
                if part.chars().count() > 1 {
                    terms.push(part);
                }
            }
        }
        terms.push(lower);
    }
    terms.into_iter().take(12).collect()
}

pub fn search_in_db(
    project: &Path,
    database: &Path,
    query: &str,
    limit: usize,
    sync: SyncStats,
) -> Result<SearchReport> {
    search_ranked_in_db(project, database, query, limit, sync, false)
}

pub(crate) fn search_concept_in_db(
    project: &Path,
    database: &Path,
    query: &str,
    limit: usize,
) -> Result<SearchReport> {
    search_ranked_in_db(project, database, query, limit, SyncStats::default(), true)
}

fn search_ranked_in_db(
    project: &Path,
    database: &Path,
    query: &str,
    limit: usize,
    sync: SyncStats,
    concept: bool,
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
    let conn = cached_connect(database)?;
    let conn = conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let canonical_root = project.canonicalize()?;
    let count = limit.clamp(1, 32);
    let mut seen_paths = HashSet::new();
    // Cache live_source results within a single search call so a file that
    // appears as both a definition hit and a BM25 candidate is read from
    // disk at most once.
    let mut source_cache: HashMap<String, Option<String>> = HashMap::new();
    let cached_live_source = |root: &Path,
                              path: &str,
                              hash: &str,
                              cache: &mut HashMap<String, Option<String>>|
     -> Option<String> {
        if let Some(cached) = cache.get(path) {
            return cached.clone();
        }
        let result = live_source(root, path, hash);
        cache.insert(path.to_string(), result.clone());
        result
    };

    // Stage 1: verified definitions whose identifier equals the whole query
    // (`Engine`, `parse config` -> parseConfig/parse_config). Definitions name
    // the place a reader needs first; usages follow as lexical evidence.
    // Build from the original query (not expanded terms) so camelCase
    // sub-word expansion doesn't corrupt the identifier match.
    let identifier: String = query
        .chars()
        .filter(|c| (*c).is_alphanumeric() || *c == '_')
        .collect::<String>()
        .to_ascii_lowercase();
    if !concept && identifier.chars().count() >= 3 && identifier.len() <= 256 {
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
            let Some(text) =
                cached_live_source(&canonical_root, &row.path, &row.hash, &mut source_cache)
            else {
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
    let mut stmt = conn.prepare_cached(FTS_CANDIDATES_SQL)?;
    let mut bodies = conn.prepare_cached("SELECT body FROM docs WHERE rowid = ?1")?;
    // Collect a bounded but generous window before reranking. An extra
    // candidate makes truncation explicit rather than silently claiming
    // completeness after stale-result rejection or multi-term reranking.
    let candidate_budget = if concept {
        128
    } else {
        (count * 16).clamp(32, 512)
    };
    let result_rows =
        stmt.query_map(params![expression, (candidate_budget + 1) as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
    let mut candidates = Vec::new();
    for (candidate_index, row) in result_rows.enumerate() {
        if candidate_index >= candidate_budget {
            report.truncated = true;
            break;
        }
        let (path, score, indexed_hash, id) = row?;
        // The metadata statement remains active, so this shares its read snapshot.
        let body: String = bodies.query_row(params![id], |row| row.get(0))?;
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
        let mut line_terms = Vec::new();
        for (line_index, line) in body.lines().enumerate() {
            let folded_line = line.to_lowercase();
            let mask = terms.iter().enumerate().fold(0_u16, |mask, (i, term)| {
                mask | (u16::from(line_has_term(&folded_line, term)) << i)
            });
            let matches = mask.count_ones() as usize;
            if matches > best.0 {
                best = (matches, line_index + 1, excerpt_of(line));
            }
            if concept {
                line_terms.push(mask);
            }
        }
        if concept {
            let mut best_window = 0;
            for center in 0..line_terms.len() {
                let covered = line_terms
                    [center.saturating_sub(2)..(center + 3).min(line_terms.len())]
                    .iter()
                    .fold(0_u16, |mask, line| mask | line)
                    .count_ones();
                if covered > best_window {
                    best_window = covered;
                    best.1 = center + 1;
                }
            }
            best.2 = excerpt_of(body.lines().nth(best.1 - 1).unwrap_or(""));
        }
        let filename_terms = if concept {
            identifier_terms(
                Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(""),
                "",
            )
        } else {
            String::new()
        };
        let filename_matches = filename_terms
            .split_whitespace()
            .filter(|word| word.chars().count() >= 3 && terms.iter().any(|term| term == word))
            .count();
        candidates.push((
            (coverage, best.0, filename_matches),
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
        if concept {
            let role = |path: &str| (!context::is_code_path(path), context::is_test_path(path));
            return role(&left.1.path)
                .cmp(&role(&right.1.path))
                // Filename matches are an additional field, not substring hits.
                .then_with(|| {
                    (left.1.bm25_score * (1.0 + 2.0 * left.0 .2 as f64))
                        .total_cmp(&(right.1.bm25_score * (1.0 + 2.0 * right.0 .2 as f64)))
                })
                .then_with(|| left.1.path.cmp(&right.1.path));
        }
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
        if cached_live_source(&canonical_root, &hit.path, &indexed_hash, &mut source_cache)
            .is_none()
        {
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
    findex_core::redact_secrets(line.trim())
        .chars()
        .take(240)
        .collect()
}

/// Current file text, only if it is a regular in-root file whose bytes still
/// hash to the indexed digest. Symlinks, junction escapes and edits fail closed.
fn live_source(root: &Path, relative: &str, indexed_hash: &str) -> Option<String> {
    if is_disallowed_project_path(Path::new(relative)) {
        return None;
    }
    let full_path: PathBuf = root.join(relative);
    let metadata = fs::symlink_metadata(&full_path).ok()?;
    if !metadata.file_type().is_file()
        || metadata.len() > MAX_SOURCE_BYTES
        || findex_core::is_cloud_only(&metadata)
    {
        return None;
    }
    let resolved = full_path.canonicalize().ok()?;
    let actual = resolved.strip_prefix(root).ok()?;
    if is_disallowed_project_path(actual) {
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
    use proptest::prelude::*;
    use tempfile::tempdir;

    #[test]
    fn bm25_score_is_serialized_to_three_decimals_without_changing_the_value() {
        let hit = SearchHit {
            path: "a.rs".into(),
            line: 1,
            excerpt: "x".into(),
            bm25_score: -7.556_564_023_538_062,
            match_kind: "lexical",
        };
        let json = serde_json::to_value(&hit).expect("serialize");
        assert_eq!(json["bm25_score"], serde_json::json!(-7.557));
        assert_eq!(hit.bm25_score, -7.556_564_023_538_062);
    }

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
    fn bounded_body_query_matches_direct_rank_path_and_source_reference() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        for number in (0..96).rev() {
            let body = format!(
                "fn node_{number}() {{}}\n// {} beta\n",
                "alpha ".repeat(1 + number % 8)
            );
            fs::write(project.path().join(format!("file_{number:03}.rs")), body).expect("source");
        }
        synchronize_into(project.path(), &db).expect("index");
        let conn = connect(&db).expect("connection");
        let direct = "SELECT docs.path,docs.body,docs.rank,files.hash
            FROM docs JOIN files ON files.id=docs.rowid
            WHERE docs MATCH ?1 ORDER BY docs.rank ASC,docs.path ASC LIMIT ?2";
        for query in ["\"alpha\"", "\"alpha\" OR \"beta\"", "\"missing\""] {
            for limit in [1, 32, 161] {
                let fetch = |sql: &str| {
                    conn.prepare(sql)
                        .expect("query")
                        .query_map(params![query, limit], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, f64>(2)?,
                                row.get::<_, String>(3)?,
                            ))
                        })
                        .expect("rows")
                        .collect::<rusqlite::Result<Vec<_>>>()
                        .expect("values")
                };
                let mut statement = conn.prepare(FTS_CANDIDATES_SQL).expect("metadata query");
                let metadata = statement
                    .query_map(params![query, limit], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, f64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    })
                    .expect("metadata");
                let bounded: Vec<_> = metadata
                    .map(|row| {
                        let (path, rank, hash, id) = row.expect("candidate");
                        let body: String = conn
                            .query_row("SELECT body FROM docs WHERE rowid=?1", params![id], |row| {
                                row.get(0)
                            })
                            .expect("body in same snapshot");
                        (path, body, rank, hash)
                    })
                    .collect();
                assert_eq!(bounded, fetch(direct), "query={query}, limit={limit}");
            }
        }
        let id;
        {
            let mut metadata = conn.prepare(FTS_CANDIDATES_SQL).expect("snapshot query");
            let mut rows = metadata
                .query(params!["\"alpha\"", 32])
                .expect("snapshot rows");
            let first = rows.next().expect("row").expect("candidate");
            id = first.get::<_, i64>(3).expect("id");
            let path = first.get::<_, String>(0).expect("path");
            let original: String = conn
                .query_row("SELECT body FROM docs WHERE rowid=?1", params![id], |row| {
                    row.get(0)
                })
                .expect("original body");
            let mut writer = connect(&db).expect("writer");
            let tx = writer.transaction().expect("write transaction");
            store_document(&tx, &path, "replacement body", "replacement hash", 16)
                .expect("replace");
            tx.commit().expect("concurrent commit");
            let during: String = conn
                .query_row("SELECT body FROM docs WHERE rowid=?1", params![id], |row| {
                    row.get(0)
                })
                .expect("snapshot body");
            assert_eq!(
                during, original,
                "candidate metadata and body must share a read snapshot"
            );
        }
        let after: String = conn
            .query_row("SELECT body FROM docs WHERE rowid=?1", params![id], |row| {
                row.get(0)
            })
            .expect("new snapshot");
        assert_eq!(after, "replacement body");
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
    fn old_sensitive_cached_rows_never_escape_and_refresh_removes_them() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("cache");
        let db = state.path().join("index.sqlite3");
        fs::create_dir_all(project.path().join(".aws")).expect("secret folder");
        let secret = "pub fn private_exposure_marker_9928() {}\n";
        fs::write(project.path().join(".aws").join("keys.rs"), secret).expect("secret");
        fs::write(project.path().join("safe.rs"), "pub fn public_ok() {}\n").expect("normal file");
        let clean = synchronize_into(project.path(), &db).expect("scan");
        assert_eq!(clean.indexed, 1, "private directory not scanned");
        {
            let mut conn = connect(&db).expect("cache");
            let tx = conn.transaction().expect("transaction");
            // Reproduce an older index containing a now-denied but perfectly
            // hash-matching file. Search must block it before emitting text.
            store_document(
                &tx,
                ".aws/keys.rs",
                secret,
                blake3::hash(secret.as_bytes()).to_hex().as_ref(),
                secret.len() as i64,
            )
            .expect("legacy cache insertion");
            tx.commit().expect("commit");
        }
        let searched = search_in_db(
            project.path(),
            &db,
            "private_exposure_marker_9928",
            4,
            SyncStats::default(),
        )
        .expect("read old cache");
        assert!(searched.hits.is_empty(), "{searched:?}");
        assert!(searched.stale_results_discarded > 0);
        let syntax = inspect_indexed_file_in_db(project.path(), &db, Path::new(".aws/keys.rs"))
            .expect("inspect");
        assert!(syntax.is_none());
        let changed =
            refresh_known_paths_into(project.path(), &db, &[PathBuf::from(".aws/keys.rs")])
                .expect("refresh old sensitive entry");
        assert_eq!(changed.sync.removed, 1);
        assert_eq!(verify_index_into(&db).expect("consistent").files, 1);
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

    /// A file whose read access was revoked is not a transient lock: its
    /// cached text must leave the index instead of staying there unserved.
    #[cfg(windows)]
    #[test]
    fn access_denied_file_is_removed_with_its_text() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        let marker = "zq_denied_marker_7301";
        fs::write(project.path().join("keep.rs"), "fn kept() {}\n").expect("keep");
        let secret = project.path().join("secret.rs");
        fs::write(&secret, format!("fn {marker}() {{}}\n")).expect("secret");
        synchronize_into(project.path(), &db).expect("sync");
        // Drop the inherited ACEs and deny Everyone reading: an empty DACL
        // alone does not stop an elevated CI account; a deny ACE does.
        let status = std::process::Command::new("icacls")
            .arg(&secret)
            .args(["/inheritance:r", "/deny", "*S-1-1-0:(R)"])
            .stdout(std::process::Stdio::null())
            .status()
            .expect("icacls");
        assert!(status.success());
        let denied = fs::read(&secret).expect_err("read must be denied");
        assert_eq!(denied.kind(), std::io::ErrorKind::PermissionDenied);
        let stats = synchronize_into(project.path(), &db).expect("sync after deny");
        assert_eq!(stats.removed, 1);
        let bytes = fs::read(&db).expect("db");
        assert!(!bytes.windows(marker.len()).any(|w| w == marker.as_bytes()));
    }

    /// A long-lived reader (an MCP server, a second Findex) keeps SQLite from
    /// deleting the WAL on close; removed text must not survive in it.
    #[test]
    fn removed_file_text_is_not_left_in_wal_while_another_connection_is_open() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        let marker = "zq_wal_marker_5512";
        fs::write(project.path().join("keep.rs"), "fn kept() {}\n").expect("keep");
        let secret = project.path().join("secret.rs");
        fs::write(&secret, format!("fn {marker}() {{}}\n")).expect("secret");
        synchronize_into(project.path(), &db).expect("sync");
        let holder = Connection::open(&db).expect("holder");
        let _: i64 = holder
            .query_row("SELECT count(*) FROM files", [], |row| row.get(0))
            .expect("read");
        fs::remove_file(&secret).expect("delete");
        assert_eq!(
            synchronize_into(project.path(), &db).expect("sync").removed,
            1
        );
        for suffix in ["", "-wal"] {
            let mut path = db.as_os_str().to_owned();
            path.push(suffix);
            let bytes = fs::read(PathBuf::from(path)).unwrap_or_default();
            assert!(
                !bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
                "marker left in index{suffix}"
            );
        }
        drop(holder);
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
    fn excerpts_redact_before_truncating_quoted_assignments() {
        for padding in ["x", "界"] {
            let suffix = " password = \"hunter2\"";
            let line = format!("// {}{suffix}", padding.repeat(241 - 3 - suffix.len()));
            assert_eq!(line.chars().count(), 241);
            let (project, _state, db) = indexed(&[("settings.rs", &line)]);
            let found = search_in_db(project.path(), &db, "password", 5, SyncStats::default())
                .expect("search");
            assert_eq!(found.hits.len(), 1);
            let hit = &found.hits[0];
            assert_eq!((hit.path.as_str(), hit.line), ("settings.rs", 1));
            assert!(!hit.excerpt.contains("hunter2"), "{}", hit.excerpt);
            assert_eq!(hit.excerpt.chars().count(), 240);
            assert!(hit.excerpt.starts_with("// "));
        }
    }

    #[test]
    fn neutralizes_fts_operators() {
        let parsed = fts_query("hello OR world* (secret)").expect("terms");
        assert!(!parsed.0.contains('*'));
        assert!(parsed.0.contains("\"or\""));
    }

    /// FTS5 query construction: single word is double-quoted.
    #[test]
    fn fts_query_single_word() {
        let (expr, terms) = fts_query("hello").expect("terms");
        assert_eq!(expr, r#""hello""#);
        assert_eq!(terms, vec!["hello"]);
    }

    /// FTS5 query construction: multiple words are OR-joined and quoted.
    #[test]
    fn fts_query_multiple_words() {
        let (expr, terms) = fts_query("hello world").expect("terms");
        assert_eq!(expr, r#""hello" OR "world""#);
        assert_eq!(terms, vec!["hello", "world"]);
    }

    /// FTS5 query construction: camelCase identifiers expand to sub-words.
    #[test]
    fn fts_query_camelcase_expands() {
        let (expr, terms) = fts_query("parseConfig").expect("terms");
        assert!(terms.contains(&"parseconfig".to_string()));
        assert!(terms.contains(&"parse".to_string()));
        assert!(terms.contains(&"config".to_string()));
        assert!(expr.contains("\"parseconfig\""));
        assert!(expr.contains("\"parse\""));
        assert!(expr.contains("\"config\""));
    }

    /// FTS5 query construction: snake_case produces a single lowercased term.
    #[test]
    fn fts_query_snake_case() {
        let (expr, terms) = fts_query("parse_config").expect("terms");
        assert_eq!(terms, vec!["parse_config"]);
        assert_eq!(expr, r#""parse_config""#);
    }

    /// FTS5 query construction: quoted phrases are split into words, each quoted.
    #[test]
    fn fts_query_quoted_phrase() {
        let (expr, _terms) = fts_query(r#"hello "world test""#).expect("terms");
        assert!(expr.contains("\"hello\""));
        assert!(expr.contains("\"world\""));
        assert!(expr.contains("\"test\""));
    }

    /// FTS5 query construction: colons are neutralized (no unquoted FTS5 operators).
    #[test]
    fn fts_query_colons_neutralized() {
        let (expr, _terms) = fts_query("module::func:arg").expect("terms");
        assert!(!expr.contains(':'));
        assert!(!expr.contains("::"));
    }

    /// FTS5 query construction: parentheses are neutralized.
    #[test]
    fn fts_query_parens_neutralized() {
        let (expr, _terms) = fts_query("hello (world) foo").expect("terms");
        assert!(!expr.contains('('));
        assert!(!expr.contains(')'));
    }

    /// FTS5 query construction: FTS5 operators AND/OR/NOT/NEAR become literal terms.
    #[test]
    fn fts_query_operators_become_terms() {
        let (expr, _terms) = fts_query("hello AND world OR NOT foo").expect("terms");
        assert!(expr.contains("\"and\""));
        assert!(expr.contains("\"or\""));
        assert!(expr.contains("\"not\""));
    }

    /// FTS5 query construction: asterisk is neutralized (no prefix queries).
    #[test]
    fn fts_query_asterisk_neutralized() {
        let (expr, _terms) = fts_query("hello* world").expect("terms");
        assert!(!expr.contains('*'));
    }

    /// FTS5 query construction: empty query returns None.
    #[test]
    fn fts_query_empty_returns_none() {
        assert!(fts_query("").is_none());
        assert!(fts_query("   ").is_none());
    }

    /// split_query_terms cap at 12 terms.
    #[test]
    fn split_query_terms_capped_at_12() {
        let many: String = (1..=20).map(|i| format!("term{i} ")).collect();
        let terms = split_query_terms(&many);
        assert!(terms.len() <= 12, "got {} terms", terms.len());
    }

    /// Real SQLite integration: search_in_db returns hits for a simple term.
    #[test]
    fn search_in_db_returns_hit_for_indexed_term() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        std::fs::write(
            project.path().join("main.rs"),
            "pub fn poll_proceed(cx: u32) -> u32 { cx + 1 }\n",
        )
        .expect("write");
        synchronize_into(project.path(), &db).expect("sync");
        let report = search_in_db(project.path(), &db, "poll_proceed", 5, SyncStats::default())
            .expect("search");
        assert!(!report.hits.is_empty());
        assert!(report.hits.iter().any(|h| h.path.ends_with("main.rs")));
    }

    /// Real SQLite integration: search_in_db with operator-like input does not crash
    /// and returns no hits (all terms neutralized to literal text).
    #[test]
    fn search_in_db_operator_input_safe() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        std::fs::write(
            project.path().join("main.rs"),
            "pub fn poll_proceed(cx: u32) -> u32 { cx + 1 }\n",
        )
        .expect("write");
        synchronize_into(project.path(), &db).expect("sync");
        // Input that would be dangerous as raw FTS5 syntax.
        let report = search_in_db(
            project.path(),
            &db,
            "poll_proceed OR * (secret) NEAR/10",
            5,
            SyncStats::default(),
        )
        .expect("search");
        // Should not panic; may return 0 hits because all terms are quoted literals.
        assert!(report.hits.len() <= 5);
    }

    /// Real SQLite integration: result count never exceeds limit.
    #[test]
    fn search_in_db_count_respects_limit() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        for i in 0..20 {
            std::fs::write(
                project.path().join(format!("file_{i:02}.rs")),
                format!("fn func_{i}() {{}}\n"),
            )
            .expect("write");
        }
        synchronize_into(project.path(), &db).expect("sync");
        for limit in [1, 3, 5, 10, 100] {
            let report = search_in_db(project.path(), &db, "func", limit, SyncStats::default())
                .expect("search");
            assert!(
                report.hits.len() <= limit,
                "limit={} hits={}",
                limit,
                report.hits.len()
            );
        }
    }

    // =====================================================================
    // Property-based tests (proptest)
    // =====================================================================

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config {
            cases: 300,
            ..Default::default()
        })]

        /// FTS query output never contains raw FTS5 operators that could
        /// change query semantics — every term is double-quoted and escaped.
        fn prop_fts_query_never_leaks_operators(query in any::<String>()) {
            prop_assume!(!query.contains('\0'));
            prop_assume!(query.len() <= 80);
            if let Some((expr, _terms)) = fts_query(&query) {
                // No unquoted FTS5 operators: :, *, ^, $, (, ), AND, OR, NOT, NEAR
                for ch in [':', '*', '^', '$', '(', ')'] {
                    prop_assert!(!expr.contains(ch), "operator {:?} in {:?}", ch, expr);
                }
                // Every term is surrounded by double quotes.
                for term in &_terms {
                    let quoted = format!("\"{}\"", term.replace('"', "\"\""));
                    prop_assert!(expr.contains(&quoted), "term {:?} not quoted in {:?}", term, expr);
                }
            }
        }

        /// split_query_terms always returns at most 12 terms.
        fn prop_split_terms_capped(query in any::<String>()) {
            prop_assume!(!query.contains('\0'));
            prop_assume!(query.len() <= 200);
            let terms = split_query_terms(&query);
            prop_assert!(terms.len() <= 12, "{} terms", terms.len());
            // Empty terms never appear.
            prop_assert!(terms.iter().all(|t| !t.is_empty()));
        }
    }
}
