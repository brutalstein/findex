//! Model-free file ranking built on top of the lexical index.
//!
//! Synchronizes the index, runs BM25 to get candidates, then enriches each
//! candidate with Tree-sitter definition / call-site signals from the
//! persistent symbol tables. Zero LLM calls.

use crate::{cached_connect, split_query_terms, synchronize, Result, SyncStats};
use findex_core::{
    rank::{file_type_for, rank_file, RankParams, RankWeights},
    storage_dir, MAX_SOURCE_BYTES,
};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::time::SystemTime;

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
    let conn = cached_connect(database)?;
    let conn = conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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

    // Collect mtimes for all candidates so recency can be computed relative
    // to the most recently modified file in this batch.
    let mut mtimes: Vec<Option<SystemTime>> = Vec::with_capacity(candidates.len());
    for cand in &candidates {
        let abs = project.join(Path::new(&cand.path));
        mtimes.push(
            std::fs::symlink_metadata(&abs)
                .ok()
                .and_then(|m| m.modified().ok()),
        );
    }
    let newest_mtime = mtimes.iter().flatten().max();

    // Enrich each candidate with definition / call-site signals.
    let mut hits = Vec::new();
    for (cand, mtime) in candidates.iter().zip(mtimes.iter()) {
        let path = Path::new(&cand.path);
        let defined = file_defines_names(&conn, path, &terms)?;
        let called = file_calls_names(project, path, Some(&cand.hash), &terms)?;

        let ft = file_type_for(path);
        let recency = newest_mtime
            .map(|newest| {
                mtime
                    .map(|mt| {
                        let age = newest.duration_since(mt).unwrap_or_default();
                        // Recency decays over a 30-day window: 1.0 = modified now,
                        // approaching 0.0 as age approaches 30 days.
                        let days = age.as_secs_f64() / 86_400.0;
                        (1.0 - days / 30.0).clamp(0.0, 1.0)
                    })
                    .unwrap_or(0.0)
            })
            .unwrap_or(0.5);
        let size_penalty = file_size_penalty(project, path);
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

    // Truncate first, then report the accurate count of what is actually
    // returned. `returned` must reflect hits.len() after truncation so
    // consumers never see a count larger than the hit array.
    hits.truncate(limit);
    let returned = hits.len();

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
    hash: String,
    excerpt: Option<String>,
}

/// Run BM25 FTS5 search and return candidates with scores.
fn bm25_candidates(conn: &Connection, query: &str, limit: usize) -> Result<Vec<Bm25Candidate>> {
    let fts_query = terms_to_fts_query(query);
    let mut stmt = conn.prepare_cached(
        "SELECT files.path, docs.rank, files.hash
         FROM docs JOIN files ON files.id = docs.rowid
         WHERE docs MATCH ?1 ORDER BY docs.rank ASC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
        Ok(Bm25Candidate {
            path: row.get(0)?,
            bm25_score: row.get::<_, f64>(1)?,
            hash: row.get(2)?,
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
/// not query syntax. camelCase/PascalCase identifiers are expanded into
/// sub-words for better recall.
fn terms_to_fts_query(query: &str) -> String {
    let terms = split_query_terms(query);
    if terms.is_empty() {
        String::new()
    } else {
        terms
            .iter()
            .map(|term| {
                let escaped = term.replace('"', "\"\"");
                format!("\"{}\"", escaped)
            })
            .collect::<Vec<_>>()
            .join(" OR ")
    }
}

/// Check which query terms are defined in the given file's symbol table.
/// Single query instead of one per term: fetches all defined names for the
/// path/hash once, then filters against query terms in Rust.
fn file_defines_names(conn: &Connection, path: &Path, terms: &[String]) -> Result<Vec<String>> {
    let key = findex_core::normalized_relative_key(path)?;
    let term_set: HashSet<&str> = terms.iter().map(|t| t.as_str()).collect();
    let mut found = Vec::new();
    let mut stmt = conn.prepare_cached(
        "SELECT lower(replace(s.name, '_', '')) as term
         FROM symbols s
         JOIN symbol_state st ON st.path = s.path AND st.extractor_revision = ?2
         JOIN files f ON f.path = s.path AND f.hash = st.hash
         WHERE s.path = ?1",
    )?;
    let rows = stmt.query_map(params![key, 3i64], |row| row.get::<_, String>(0))?;
    for row in rows {
        let term = row?;
        if term_set.contains(term.as_str()) && !found.contains(&term) {
            found.push(term);
        }
    }
    Ok(found)
}

/// Check which query terms are syntactically called in the given file.
/// Reads the file through a hash-verified path so call-site evidence is
/// never stale: if the on-disk content no longer matches the indexed
/// digest, the call-site signal is conservatively dropped.
fn file_calls_names(
    project: &Path,
    path: &Path,
    indexed_hash: Option<&str>,
    terms: &[String],
) -> Result<Vec<String>> {
    let Some(source) = hash_verified_source(project, path, indexed_hash) else {
        return Ok(Vec::new());
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

/// Read a file's source text only if it still hash-matches the indexed digest.
/// Returns None when the file is missing, inaccessible, oversized, binary,
/// escapes the workspace, or its content hash has changed — fail closed.
fn hash_verified_source(
    project: &Path,
    relative: &Path,
    indexed_hash: Option<&str>,
) -> Option<String> {
    let abs = project.join(relative);
    let metadata = std::fs::symlink_metadata(&abs).ok()?;
    if !metadata.file_type().is_file()
        || metadata.len() > MAX_SOURCE_BYTES
        || findex_core::is_cloud_only(&metadata)
    {
        return None;
    }
    let resolved = abs.canonicalize().ok()?;
    let actual = resolved.strip_prefix(project).ok()?;
    if is_disallowed_project_path(actual) {
        return None;
    }
    let bytes = std::fs::read(&abs).ok()?;
    if let Some(expected) = indexed_hash {
        if blake3::hash(&bytes).to_hex().as_str() != expected {
            return None;
        }
    }
    String::from_utf8(bytes).ok()
}

/// Size penalty: 1.0 up to 4 KiB, decaying toward 0.5 at 256 KiB.
fn file_size_penalty(project: &Path, relative: &Path) -> f64 {
    let abs = project.join(relative);
    let metadata = match std::fs::symlink_metadata(&abs) {
        Ok(m) => m,
        Err(_) => return 1.0,
    };
    let len = metadata.len();
    if len <= 4096 {
        1.0
    } else {
        (1.0 - ((len as f64 - 4096.0) / (256_000.0 - 4096.0)).clamp(0.0, 1.0) * 0.5).clamp(0.5, 1.0)
    }
}

/// Wrapper around [`findex_core::is_disallowed_project_path`].
fn is_disallowed_project_path(path: &Path) -> bool {
    findex_core::is_disallowed_project_path(path)
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
    fn file_size_penalty_small_file_full_penalty() {
        let project = tempdir().expect("project");
        let abs = project.path().join("small.rs");
        std::fs::write(&abs, "fn x() {}\n").expect("write");
        let size_penalty = file_size_penalty(project.path(), Path::new("small.rs"));
        assert_eq!(size_penalty, 1.0);
    }

    #[test]
    fn file_size_penalty_nonexistent_file_defaults() {
        let project = tempdir().expect("project");
        let size_penalty = file_size_penalty(project.path(), Path::new("missing.rs"));
        assert_eq!(size_penalty, 1.0);
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
        // Scores must be non-increasing (recency may differ slightly due to
        // filesystem mtime resolution, so allow a tiny epsilon).
        for pair in report.hits.windows(2) {
            assert!(
                pair[0].score >= pair[1].score - 1e-6,
                "scores must be non-increasing: {:?} vs {:?}",
                pair[0],
                pair[1]
            );
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

    /// camelCase queries match indexed sub-words: "parseConfig" should
    /// surface a file defining `parse_config_file` even though the token
    /// itself doesn't literally appear.
    #[test]
    fn rank_in_db_camelcase_query_matches_subword_definition() {
        let project = tempdir().expect("project");
        let src = project.path().join("src");
        std::fs::create_dir(&src).expect("make src");
        std::fs::write(
            src.join("config.ts"),
            "// loader\nexport function parse_config_file() {}\n",
        )
        .expect("write config.ts");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        synchronize_into(project.path(), &db).expect("sync");
        let report =
            rank_in_db(project.path(), &db, "parseConfig", 5, SyncStats::default()).expect("rank");
        assert!(!report.hits.is_empty());
        assert_eq!(report.hits[0].path, "src/config.ts");
    }

    /// Final sort happens before truncation: `returned` equals hits.len()
    /// after truncation and never exceeds `limit`.
    #[test]
    fn rank_in_db_sort_before_truncate_and_count() {
        let project = tempdir().expect("project");
        let src = project.path().join("src");
        std::fs::create_dir(&src).expect("make src");
        for i in 0..20 {
            std::fs::write(
                src.join(format!("file_{i:02}.rs")),
                format!("fn func_{i}() {{}}\n"),
            )
            .expect("write");
        }
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        synchronize_into(project.path(), &db).expect("sync");
        let report =
            rank_in_db(project.path(), &db, "func", 5, SyncStats::default()).expect("rank");
        assert_eq!(report.returned, report.hits.len());
        assert!(report.returned <= 5);
        assert!(report.hits.len() <= 5);
    }

    /// Scores are non-increasing after sort (deterministic ordering).
    #[test]
    fn rank_in_db_scores_are_non_increasing() {
        let project = tempdir().expect("project");
        let src = project.path().join("src");
        std::fs::create_dir(&src).expect("make src");
        std::fs::write(src.join("a.rs"), "fn alpha() {}\n").expect("write a");
        std::fs::write(src.join("b.rs"), "fn alpha() {}\n").expect("write b");
        std::fs::write(src.join("c.rs"), "fn beta() {}\n").expect("write c");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        synchronize_into(project.path(), &db).expect("sync");
        let report =
            rank_in_db(project.path(), &db, "alpha", 5, SyncStats::default()).expect("rank");
        for pair in report.hits.windows(2) {
            assert!(
                pair[0].score >= pair[1].score - 1e-6,
                "scores must be non-increasing: {:?} vs {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    /// Deterministic tie-break: equal scores are ordered by path ascending.
    #[test]
    fn rank_in_db_tie_break_by_path() {
        let project = tempdir().expect("project");
        let src = project.path().join("src");
        std::fs::create_dir(&src).expect("make src");
        std::fs::write(src.join("zeta.rs"), "fn alpha() {}\n").expect("write zeta");
        std::fs::write(src.join("alpha.rs"), "fn alpha() {}\n").expect("write alpha");
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        synchronize_into(project.path(), &db).expect("sync");
        let report =
            rank_in_db(project.path(), &db, "alpha", 5, SyncStats::default()).expect("rank");
        assert!(report.hits.len() >= 2);
        // Equal scores → path ascending: alpha.rs before zeta.rs.
        let mut paths_sorted: Vec<&str> = report.hits.iter().map(|h| h.path.as_str()).collect();
        paths_sorted.sort();
        assert_eq!(
            paths_sorted,
            report
                .hits
                .iter()
                .map(|h| h.path.as_str())
                .collect::<Vec<_>>(),
            "tie-broken hits must be path-sorted"
        );
    }
}
