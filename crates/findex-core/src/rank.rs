//! Model-free file ranking for code navigation.
//!
//! Ranks files for a query using pure structural + lexical signals:
//! BM25 term match, definition presence, syntactic call-site presence,
//! recency, file-type weight and size penalty. Zero LLM calls — the
//! model is never burdened, making this the cheapest and fastest
//! file-ranking layer in the stack.

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// A ranked file result.
#[derive(Debug, Clone, Serialize)]
pub struct RankedFile {
    /// Workspace-relative path.
    pub path: String,
    /// Combined score (higher = more relevant). Not a probability.
    pub score: f64,
    /// Individual signal contributions, for transparency.
    pub signals: RankSignals,
    /// One-line human-readable reason for the rank.
    pub reason: String,
}

/// Individual ranking signal contributions.
#[derive(Debug, Clone, Serialize, Default)]
pub struct RankSignals {
    /// BM25 lexical match score (lower BM25 rank = better; normalized to 0..1).
    pub bm25: f64,
    /// Does the file contain a definition of a queried name? (0 or 1)
    pub definition: f64,
    /// Does the file syntactically call a queried name? (0 or 1)
    pub call_site: f64,
    /// Recency score based on last-modified time (0..1, newer = higher).
    pub recency: f64,
    /// File-type weight (source > test > doc > config).
    pub file_type: f64,
    /// Size penalty for very large files (0..1, smaller = less penalty).
    pub size_penalty: f64,
}

/// Configuration weights for combining signals.
#[derive(Debug, Clone, Copy)]
pub struct RankWeights {
    pub bm25: f64,
    pub definition: f64,
    pub call_site: f64,
    pub recency: f64,
    pub file_type: f64,
    pub size_penalty: f64,
}

impl Default for RankWeights {
    fn default() -> Self {
        Self {
            bm25: 0.35,
            definition: 0.30,
            call_site: 0.20,
            recency: 0.05,
            file_type: 0.05,
            size_penalty: 0.05,
        }
    }
}

/// File type category, used for weighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Source,
    Test,
    Doc,
    Config,
    Other,
}

/// Rank files for a query using only structural + lexical signals.
///
/// `query_terms` are the lexical terms to match (BM25).
/// `defined_names` is a set of names the file defines (from Tree-sitter).
/// `called_names` is a set of names the file syntactically calls.
/// `file_type` is the category of the file.
/// `recency` is 0..1 where 1 = most recently modified.
/// `size_penalty` is 0..1 where 1 = no penalty (small file).
pub fn rank_file(
    path: &Path,
    bm25_score: f64,
    defined_names: &[String],
    called_names: &[String],
    query_terms: &[String],
    file_type: FileType,
    recency: f64,
    size_penalty: f64,
    weights: RankWeights,
) -> RankedFile {
    let query_set: HashMap<&str, bool> = query_terms.iter().map(|t| (t.as_str(), true)).collect();

    // BM25: lower is better; invert and clamp to 0..1.
    let bm25_norm = (1.0 / (1.0 + bm25_score.max(0.0))).clamp(0.0, 1.0);

    // Definition match: any query term is a defined name.
    let definition = if defined_names
        .iter()
        .any(|n| query_set.contains_key(n.as_str()))
    {
        1.0
    } else {
        0.0
    };

    // Call-site match: any query term is a syntactically called name.
    let call_site = if called_names.iter().any(|n| query_set.contains_key(n.as_str())) {
        1.0
    } else {
        0.0
    };

    let file_type_score = match file_type {
        FileType::Source => 1.0,
        FileType::Test => 0.8,
        FileType::Doc => 0.6,
        FileType::Config => 0.4,
        FileType::Other => 0.5,
    };

    let score = weights.bm25 * bm25_norm
        + weights.definition * definition
        + weights.call_site * call_site
        + weights.recency * recency
        + weights.file_type * file_type_score
        + weights.size_penalty * size_penalty;

    let reason = build_reason(bm25_norm, definition, call_site, file_type_score, recency);

    RankedFile {
        path: path.to_string_lossy().into_owned(),
        score,
        signals: RankSignals {
            bm25: bm25_norm,
            definition,
            call_site,
            recency,
            file_type: file_type_score,
            size_penalty,
        },
        reason,
    }
}

fn build_reason(bm25: f64, definition: f64, call_site: f64, file_type: f64, recency: f64) -> String {
    let mut parts = Vec::new();
    if definition > 0.0 {
        parts.push("defines queried name");
    }
    if call_site > 0.0 {
        parts.push("calls queried name");
    }
    if bm25 > 0.5 {
        parts.push("strong lexical match");
    } else if bm25 > 0.2 {
        parts.push("lexical match");
    }
    if file_type >= 1.0 {
        parts.push("source file");
    }
    if recency > 0.7 {
        parts.push("recently modified");
    }
    if parts.is_empty() {
        "weak match".to_string()
    } else {
        parts.join(", ")
    }
}

/// Classify a file by extension.
pub fn file_type_for(path: &Path) -> FileType {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower = name.to_ascii_lowercase();

    if lower.contains("test") || lower.contains("_test") || lower.ends_with("test.rs") {
        return FileType::Test;
    }
    if lower.starts_with("test_") {
        return FileType::Test;
    }
    match ext {
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "cpp" | "cc" | "cxx" | "h" | "hpp"
        | "c" | "go" | "java" | "cs" => FileType::Source,
        "md" | "mdx" | "rst" | "txt" | "adoc" => FileType::Doc,
        "toml" | "json" | "yaml" | "yml" | "ini" | "cfg" | "lock" => FileType::Config,
        _ => FileType::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_match_outranks_lexical_only() {
        let weights = RankWeights::default();
        let a = rank_file(
            Path::new("src/lib.rs"),
            10.0,
            &["foo".to_string()],
            &[],
            &["foo".to_string()],
            FileType::Source,
            0.5,
            1.0,
            weights,
        );
        let b = rank_file(
            Path::new("src/use.rs"),
            1.0,
            &[],
            &[],
            &["foo".to_string()],
            FileType::Source,
            0.5,
            1.0,
            weights,
        );
        assert!(a.score > b.score, "definition should outrank lexical-only: {a:?} vs {b:?}");
    }

    #[test]
    fn call_site_boosts_rank() {
        let weights = RankWeights::default();
        let a = rank_file(
            Path::new("src/caller.rs"),
            5.0,
            &[],
            &["foo".to_string()],
            &["foo".to_string()],
            FileType::Source,
            0.5,
            1.0,
            weights,
        );
        let b = rank_file(
            Path::new("src/other.rs"),
            5.0,
            &[],
            &[],
            &["foo".to_string()],
            FileType::Source,
            0.5,
            1.0,
            weights,
        );
        assert!(a.score > b.score, "call site should boost rank: {a:?} vs {b:?}");
    }

    #[test]
    fn source_outranks_config() {
        let weights = RankWeights::default();
        let a = rank_file(
            Path::new("src/main.rs"),
            1.0,
            &[],
            &[],
            &["foo".to_string()],
            FileType::Source,
            0.5,
            1.0,
            weights,
        );
        let b = rank_file(
            Path::new("config.toml"),
            1.0,
            &[],
            &[],
            &["foo".to_string()],
            FileType::Config,
            0.5,
            1.0,
            weights,
        );
        assert!(a.score > b.score, "source should outrank config: {a:?} vs {b:?}");
    }

    #[test]
    fn zero_query_terms_yields_low_score() {
        let weights = RankWeights::default();
        let r = rank_file(
            Path::new("src/lib.rs"),
            0.0,
            &[],
            &[],
            &[],
            FileType::Source,
            0.5,
            1.0,
            weights,
        );
        assert_eq!(r.signals.bm25, 1.0);
        assert_eq!(r.signals.definition, 0.0);
        assert_eq!(r.signals.call_site, 0.0);
    }
}
