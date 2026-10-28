//! Import *captured and normalized* LSP reference results.
//! Each batch indexes many independent symbol identities in one workspace scan.
//! This does not launch a language server or implement the LSP lifecycle.

use super::{
    admitted, language_for_path, persist, prepare, valid_occurrence, Evidence, ImportReport,
    Result, SemanticSite, MAX_DOCUMENTS, MAX_INPUT_BYTES, MAX_OCCURRENCES,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceLocation {
    path: String,
    line: usize,
    character: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapturedReferences {
    definition: SourceLocation,
    references: Vec<SourceLocation>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum CapturedInput {
    Single(CapturedReferences),
    Batch(Vec<CapturedReferences>),
}

/// Accept a single captured reference result or an array of independent
/// captures. Both formats use project-relative paths and zero-based LSP
/// UTF-16 line/character offsets. Multiple captures amortize index scanning.
pub fn import_lsp_snapshot(project: &Path, json: &str) -> Result<ImportReport> {
    if json.len() > MAX_INPUT_BYTES {
        return Err("LSP snapshot exceeds input budget".into());
    }
    let input: CapturedInput = serde_json::from_str(json)?;
    let captures = match input {
        CapturedInput::Single(capture) => vec![capture],
        CapturedInput::Batch(captures) => captures,
    };
    if captures.is_empty() {
        return Err("empty LSP capture array".into());
    }
    let occurrences_count = captures.iter().try_fold(0_usize, |sum, capture| {
        sum.checked_add(1)?.checked_add(capture.references.len())
    });
    let Some(occurrences_count) = occurrences_count else {
        return Err("LSP occurrence count overflow".into());
    };
    if occurrences_count > MAX_OCCURRENCES {
        return Err("LSP occurrences exceed safety budget".into());
    }

    let source_index = prepare(project)?;
    let mut docs = HashMap::<String, Evidence>::new();
    let mut line_lengths = HashMap::<String, Vec<usize>>::new();
    let mut occurrences = Vec::with_capacity(occurrences_count);
    let mut sites = Vec::with_capacity(occurrences_count);
    let mut identities = HashSet::<String>::new();

    // Validate the complete batch before persisting anything. Read and hash
    // each distinct admitted source once, regardless of reference count.
    for capture in &captures {
        for loc in std::iter::once(&capture.definition).chain(capture.references.iter()) {
            if loc.character > 1_000_000 {
                return Err("LSP character offset exceeds safety budget".into());
            }
            if !docs.contains_key(&loc.path) {
                let checked = admitted(project, &source_index, &loc.path)?;
                let bytes = fs::read(project.join(&checked.path))?;
                if blake3::hash(&bytes).to_hex().as_str() != checked.digest {
                    return Err("LSP source changed during position validation".into());
                }
                let source = std::str::from_utf8(&bytes)?;
                let lengths = super::source_lines(source)
                    .map(|line| line.encode_utf16().count())
                    .collect::<Vec<_>>();
                line_lengths.insert(loc.path.clone(), lengths);
                docs.insert(loc.path.clone(), checked);
                if docs.len() > MAX_DOCUMENTS {
                    return Err("too many LSP files".into());
                }
            }
            let lengths = line_lengths
                .get(&loc.path)
                .ok_or("LSP source line metadata missing")?;
            if lengths
                .get(loc.line)
                .is_none_or(|length| loc.character > *length)
            {
                return Err("LSP UTF-16 character offset or source line is out of bounds".into());
            }
        }
    }

    for capture in &captures {
        let definition_file = docs
            .get(&capture.definition.path)
            .ok_or("LSP definition source missing")?;
        let identity = format!(
            "lsp:{}:{}:{}",
            definition_file.path, capture.definition.line, capture.definition.character
        );
        if !identities.insert(identity.clone()) {
            return Err("duplicate definition anchor in LSP batch".into());
        }
        let line = capture
            .definition
            .line
            .checked_add(1)
            .ok_or("LSP line overflow")?;
        occurrences.push(valid_occurrence(definition_file, &identity, line, true)?);
        sites.push(SemanticSite {
            symbol: identity.clone(),
            path: definition_file.path.clone(),
            line,
            column: capture.definition.character,
            end_line: None,
            end_column: None,
            position_encoding: 2,
            language: language_for_path(&definition_file.path).to_string(),
            kind: "definition",
        });
        for reference in &capture.references {
            let file = docs.get(&reference.path).ok_or("missing LSP file")?;
            let line = reference.line.checked_add(1).ok_or("LSP line overflow")?;
            occurrences.push(valid_occurrence(file, &identity, line, false)?);
            sites.push(SemanticSite {
                symbol: identity.clone(),
                path: file.path.clone(),
                line,
                column: reference.character,
                end_line: None,
                end_column: None,
                position_encoding: 2,
                language: language_for_path(&file.path).to_string(),
                kind: "reference",
            });
        }
    }

    let mut files: Vec<_> = docs.into_values().collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    persist(
        project,
        "lsp-captured-references",
        &files,
        &occurrences,
        &sites,
        &[],
    )
}
