//! Official SCIP protobuf import. Bounded input is parsed in memory;
//! no arbitrary indexing executable or network request is launched.

use super::{admitted, prepare, persist, valid_occurrence, Evidence, ImportReport, Occurrence, Result, MAX_DOCUMENTS, MAX_INPUT_BYTES, MAX_OCCURRENCES};
use protobuf::Message;
use scip::types::{Index, SymbolRole};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn import_scip(project: &Path, file: &Path) -> Result<ImportReport> {
    let meta = fs::metadata(file)?;
    if !meta.is_file() || meta.len() > MAX_INPUT_BYTES as u64 {
        return Err("SCIP index exceeds input budget or is not a file".into());
    }
    import_scip_bytes(project, &fs::read(file)?)
}
/// Symbol IDs are opaque and scoped to the exact SCIP producer. Local SCIP
/// symbols must be qualified with their containing document to avoid aliasing.
pub fn import_scip_bytes(project: &Path, bytes: &[u8]) -> Result<ImportReport> {
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err("SCIP payload outside input budget".into());
    }
    let index = Index::parse_from_bytes(bytes)?;
    if index.documents.len() > MAX_DOCUMENTS {
        return Err("too many SCIP documents".into());
    }
    let total: usize = index.documents.iter()
        .map(|d| d.occurrences.len()).sum();
    if total > MAX_OCCURRENCES {
        return Err("too many SCIP occurrences".into());
    }
    let indexed = prepare(project)?;
    let mut files = HashMap::<String, Evidence>::new();
    let mut rows = Vec::<Occurrence>::with_capacity(total);
    for doc in &index.documents {
        if files.contains_key(&doc.relative_path) {
            return Err("duplicate SCIP document path".into());
        }
        let source = admitted(project, &indexed, &doc.relative_path)?;
        for occurrence in &doc.occurrences {
            if occurrence.symbol.is_empty() { continue; }
            // The SCIP 0.10 Rust bindings expose compact 0-based range arrays:
            // [line, start, end] or [start_line, start, end_line, end].
            let start_line = match occurrence.range.as_slice() {
                [line, start, end] if *line >= 0 && *start >= 0 && end >= start => *line as usize,
                [line, start, end_line, end] if *line >= 0 && *start >= 0
                    && end_line >= line && *end >= 0 => *line as usize,
                _ => return Err("malformed SCIP occurrence range".into()),
            };
            let symbol = if occurrence.symbol.starts_with("local ") {
                format!("scip-local:{}:{}",source.path,occurrence.symbol)
            } else {
                occurrence.symbol.clone()
            };
            let definition = (occurrence.symbol_roles & SymbolRole::Definition as i32) != 0;
            rows.push(valid_occurrence(&source, &symbol, start_line + 1, definition)?);
        }
        files.insert(source.path.clone(), source);
    }
    let mut docs: Vec<_> = files.into_values().collect();
    docs.sort_by(|a,b| a.path.cmp(&b.path));
    persist(project, "scip-protobuf", &docs, &rows)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::impact;
    use scip::types::{Document, Occurrence};
    use tempfile::tempdir;

    fn occurrence(symbol: &str, definition: bool, line: i32) -> Occurrence {
        Occurrence {
            symbol: symbol.to_owned(),
            symbol_roles: if definition {SymbolRole::Definition as i32} else {0},
            range: vec![line,0,4],
            ..Occurrence::default()
        }
    }
    fn binary() -> Vec<u8> {
        let symbol = "rust-analyzer cargo sample 0.1 target().";
        let index = Index {
            documents: vec![
                Document {
                    relative_path: "a.rs".to_owned(),
                    occurrences: vec![occurrence(symbol,true,0)],
                    ..Document::default()
                },
                Document {
                    relative_path: "b.rs".to_owned(),
                    occurrences: vec![occurrence(symbol,false,0)],
                    ..Document::default()
                },
            ],
            ..Index::default()
        };
        index.write_to_bytes().expect("protobuf serialization")
    }
    #[test]
    fn native_scip_binary_produces_reference_impact() {
        let root = tempdir().expect("workspace");
        fs::write(root.path().join("a.rs"), "fn target() {}").expect("a");
        fs::write(root.path().join("b.rs"), "fn uses() { target(); }").expect("b");
        let imported = import_scip_bytes(root.path(), &binary()).expect("scip import");
        assert_eq!(imported.definitions, 1);
        assert_eq!(imported.references, 1);
        let affected = impact(root.path(), Path::new("a.rs"), 2, 8).expect("impact");
        assert_eq!(affected.hits[0].path, "b.rs");
        assert_eq!(affected.hits[0].provenance, "scip-protobuf");
    }
    #[test]
    fn malformed_scip_ranges_are_rejected_without_replacing_snapshot() {
        let root = tempdir().expect("workspace");
        fs::write(root.path().join("a.rs"), "fn target() {}").expect("a");
        fs::write(root.path().join("b.rs"), "fn uses() { target(); }").expect("b");
        import_scip_bytes(root.path(), &binary()).expect("good");
        let mut broken = Index::parse_from_bytes(&binary()).expect("parse");
        broken.documents[1].occurrences[0].range = vec![-1,0,4];
        assert!(import_scip_bytes(
            root.path(), &broken.write_to_bytes().expect("serialized")
        ).is_err());
        assert_eq!(impact(root.path(), Path::new("a.rs"), 1, 8)
            .expect("previous snapshot").hits.len(),1);
    }
}
