//! Model-free file ranking built on top of the lexical index.
//!
//! Synchronizes the index, runs BM25 to get candidates, then enriches each
//! candidate with Tree-sitter definition / call-site signals from the
//! persistent symbol tables. Zero LLM calls.

use crate::{connect, synchronize, Result, SyncStats};
use findex_core::{
    rank::{rank_file, file_type_for, RankWeights},
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
pub fn rank(
    project: &Path,
    query: &str,
    limit: usize,
) -> Result<RankReport> {
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
    let candidates = bm25_candidates(&conn, query, limit * 4)?;

    // Enrich each candidate with definition / call-site signals.
    let mut hits = Vec::new();
    for cand in &candidates {
        let path = Path::new(&cand.path);
        let defined = file_defines_names(&conn, path, &terms)?;
        let called = file_calls_names(project, path, &terms)?;

        let ft = file_type_for(path);
        let ranked = rank_file(
            path,
            cand.bm25_score,
            &defined,
            &called,
            &terms,
            ft,
            0.5, // recency: neutral default
            1.0, // size_penalty: neutral default
            RankWeights::default(),
        );

        hits.push(RankedHit {
            path: cand.path.clone(),
            score: ranked.score,
            bm25_score: cand.bm25_score,
            signals: ranked.signals,
            reason: ranked.reason,
            excerpt: cand.excerpt.clone(),
        });
    }

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
fn bm25_candidates(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<Bm25Candidate>> {
    let fts_query = terms_to_fts_query(query);
    let mut stmt = conn.prepare_cached(
        "SELECT files.path, docs.rank
         FROM docs JOIN files ON files.id = docs.rowid
         WHERE docs MATCH ?1 ORDER BY docs.rank ASC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
        Ok(Bm25Candidate {
            path: row.get(0)?,
            bm25_score: row.get::<_, f64>(1)?.abs(),
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
fn terms_to_fts_query(query: &str) -> String {
    let terms: Vec<&str> = query.split_whitespace().collect();
    terms.join(" | ")
}

/// Check which query terms are defined in the given file's symbol table.
fn file_defines_names(
    conn: &Connection,
    path: &Path,
    terms: &[String],
) -> Result<Vec<String>> {
    let key = findex_core::normalized_relative_key(path)?;
    let term_set: HashSet<&str> = terms.iter().map(|t| t.as_str()).collect();
    let mut found = Vec::new();
    for term in terms {
        if !term_set.contains(term.as_str()) {
            continue;
        }
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM symbols s
             JOIN symbol_state st ON st.path = s.path AND st.extractor_revision = 3
             JOIN files f ON f.path = s.path AND f.hash = st.hash
             WHERE s.path = ?1 AND lower(replace(s.name, '_', '')) = ?2",
            params![key, term],
            |row| row.get(0),
        ).optional()?.unwrap_or(0);
        if count > 0 {
            found.push(term.clone());
        }
    }
    Ok(found)
}

/// Check which query terms are syntactically called in the given file.
/// Uses a lexical heuristic on the on-disk source.
fn file_calls_names(
    project: &Path,
    path: &Path,
    terms: &[String],
) -> Result<Vec<String>> {
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
