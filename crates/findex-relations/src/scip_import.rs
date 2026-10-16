//! Official SCIP protobuf import. Bounded input is parsed in memory;
//! no arbitrary indexing executable or network request is launched.

use super::{
    admitted, language_for_path, persist, prepare, valid_occurrence, Evidence, ImportReport,
    Occurrence, Result, SemanticLink, SemanticSite, MAX_DOCUMENTS, MAX_INPUT_BYTES,
    MAX_OCCURRENCES, MAX_SYMBOL_LENGTH,
};
use protobuf::Message;
use scip::types::{Index, Occurrence as ScipOccurrence, SymbolRole};
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
/// SCIP 0.10 typed ranges take precedence over the older packed field.
/// Validate start/end locations against the actual hash-verified source.
fn checked_range(occ: &ScipOccurrence, lines: &[&str], encoding: i32) -> Result<(usize, usize)> {
    let typed = if occ.has_single_line_range() {
        let range = occ.single_line_range();
        Some((range.line, range.start_character, range.line, range.end_character))
    } else if occ.has_multi_line_range() {
        let range = occ.multi_line_range();
        Some((range.start_line, range.start_character, range.end_line, range.end_character))
    } else {
        None
    };
    let packed = match occ.range.as_slice() {
        [] => None,
        [line, start, end] => Some((*line, *start, *line, *end)),
        [line, start, end_line, end] => Some((*line, *start, *end_line, *end)),
        _ => return Err("malformed SCIP packed occurrence range".into()),
    };
    if let (Some(a), Some(b)) = (typed, packed) {
        if a != b {
            return Err("SCIP typed and packed ranges disagree".into());
        }
    }
    let (start_line, start, end_line, end) = typed.or(packed)
        .ok_or("SCIP occurrence has no source range")?;
    if start_line < 0 || start < 0 || end_line < start_line || end < 0
        || (start_line == end_line && end < start)
    {
        return Err("invalid SCIP source range ordering".into());
    }
    let start_line = usize::try_from(start_line)?;
    let end_line = usize::try_from(end_line)?;
    let start = usize::try_from(start)?;
    let end = usize::try_from(end)?;
    let first = lines.get(start_line).ok_or("SCIP start line out of bounds")?;
    let last = lines.get(end_line).ok_or("SCIP end line out of bounds")?;
    // 0=unspecified: tolerate offsets measured by any of the supported
    // encodings, but do not pretend such offsets are independently verified.
    let bound = |line: &str| match encoding {
        1 => line.len(),
        2 => line.encode_utf16().count(),
        3 => line.chars().count(),
        _ => line.len().max(line.encode_utf16().count()).max(line.chars().count()),
    };
    if start > bound(first) || end > bound(last)
        || (encoding == 1 && (!first.is_char_boundary(start) || !last.is_char_boundary(end)))
    {
        return Err("SCIP source character offset out of bounds".into());
    }
    Ok((start_line, start))
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
    let total: usize = index.documents.iter().map(|d| d.occurrences.len()).sum();
    if total > MAX_OCCURRENCES {
        return Err("too many SCIP occurrences".into());
    }
    let indexed = prepare(project)?;
    let mut files = HashMap::<String, Evidence>::new();
    let mut rows = Vec::<Occurrence>::with_capacity(total);
    let mut sites = Vec::<SemanticSite>::with_capacity(total);
    let mut links = Vec::<SemanticLink>::new();
    for doc in &index.documents {
        if files.contains_key(&doc.relative_path) {
            return Err("duplicate SCIP document path".into());
        }
        let source = admitted(project, &indexed, &doc.relative_path)?;
        if doc.language.len() > 64 || doc.language.chars().any(char::is_control) {
            return Err("invalid SCIP document language".into());
        }
        let language = if doc.language.is_empty() {
            language_for_path(&source.path).to_string()
        } else {
            doc.language.clone()
        };
        let body = fs::read(project.join(&source.path))?;
        if blake3::hash(&body).to_hex().as_str() != source.digest {
            return Err("SCIP source changed during location validation".into());
        }
        let contents = std::str::from_utf8(&body)?;
        let lines: Vec<&str> = contents.lines().collect();
        let lines = if lines.is_empty() { vec![""] } else { lines };
        let encoding = doc.position_encoding.value();
        for occurrence in &doc.occurrences {
            if occurrence.symbol.is_empty() {
                continue;
            }
            let (start_line, column) = checked_range(occurrence, &lines, encoding)?;
            let symbol = if occurrence.symbol.starts_with("local ") {
                format!("scip-local:{}:{}", source.path, occurrence.symbol)
            } else {
                occurrence.symbol.clone()
            };
            let definition = (occurrence.symbol_roles & SymbolRole::Definition as i32) != 0;
            let kind = if definition {
                "definition"
            } else if (occurrence.symbol_roles & SymbolRole::Import as i32) != 0 {
                "import"
            } else {
                "reference"
            };
            rows.push(valid_occurrence(
                &source,
                &symbol,
                start_line + 1,
                definition,
            )?);
            sites.push(SemanticSite {
                symbol,
                path: source.path.clone(),
                line: start_line + 1,
                column,
                language: language.clone(),
                kind,
            });
        }
        for information in &doc.symbols {
            let owner = if information.symbol.starts_with("local ") {
                format!("scip-local:{}:{}", source.path, information.symbol)
            } else {
                information.symbol.clone()
            };
            if owner.is_empty()
                || owner.len() > MAX_SYMBOL_LENGTH
                || owner.chars().any(char::is_control)
            {
                return Err("invalid SCIP relationship source symbol".into());
            }
            for relation in &information.relationships {
                let target = if relation.symbol.starts_with("local ") {
                    format!("scip-local:{}:{}", source.path, relation.symbol)
                } else {
                    relation.symbol.clone()
                };
                if target.is_empty()
                    || target.len() > MAX_SYMBOL_LENGTH
                    || target.chars().any(char::is_control)
                {
                    return Err("invalid SCIP relationship target symbol".into());
                }
                let mut add = |kind| {
                    links.push(SemanticLink {
                        owner: owner.clone(),
                        target: target.clone(),
                        kind,
                        path: source.path.clone(),
                        language: language.clone(),
                    });
                };
                if relation.is_reference {
                    add("reference_equivalence");
                }
                if relation.is_implementation {
                    add("implements");
                }
                if relation.is_type_definition {
                    add("type_definition");
                }
                if relation.is_definition {
                    add("definition_alias");
                }
                if links.len() > MAX_OCCURRENCES {
                    return Err("too many SCIP semantic relationships".into());
                }
            }
        }
        files.insert(source.path.clone(), source);
    }
    let mut docs: Vec<_> = files.into_values().collect();
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    persist(project, "scip-protobuf", &docs, &rows, &sites, &links)
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
            symbol_roles: if definition {
                SymbolRole::Definition as i32
            } else {
                0
            },
            range: vec![line, 0, 4],
            ..Occurrence::default()
        }
    }
    fn binary() -> Vec<u8> {
        let symbol = "rust-analyzer cargo sample 0.1 target().";
        let index = Index {
            documents: vec![
                Document {
                    relative_path: "a.rs".to_owned(),
                    occurrences: vec![occurrence(symbol, true, 0)],
                    ..Document::default()
                },
                Document {
                    relative_path: "b.rs".to_owned(),
                    occurrences: vec![occurrence(symbol, false, 0)],
                    ..Document::default()
                },
            ],
            ..Index::default()
        };
        index.write_to_bytes().expect("protobuf serialization")
    }
    /// Four distinct languages using opaque SCIP symbol identities. This
    /// checks ingestion semantics, not the accuracy of any specific compiler.
    #[test]
    fn multilang_typed_roles_are_precise_and_same_named_symbols_stay_distinct() {
        use crate::symbol_evidence;
        use scip::types::{Relationship, SymbolInformation};
        let root = tempdir().expect("workspace");
        let cases = [
            (
                "a.rs",
                "rust",
                "fn shared() {}\n",
                "rust-analyzer cargo demo 1 a#shared().",
            ),
            (
                "b.cpp",
                "cpp",
                "int shared() {return 1;}\n",
                "clang . demo 1 b#shared().",
            ),
            (
                "c.ts",
                "typescript",
                "function shared() {}\n",
                "scip-typescript npm demo 1 c#shared().",
            ),
            (
                "d.py",
                "python",
                "def shared(): pass\n",
                "scip-python pip demo 1 d#shared().",
            ),
        ];
        let mut docs = Vec::new();
        for (name, language, source, symbol) in cases {
            fs::write(root.path().join(name), source).expect("source");
            docs.push(Document {
                relative_path: name.to_owned(),
                language: language.to_owned(),
                occurrences: vec![occurrence(symbol, true, 0)],
                ..Document::default()
            });
        }
        let ts_sym = "scip-typescript npm demo 1 c#shared().";
        let rust_sym = "rust-analyzer cargo demo 1 a#shared().";
        docs[2].occurrences.push(Occurrence {
            symbol: rust_sym.to_owned(),
            symbol_roles: SymbolRole::Import as i32,
            range: vec![0, 0, 4],
            ..Occurrence::default()
        });
        docs[2].symbols.push(SymbolInformation {
            symbol: ts_sym.to_owned(),
            relationships: vec![Relationship {
                symbol: rust_sym.to_owned(),
                is_implementation: true,
                ..Relationship::default()
            }],
            ..SymbolInformation::default()
        });
        let payload = Index {
            documents: docs,
            ..Index::default()
        };
        let bytes = payload.write_to_bytes().expect("encode");
        let report = import_scip_bytes(root.path(), &bytes).expect("import");
        assert_eq!(report.schema_version, 2);
        assert_eq!(report.typed_relationships, 1);
        let rust = symbol_evidence(root.path(), rust_sym, 16).expect("rust");
        assert_eq!(rust.sites.len(), 2);
        assert!(rust
            .sites
            .iter()
            .any(|site| site.kind == "import" && site.language == "typescript"));
        assert!(rust
            .sites
            .iter()
            .any(|site| site.kind == "definition" && site.language == "rust"));
        for (_, _, _, unrelated) in [cases[1], cases[2], cases[3]] {
            let result = symbol_evidence(root.path(), unrelated, 16).expect("distinct");
            assert_eq!(
                result
                    .sites
                    .iter()
                    .filter(|site| site.kind == "definition")
                    .count(),
                1
            );
            assert!(!result.sites.iter().any(|site| site.path == "a.rs"));
        }
        let ts = symbol_evidence(root.path(), ts_sym, 16).expect("typescript");
        assert_eq!(ts.relationships.len(), 1);
        assert_eq!(ts.relationships[0].kind, "implements");
        assert_eq!(ts.relationships[0].target_symbol, rust_sym);
        fs::write(root.path().join("c.ts"), "function updated() {}\n").expect("edit");
        let stale = symbol_evidence(root.path(), ts_sym, 16).expect("stale");
        assert!(stale.relationships.is_empty());
        assert!(stale.stale_evidence_discarded > 0);
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
    fn oversized_payload_and_missing_documents_are_rejected() {
        let root = tempdir().expect("workspace");
        fs::write(root.path().join("a.rs"), "fn target() {}").expect("a");
        fs::write(root.path().join("b.rs"), "fn uses() { target(); }").expect("b");
        assert!(import_scip_bytes(root.path(), &[]).is_err());
        assert!(import_scip_bytes(root.path(), &vec![0_u8; MAX_INPUT_BYTES + 1]).is_err());
        let mut broken = Index::parse_from_bytes(&binary()).expect("parse");
        broken.documents[1].relative_path = "../escape.rs".to_owned();
        assert!(
            import_scip_bytes(root.path(), &broken.write_to_bytes().expect("serialize")).is_err()
        );
    }

    #[test]
    fn scip_definition_and_reference_role_are_not_interchanged() {
        let root = tempdir().expect("workspace");
        fs::write(root.path().join("a.rs"), "fn target() {}").expect("a");
        fs::write(root.path().join("b.rs"), "fn uses() { target(); }").expect("b");
        let mut swapped = Index::parse_from_bytes(&binary()).expect("parse");
        swapped.documents[0].occurrences[0].symbol_roles = 0;
        swapped.documents[1].occurrences[0].symbol_roles = SymbolRole::Definition as i32;
        let imported =
            import_scip_bytes(root.path(), &swapped.write_to_bytes().expect("serialize"))
                .expect("valid reversed roles");
        assert_eq!(imported.definitions, 1);
        assert_eq!(imported.references, 1);
        assert!(impact(root.path(), Path::new("a.rs"), 1, 8)
            .expect("not a definition in a.rs")
            .hits
            .is_empty());
    }

    #[test]
    fn scip_typed_ranges_override_packed_ranges_and_reject_invalid_offsets() {
        use scip::types::SingleLineRange;
        let root = tempdir().expect("workspace");
        fs::write(root.path().join("a.rs"), "fn target() {}\n").expect("source");
        let mut parsed = Index::parse_from_bytes(&binary()).expect("parse fixture");
        // Keep only the admitted source file from the fixture.
        parsed.documents.truncate(1);
        let occurrence = &mut parsed.documents[0].occurrences[0];
        occurrence.range = vec![0, 3, 9];
        occurrence.set_single_line_range(SingleLineRange {
            line: 0, start_character: 3, end_character: 9,
            ..SingleLineRange::default()
        });
        import_scip_bytes(root.path(), &parsed.write_to_bytes().expect("typed"))
            .expect("typed accepted");
        let expected = impact(root.path(), Path::new("a.rs"), 1, 8).expect("impact");
        assert!(expected.hits.is_empty());
        let original = parsed.clone();
        parsed.documents[0].occurrences[0].range = vec![0, 0, 4];
        assert!(import_scip_bytes(root.path(), &parsed.write_to_bytes().expect("mismatch")).is_err());
        parsed = original;
        parsed.documents[0].occurrences[0].set_single_line_range(SingleLineRange {
            line: 0, start_character: 1000, end_character: 1001,
            ..SingleLineRange::default()
        });
        parsed.documents[0].occurrences[0].range.clear();
        assert!(import_scip_bytes(root.path(), &parsed.write_to_bytes().expect("out of bounds")).is_err());
    }

    #[test]
    fn malformed_scip_ranges_are_rejected_without_replacing_snapshot() {
        let root = tempdir().expect("workspace");
        fs::write(root.path().join("a.rs"), "fn target() {}").expect("a");
        fs::write(root.path().join("b.rs"), "fn uses() { target(); }").expect("b");
        import_scip_bytes(root.path(), &binary()).expect("good");
        let mut broken = Index::parse_from_bytes(&binary()).expect("parse");
        broken.documents[1].occurrences[0].range = vec![-1, 0, 4];
        assert!(
            import_scip_bytes(root.path(), &broken.write_to_bytes().expect("serialized")).is_err()
        );
        assert_eq!(
            impact(root.path(), Path::new("a.rs"), 1, 8)
                .expect("previous snapshot")
                .hits
                .len(),
            1
        );
    }
}
