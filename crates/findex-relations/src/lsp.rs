//! Import a *captured and normalized* LSP references result.
//! This does not launch arbitrary language servers or pretend to implement LSP
//! initialization, document synchronization or the full transport lifecycle.

use super::{
    admitted, persist, prepare, valid_occurrence, Evidence, ImportReport, Result, MAX_DOCUMENTS,
    MAX_INPUT_BYTES, MAX_OCCURRENCES,
};
use serde::Deserialize;
use std::collections::HashMap;
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
/// The normalized capture format uses project-relative paths and 0-based LSP
/// line/character offsets. The definition location is an authoritative anchor
/// supplied by the caller, not inferred from symbol names.
pub fn import_lsp_snapshot(project: &Path, json: &str) -> Result<ImportReport> {
    if json.len() > MAX_INPUT_BYTES {
        return Err("LSP snapshot exceeds input budget".into());
    }
    let response: CapturedReferences = serde_json::from_str(json)?;
    if response.references.len() >= MAX_OCCURRENCES {
        return Err("LSP references exceed safety budget".into());
    }
    let source_index = prepare(project)?;
    let mut docs = HashMap::<String, Evidence>::new();
    let mut occurrences = Vec::with_capacity(response.references.len() + 1);
    for loc in std::iter::once(&response.definition).chain(response.references.iter()) {
        if loc.character > 1_000_000 {
            return Err("LSP character offset exceeds safety budget".into());
        }
        if !docs.contains_key(&loc.path) {
            let checked = admitted(project, &source_index, &loc.path)?;
            docs.insert(loc.path.clone(), checked);
            if docs.len() > MAX_DOCUMENTS {
                return Err("too many LSP files".into());
            }
        }
    }
    let definition_file = docs
        .get(&response.definition.path)
        .ok_or("LSP definition source missing")?;
    let identity = format!(
        "lsp:{}:{}:{}",
        definition_file.path, response.definition.line, response.definition.character
    );
    occurrences.push(valid_occurrence(
        definition_file,
        &identity,
        response.definition.line + 1,
        true,
    )?);
    for reference in &response.references {
        let file = docs.get(&reference.path).ok_or("missing LSP file")?;
        occurrences.push(valid_occurrence(
            file,
            &identity,
            reference.line + 1,
            false,
        )?);
    }
    let mut files: Vec<_> = docs.into_values().collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    persist(project, "lsp-captured-references", &files, &occurrences)
}
