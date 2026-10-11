//! SQLite/FTS5-backed *lexical* code discovery. No embeddings or model calls.
//! v0.1 reconciles file contents before each search; a native watcher is a planned optimization.

use findex_core::{is_sensitive_filename, storage_dir, MAX_FILES_PER_SYNC, MAX_SOURCE_BYTES};
use ignore::WalkBuilder;
use rusqlite::{params, Connection};
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
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    /// Workspace-relative path, not an arbitrary disk path.
    pub path: String,
    pub line: usize,
    pub excerpt: String,
    /// Lower BM25 numbers mean more relevant lexical matches; not a probability.
    pub bm25_score: f64,
}

#[derive(Debug, Serialize)]
pub struct SearchReport {
    pub hits: Vec<SearchHit>,
    pub sync: SyncStats,
    pub stale_results_discarded: usize,
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

fn connect(db: &Path) -> Result<Connection> {
    if let Some(parent) = db.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS files(
             path TEXT PRIMARY KEY NOT NULL,
             hash TEXT NOT NULL,
             size_bytes INTEGER NOT NULL
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS docs USING fts5(
             path UNINDEXED,
             body,
             tokenize='unicode61'
         );",
    )?;
    Ok(conn)
}

/// Atomic reconciliation against project files. Paths outside `project` are never indexed.
/// If the file limit is reached, the transaction is rolled back rather than deleting unseen files.
pub fn synchronize_into(project: &Path, database: &Path) -> Result<SyncStats> {
    let project = project.canonicalize()?;
    let mut conn = connect(database)?;
    let tx = conn.transaction()?;
    let existing: HashMap<String, String> = {
        let mut statement = tx.prepare("SELECT path, hash FROM files")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<std::result::Result<HashMap<_, _>, _>>()?
    };
    let mut seen = HashSet::new();
    let mut stats = SyncStats::default();
    let walker = WalkBuilder::new(&project)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .follow_links(false)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            !matches!(
                name.as_str(),
                ".git"
                    | ".findex"
                    | "node_modules"
                    | "target"
                    | "vendor"
                    | "dist"
                    | "build"
                    | ".venv"
                    | "__pycache__"
            )
        })
        .build();

    for item in walker {
        let entry = match item {
            Ok(e) => e,
            Err(_) => {
                stats.skipped += 1;
                continue;
            }
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if is_sensitive_filename(entry.path()) {
            stats.skipped += 1;
            continue;
        }
        let rel = entry.path().strip_prefix(&project)?;
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
            Err(_) => {
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
        let digest = blake3::hash(&bytes).to_hex().to_string();
        if existing
            .get(&rel_string)
            .is_some_and(|old| old.as_str() == digest.as_str())
        {
            stats.unchanged += 1;
            continue;
        }
        tx.execute("DELETE FROM docs WHERE path = ?1", params![rel_string])?;
        tx.execute(
            "INSERT INTO docs(path, body) VALUES(?1, ?2)",
            params![rel_string, contents],
        )?;
        tx.execute(
            "INSERT INTO files(path, hash, size_bytes) VALUES(?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET hash=excluded.hash, size_bytes=excluded.size_bytes",
            params![rel_string, digest, bytes.len() as i64],
        )?;
        stats.indexed += 1;
    }

    // Skipped but previously indexed files must be removed to prevent leaking obsolete results.
    for old_path in existing.keys() {
        if !seen.contains(old_path) || !is_eligible_file(&project.join(old_path)) {
            tx.execute("DELETE FROM docs WHERE path = ?1", params![old_path])?;
            tx.execute("DELETE FROM files WHERE path = ?1", params![old_path])?;
            stats.removed += 1;
        }
    }
    tx.commit()?;
    Ok(stats)
}

fn is_eligible_file(path: &Path) -> bool {
    if is_sensitive_filename(path) {
        return false;
    }
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() > MAX_SOURCE_BYTES {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    !bytes.contains(&0) && std::str::from_utf8(&bytes).is_ok()
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
        retrieval: "sqlite-fts5-bm25",
    };
    let Some((expression, terms)) = fts_query(query) else {
        return Ok(report);
    };
    let conn = connect(database)?;
    let mut stmt = conn.prepare(
        "SELECT docs.path, docs.body, bm25(docs), files.hash
         FROM docs JOIN files ON files.path = docs.path
         WHERE docs MATCH ?1 ORDER BY bm25(docs) ASC LIMIT ?2",
    )?;
    let count = limit.clamp(1, 32);
    let result_rows = stmt.query_map(params![expression, (count * 4) as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for row in result_rows {
        let (path, body, score, indexed_hash) = row?;
        // Never return stale cached code if a file changed after synchronization.
        let full_path: PathBuf = project.join(&path);
        let Ok(current_bytes) = fs::read(&full_path) else {
            report.stale_results_discarded += 1;
            continue;
        };
        if blake3::hash(&current_bytes).to_hex().as_str() != indexed_hash {
            report.stale_results_discarded += 1;
            continue;
        }
        let (line_number, excerpt) = body
            .lines()
            .enumerate()
            .find(|(_, line)| terms.iter().any(|term| line.to_lowercase().contains(term)))
            .map(|(index, line)| (index + 1, line.trim().chars().take(240).collect::<String>()))
            .unwrap_or((1, String::new()));
        report.hits.push(SearchHit {
            path,
            line: line_number,
            excerpt,
            bm25_score: score,
        });
        if report.hits.len() >= count {
            break;
        }
    }
    Ok(report)
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
    fn neutralizes_fts_operators() {
        let parsed = fts_query("hello OR world* (secret)").expect("terms");
        assert!(!parsed.0.contains('*'));
        assert!(parsed.0.contains("\"or\""));
    }
}
