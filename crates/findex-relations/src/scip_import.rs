//! Official SCIP protobuf import. Bounded input is parsed in memory;
//! no arbitrary indexing executable or network request is launched.

use super::{
    admitted, language_for_path, persist, prepare, valid_occurrence, Evidence, ImportReport,
    Occurrence, Result, SemanticLink, SemanticSite, MAX_DOCUMENTS, MAX_INPUT_BYTES,
    MAX_OCCURRENCES, MAX_SYMBOL_LENGTH,
};
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
        for occurrence in &doc.occurrences {
            if occurrence.symbol.is_empty() {
                continue;
            }
            // The SCIP 0.10 Rust bindings expose compact 0-based range arrays:
            // [line, start, end] or [start_line, start, end_line, end].
            let start_line = match occurrence.range.as_slice() {
                [line, start, end] if *line >= 0 && *start >= 0 && end >= start => *line as usize,
                [line, start, end_line, end]
                    if *line >= 0 && *start >= 0 && end_line >= line && *end >= 0 =>
                {
                    *line as usize
                }
                _ => return Err("malformed SCIP occurrence range".into()),
            };
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
            let column = usize::try_from(*occurrence.range.get(1).ok_or("missing column")?)?;
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
