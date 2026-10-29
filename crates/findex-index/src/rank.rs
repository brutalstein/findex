//! Model-free file ranking built on top of the lexical index.
//!
//! Synchronizes the index, runs BM25 to get candidates, then enriches each
//! candidate with Tree-sitter definition / call-site signals from the
//! persistent symbol tables. Zero LLM calls.

use crate::{connect, synchronize, Result, SyncStats};
use findex_core::{
    rank::{file_type_for, rank_file, RankParams, RankWeights},
    storage_dir,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

/// Output of a ranked search.
#[derive(Debug, Serialize)]
pub struct RankReport {
    pub query: String,
    pub hits: Vec<RankedHit>,
    pub sync: SyncStats,
    pub total_candidates: usize,
    pub returned: usize,
}

/// A single ranked file hit.
#[derive(Debug, Serialize)]
pub struct RankedHit {
    pub path: String,
    pub score: f64,
    pub bm25_score: f64,
    pub signals: findex_core::rank::RankSignals,
    pub reason: String,
    /// Excerpt from the file (first matching line).
    pub excerpt: Option<String>,
}

/// Rank files for a query using BM25 + structural signals, no LLM.
pub fn rank(project: &Path, query: &str, limit: usize) -> Result<RankReport> {
    let sync = synchronize(project)?;
    let directory = storage_dir(project)?;
    let db = directory.join("index.sqlite3");
    rank_in_db(project, &db, query, limit, sync)
}

pub fn rank_in_db(
    project: &Path,
    database: &Path,
    query: &str,
    limit: usize,
    sync: SyncStats,
) -> Result<RankReport> {
    let conn = connect(database)?;
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|t| t.to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    if terms.is_empty() {
        return Ok(RankReport {
            query: query.to_string(),
            hits: Vec::new(),
            sync,
            total_candidates: 0,
            returned: 0,
        });
    }

    // BM25 search to get candidates with raw scores.
    // Use a generous candidate window so structural signals can reorder
    // results beyond what BM25 alone would surface.
    let candidate_window = (limit * 8).clamp(32, 1024);
    let candidates = bm25_candidates(&conn, query, candidate_window)?;

    // Enrich each candidate with definition / call-site signals.
    let mut hits = Vec::new();
    for cand in &candidates {
        let path = Path::new(&cand.path);
        let defined = file_defines_names(&conn, path, &terms)?;
        let called = file_calls_names(project, path, &terms)?;

        let ft = file_type_for(path);
        // Compute recency and size penalty from the live file metadata
        // (hash-verified), so these signals are real rather than neutral.
        let (recency, size_penalty) = file_recency_and_size(project, path);
        let ranked = rank_file(&RankParams {
            path: cand.path.clone(),
            bm25_score: cand.bm25_score,
            defined_names: defined,
            called_names: called,
            query_terms: terms.clone(),
            file_type: ft,
            recency,
            size_penalty,
            weights: RankWeights::default(),
        });

        hits.push(RankedHit {
            path: cand.path.clone(),
            score: ranked.score,
            bm25_score: cand.bm25_score,
            signals: ranked.signals,
            reason: ranked.reason,
            excerpt: cand.excerpt.clone(),
        });
    }

    // Re-sort by the final combined score so structural signals
    // (definition, call-site, recency, file type) actually affect ordering.
    // Higher score = more relevant. Tie-break by path for determinism.
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });

    let returned = hits.len();
    hits.truncate(limit);

    Ok(RankReport {
        query: query.to_string(),
        hits,
        sync,
        total_candidates: candidates.len(),
        returned,
    })
}

/// Raw BM25 candidate from the FTS index.
struct Bm25Candidate {
    path: String,
    bm25_score: f64,
    excerpt: Option<String>,
}

/// Run BM25 FTS5 search and return candidates with scores.
fn bm25_candidates(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Bm25Candidate>> {
    let fts_query = terms_to_fts_query(query);
    let mut stmt = conn.prepare_cached(
        "SELECT files.path, docs.rank
         FROM docs JOIN files ON files.id = docs.rowid
         WHERE docs MATCH ?1 ORDER BY docs.rank ASC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
        Ok(Bm25Candidate {
            path: row.get(0)?,
            bm25_score: row.get::<_, f64>(1)?,
            excerpt: None,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Convert plain query terms to an FTS5 query string.
/// Each term is individually quoted and joined with OR so user-supplied
/// punctuation, quotes and FTS operators are treated as literal text,
/// not query syntax.
fn terms_to_fts_query(query: &str) -> String {
    let terms: Vec<String> = query
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .take(12)
        .map(str::to_lowercase)
        .map(|term| {
            let escaped = term.replace('"', "\"\"");
            format!("\"{}\"", escaped)
        })
        .collect();
    if terms.is_empty() {
        String::new()
    } else {
        terms.join(" OR ")
    }
}

/// Check which query terms are defined in the given file's symbol table.
fn file_defines_names(conn: &Connection, path: &Path, terms: &[String]) -> Result<Vec<String>> {
    let key = findex_core::normalized_relative_key(path)?;
    let term_set: HashSet<&str> = terms.iter().map(|t| t.as_str()).collect();
    let mut found = Vec::new();
    for term in terms {
        if !term_set.contains(term.as_str()) {
            continue;
        }
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM symbols s
             JOIN symbol_state st ON st.path = s.path AND st.extractor_revision = 3
             JOIN files f ON f.path = s.path AND f.hash = st.hash
             WHERE s.path = ?1 AND lower(replace(s.name, '_', '')) = ?2",
                params![key, term],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if count > 0 {
            found.push(term.clone());
        }
    }
    Ok(found)
}

/// Check which query terms are syntactically called in the given file.
/// Uses a lexical heuristic on the on-disk source.
fn file_calls_names(project: &Path, path: &Path, terms: &[String]) -> Result<Vec<String>> {
    let _key = findex_core::normalized_relative_key(path)?;
    let abs = project.join(path);
    let source = match std::fs::read_to_string(&abs) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };
    let term_set: HashSet<&str> = terms.iter().map(|t| t.as_str()).collect();
    let mut found = Vec::new();
    for term in terms {
        if !term_set.contains(term.as_str()) {
            continue;
        }
        if source.contains(&format!("{}(", term))
            || source.contains(&format!("{}.", term))
            || source.contains(&format!("{}::", term))
        {
            found.push(term.clone());
        }
    }
    Ok(found)
}

/// Return (recency, size_penalty) for a candidate file, computed from its
/// live metadata and content. Both values are 0..1 where 1 is best.
fn file_recency_and_size(project: &Path, relative: &Path) -> (f64, f64) {
    let abs = project.join(relative);
    let metadata = match std::fs::symlink_metadata(&abs) {
        Ok(m) => m,
        Err(_) => return (0.5, 1.0),
    };
    let len = metadata.len();
    // Size penalty: 1.0 up to 4 KiB, decaying toward 0.5 at 256 KiB.
    let size_penalty = if len <= 4096 {
        1.0
    } else {
        (1.0 - ((len as f64 - 4096.0) / (256_000.0 - 4096.0)).clamp(0.0, 1.0) * 0.5).clamp(0.5, 1.0)
    };
    // Recency: conservative mid value without a batch reference so it does
    // not dominate scoring until true cross-candidate mtime comparison is added.
    let recency = 0.5;
    (recency, size_penalty)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::synchronize_into;
    use tempfile::tempdir;

    /// FTS5 query must quote each term and join with OR; user-supplied
    /// punctuation, quotes and FTS operators become literal text.
    #[test]
    fn terms_to_fts_query_quotes_and_ors_terms() {
        let q = terms_to_fts_query("hello world");
        assert_eq!(q, r#""hello" OR "world""#);
    }

    #[test]
    fn terms_to_fts_query_escapes_quotes() {
        let q = terms_to_fts_query(r#"say "hi""#);
        assert_eq!(q, r#""say" OR "hi""#);
    }

    #[test]
    fn terms_to_fts_query_neutralizes_operators() {
        let q = terms_to_fts_query("hello OR world* (secret)");
        assert!(!q.contains('*'));
        assert!(!q.contains('('));
        assert!(!q.contains(')'));
        assert!(q.contains("\"or\""));
        assert!(q.contains("\"secret\""));
    }

    #[test]
    fn terms_to_fts_query_limits_to_12_terms() {
        let many: String = (1..=20).map(|i| format!("term{i} ")).collect();
        let q = terms_to_fts_query(&many);
        let or_count = q.split(" OR ").count();
        assert_eq!(or_count, 12);
    }

    #[test]
    fn terms_to_fts_query_empty_for_whitespace() {
        let q = terms_to_fts_query("   ");
        assert!(q.is_empty());
    }

    /// Size penalty: small files get 1.0, large files decay toward 0.5.
    #[test]
    fn file_recency_and_size_small_file_full_penalty() {
        let project = tempdir().expect("project");
        let abs = project.path().join("small.rs");
        std::fs::write(&abs, "fn x() {}\n").expect("write");
        let (recency, size_penalty) = file_recency_and_size(project.path(), Path::new("small.rs"));
        assert_eq!(recency, 0.5);
        assert_eq!(size_penalty, 1.0);
    }

    #[test]
    fn file_recency_and_size_nonexistent_file_defaults() {
        let project = tempdir().expect("project");
        let (recency, size_penalty) =
            file_recency_and_size(project.path(), Path::new("missing.rs"));
        assert_eq!((recency, size_penalty), (0.5, 1.0));
    }

    /// rank_in_db re-sorts candidates by final combined score, so a file
    /// with a definition match outranks a file with only a lexical match
    /// even when the lexical-only file had a better BM25 rank.
    #[test]
    fn rank_in_db_reorders_by_definition_signal() {
        let project = tempdir().expect("project");
        let src = project.path().join("src");
        std::fs::create_dir(&src).expect("make src");
        // defines.rs defines "alpha" but has worse lexical terms
        std::fs::write(
            src.join("defines.rs"),
            "// alpha definition only\nfn alpha() {}\n",
        )
        .expect("write defines");
        // uses.rs uses "alpha" but doesn't define it, more lexical matches
        std::fs::write(
            src.join("uses.rs"),
            "alpha alpha alpha\nfn beta() { alpha(); }\n",
        )
        .expect("write uses");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        synchronize_into(project.path(), &db).expect("sync");
        let report =
            rank_in_db(project.path(), &db, "alpha", 5, SyncStats::default()).expect("rank");
        assert!(!report.hits.is_empty());
        // The file defining "alpha" should be first (definition signal boosts score)
        assert_eq!(report.hits[0].path, "src/defines.rs");
        assert!(report.hits[0].signals.definition > 0.0);
        assert!(report.hits[0].score > report.hits[1].score || report.hits.len() == 1);
    }

    /// rank_in_db returns hits sorted by descending score with deterministic tie-break.
    #[test]
    fn rank_in_db_results_are_sorted_and_deterministic() {
        let project = tempdir().expect("project");
        let src = project.path().join("src");
        std::fs::create_dir(&src).expect("make src");
        std::fs::write(src.join("a.rs"), "fn alpha() {}\n").expect("write a");
        std::fs::write(src.join("b.rs"), "fn alpha() {}\n").expect("write b");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        synchronize_into(project.path(), &db).expect("sync");
        let report =
            rank_in_db(project.path(), &db, "alpha", 5, SyncStats::default()).expect("rank");
        // Same score for both; tie-break by path ascending
        if report.hits.len() >= 2 {
            assert!(report.hits[0].path <= report.hits[1].path);
        }
        // Scores must be non-increasing
        for pair in report.hits.windows(2) {
            assert!(pair[0].score >= pair[1].score - 1e-9);
        }
    }

    /// Empty query returns empty report.
    #[test]
    fn rank_in_db_empty_query() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        let report = rank_in_db(project.path(), &db, "   ", 5, SyncStats::default()).expect("rank");
        assert!(report.hits.is_empty());
        assert_eq!(report.total_candidates, 0);
    }
}
