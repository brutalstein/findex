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

/// Input parameters for [`rank_file`].
///
/// Groups all ranking inputs into a single struct to avoid a long argument
/// list and make the call site self-documenting.
#[derive(Debug, Clone)]
pub struct RankParams {
    /// Workspace-relative path.
    pub path: String,
    /// Raw BM25 score from FTS5 (lower is better).
    pub bm25_score: f64,
    /// Names this file defines (from Tree-sitter).
    pub defined_names: Vec<String>,
    /// Names this file syntactically calls.
    pub called_names: Vec<String>,
    /// Query terms to match against (BM25).
    pub query_terms: Vec<String>,
    /// Category of the file.
    pub file_type: FileType,
    /// Recency score 0..1 where 1 = most recently modified.
    pub recency: f64,
    /// Size penalty 0..1 where 1 = no penalty (small file).
    pub size_penalty: f64,
    /// Weights for combining signals.
    pub weights: RankWeights,
}

/// Rank files for a query using only structural + lexical signals.
///
/// Takes a single [`RankParams`] struct instead of a long argument list.
pub fn rank_file(params: &RankParams) -> RankedFile {
    let query_set: HashMap<&str, bool> = params
        .query_terms
        .iter()
        .map(|t| (t.as_str(), true))
        .collect();

    // BM25: lower is better; invert and clamp to 0..1.
    let bm25_norm = (1.0 / (1.0 + params.bm25_score.max(0.0))).clamp(0.0, 1.0);

    // Definition match: any query term is a defined name.
    let definition = if params
        .defined_names
        .iter()
        .any(|n| query_set.contains_key(n.as_str()))
    {
        1.0
    } else {
        0.0
    };

    // Call-site match: any query term is a syntactically called name.
    let call_site = if params
        .called_names
        .iter()
        .any(|n| query_set.contains_key(n.as_str()))
    {
        1.0
    } else {
        0.0
    };

    let file_type_score = match params.file_type {
        FileType::Source => 1.0,
        FileType::Test => 0.8,
        FileType::Doc => 0.6,
        FileType::Config => 0.4,
        FileType::Other => 0.5,
    };

    let score = params.weights.bm25 * bm25_norm
        + params.weights.definition * definition
        + params.weights.call_site * call_site
        + params.weights.recency * params.recency
        + params.weights.file_type * file_type_score
        + params.weights.size_penalty * params.size_penalty;

    let reason = build_reason(
        bm25_norm,
        definition,
        call_site,
        file_type_score,
        params.recency,
    );

    RankedFile {
        path: params.path.clone(),
        score,
        signals: RankSignals {
            bm25: bm25_norm,
            definition,
            call_site,
            recency: params.recency,
            file_type: file_type_score,
            size_penalty: params.size_penalty,
        },
        reason,
    }
}

fn build_reason(
    bm25: f64,
    definition: f64,
    call_site: f64,
    file_type: f64,
    recency: f64,
) -> String {
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
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "c"
        | "go" | "java" | "cs" => FileType::Source,
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
        let a = rank_file(&RankParams {
            path: "src/lib.rs".into(),
            bm25_score: 10.0,
            defined_names: vec!["foo".to_string()],
            called_names: vec![],
            query_terms: vec!["foo".to_string()],
            file_type: FileType::Source,
            recency: 0.5,
            size_penalty: 1.0,
            weights,
        });
        let b = rank_file(&RankParams {
            path: "src/use.rs".into(),
            bm25_score: 1.0,
            defined_names: vec![],
            called_names: vec![],
            query_terms: vec!["foo".to_string()],
            file_type: FileType::Source,
            recency: 0.5,
            size_penalty: 1.0,
            weights,
        });
        assert!(
            a.score > b.score,
            "definition should outrank lexical-only: {a:?} vs {b:?}"
        );
    }

    #[test]
    fn call_site_boosts_rank() {
        let weights = RankWeights::default();
        let a = rank_file(&RankParams {
            path: "src/caller.rs".into(),
            bm25_score: 5.0,
            defined_names: vec![],
            called_names: vec!["foo".to_string()],
            query_terms: vec!["foo".to_string()],
            file_type: FileType::Source,
            recency: 0.5,
            size_penalty: 1.0,
            weights,
        });
        let b = rank_file(&RankParams {
            path: "src/other.rs".into(),
            bm25_score: 5.0,
            defined_names: vec![],
            called_names: vec![],
            query_terms: vec!["foo".to_string()],
            file_type: FileType::Source,
            recency: 0.5,
            size_penalty: 1.0,
            weights,
        });
        assert!(
            a.score > b.score,
            "call site should boost rank: {a:?} vs {b:?}"
        );
    }

    #[test]
    fn source_outranks_config() {
        let weights = RankWeights::default();
        let a = rank_file(&RankParams {
            path: "src/main.rs".into(),
            bm25_score: 1.0,
            defined_names: vec![],
            called_names: vec![],
            query_terms: vec!["foo".to_string()],
            file_type: FileType::Source,
            recency: 0.5,
            size_penalty: 1.0,
            weights,
        });
        let b = rank_file(&RankParams {
            path: "config.toml".into(),
            bm25_score: 1.0,
            defined_names: vec![],
            called_names: vec![],
            query_terms: vec!["foo".to_string()],
            file_type: FileType::Config,
            recency: 0.5,
            size_penalty: 1.0,
            weights,
        });
        assert!(
            a.score > b.score,
            "source should outrank config: {a:?} vs {b:?}"
        );
    }

    #[test]
    fn zero_query_terms_yields_low_score() {
        let weights = RankWeights::default();
        let r = rank_file(&RankParams {
            path: "src/lib.rs".into(),
            bm25_score: 0.0,
            defined_names: vec![],
            called_names: vec![],
            query_terms: vec![],
            file_type: FileType::Source,
            recency: 0.5,
            size_penalty: 1.0,
            weights,
        });
        assert_eq!(r.signals.bm25, 1.0);
        assert_eq!(r.signals.definition, 0.0);
        assert_eq!(r.signals.call_site, 0.0);
    }
}
