#![allow(clippy::expect_used)]
use findex_relations::{import_lsp_snapshot, import_scip_bytes, symbol_at};
use protobuf::Message;
use scip::types::{Document, Index, Occurrence, SymbolRole};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn occurrence(symbol: &str, line: i32, start: i32, end: i32, definition: bool) -> Occurrence {
    Occurrence {
        symbol: symbol.into(),
        range: vec![line, start, end],
        symbol_roles: if definition {
            SymbolRole::Definition as i32
        } else {
            0
        },
        ..Default::default()
    }
}

#[test]
fn position_selects_one_identity_without_merging_other_symbols() {
    let root = tempdir().expect("root");
    fs::write(
        root.path().join("main.rs"),
        "fn parse() {}\nfn parse_other() {}\nfn run() { parse(); parse_other(); }\n",
    )
    .expect("source");
    let index = Index {
        documents: vec![Document {
            relative_path: "main.rs".into(),
            language: "rust".into(),
            position_encoding: protobuf::EnumOrUnknown::from_i32(1),
            occurrences: vec![
                occurrence("one", 0, 3, 8, true),
                occurrence("two", 1, 3, 14, true),
                occurrence("one", 2, 11, 16, false),
                occurrence("two", 2, 20, 31, false),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    import_scip_bytes(root.path(), &index.write_to_bytes().expect("encode")).expect("import");
    let first = symbol_at(root.path(), Path::new("main.rs"), 3, 13, 32).expect("resolve");
    assert_eq!(first.resolution, "resolved");
    assert_eq!(first.symbols, vec!["one"]);
    let evidence = first.evidence.expect("evidence");
    assert!(evidence
        .sites
        .iter()
        .any(|s| s.kind == "definition" && s.line == 1));
    assert!(!evidence.sites.iter().any(|s| s.line == 2));
    assert_eq!(
        symbol_at(root.path(), Path::new("main.rs"), 3, 24, 32)
            .expect("second")
            .symbols,
        vec!["two"]
    );
    assert_eq!(
        symbol_at(root.path(), Path::new("main.rs"), 3, 16, 32)
            .expect("end exclusive")
            .resolution,
        "not_found"
    );
    fs::write(root.path().join("main.rs"), "fn changed() {}\n").expect("edit");
    let stale = symbol_at(root.path(), Path::new("main.rs"), 1, 4, 32).expect("stale");
    assert_eq!(stale.resolution, "stale");
    assert!(stale.symbols.is_empty() && stale.evidence.is_none());
}

#[test]
fn same_spelling_overloads_resolve_to_different_definition_sites() {
    let root = tempdir().expect("root");
    let source="class Parser {\n void parse(int n) {}\n void parse(String s) {}\n void run() { parse(1); parse(\"\"); }\n}\n";
    fs::write(root.path().join("Parser.java"), source).expect("source");
    let lines: Vec<_> = source.lines().collect();
    let first = lines[3].find("parse").expect("first");
    let second = lines[3].rfind("parse").expect("second");
    let index = Index {
        documents: vec![Document {
            relative_path: "Parser.java".into(),
            language: "java".into(),
            position_encoding: protobuf::EnumOrUnknown::from_i32(1),
            occurrences: vec![
                occurrence("parse(int)", 1, 6, 11, true),
                occurrence("parse(String)", 2, 6, 11, true),
                occurrence("parse(int)", 3, first as i32, (first + 5) as i32, false),
                occurrence(
                    "parse(String)",
                    3,
                    second as i32,
                    (second + 5) as i32,
                    false,
                ),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    import_scip_bytes(root.path(), &index.write_to_bytes().expect("encode")).expect("import");
    for (column, identity, definition_line) in [
        (first + 2, "parse(int)", 2),
        (second + 2, "parse(String)", 3),
    ] {
        let found =
            symbol_at(root.path(), Path::new("Parser.java"), 4, column, 32).expect("resolve");
        assert_eq!(found.symbols, vec![identity]);
        assert_eq!(
            found
                .evidence
                .expect("evidence")
                .sites
                .iter()
                .find(|s| s.kind == "definition")
                .expect("definition")
                .line,
            definition_line
        );
    }
}

#[test]
fn an_import_alias_resolves_to_the_original_identity() {
    let root = tempdir().expect("root");
    fs::write(root.path().join("lib.rs"), "pub fn original() {}\n").expect("definition");
    fs::write(
        root.path().join("use.rs"),
        "use crate::original as alias;\nfn run() { alias(); }\n",
    )
    .expect("alias");
    let index = Index {
        documents: vec![
            Document {
                relative_path: "lib.rs".into(),
                language: "rust".into(),
                position_encoding: protobuf::EnumOrUnknown::from_i32(1),
                occurrences: vec![occurrence("original-id", 0, 7, 15, true)],
                ..Default::default()
            },
            Document {
                relative_path: "use.rs".into(),
                language: "rust".into(),
                position_encoding: protobuf::EnumOrUnknown::from_i32(1),
                occurrences: vec![occurrence("original-id", 1, 11, 16, false)],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    import_scip_bytes(root.path(), &index.write_to_bytes().expect("encode")).expect("import");
    let found = symbol_at(root.path(), Path::new("use.rs"), 2, 13, 32).expect("resolve alias");
    assert_eq!(found.symbols, vec!["original-id"]);
    assert!(found
        .evidence
        .expect("evidence")
        .sites
        .iter()
        .any(|s| s.kind == "definition" && s.path == "lib.rs"));
}

#[test]
fn utf16_unicode_positions_map_to_utf8_and_lsp_points_stay_exact() {
    let root = tempdir().expect("root");
    fs::write(root.path().join("main.py"), "value = '😀'; target()\n").expect("source");
    let capture = r#"{"definition":{"path":"main.py","line":0,"character":14},"references":[]}"#;
    import_lsp_snapshot(root.path(), capture).expect("import");
    let point = symbol_at(root.path(), Path::new("main.py"), 1, 16, 32).expect("point");
    assert_eq!(point.resolution, "resolved");
    assert_eq!(point.symbols, vec!["lsp:main.py:0:14"]);
    assert_eq!(
        symbol_at(root.path(), Path::new("main.py"), 1, 17, 32)
            .expect("no invented extent")
            .resolution,
        "not_found"
    );
    assert!(symbol_at(root.path(), Path::new("main.py"), 1, 10, 32).is_err());
    assert!(symbol_at(root.path(), Path::new("../main.py"), 1, 0, 32).is_err());
}

#[test]
fn overlapping_symbols_are_ambiguous_and_no_snapshot_is_explicit() {
    let root = tempdir().expect("root");
    fs::write(
        root.path().join("main.go"),
        "package main\nfunc target() {}\n",
    )
    .expect("source");
    assert_eq!(
        symbol_at(root.path(), Path::new("main.go"), 2, 6, 32)
            .expect("no import")
            .resolution,
        "no_snapshot"
    );
    let index = Index {
        documents: vec![Document {
            relative_path: "main.go".into(),
            language: "go".into(),
            position_encoding: protobuf::EnumOrUnknown::from_i32(1),
            occurrences: vec![
                occurrence("a", 1, 5, 11, true),
                occurrence("b", 1, 5, 11, true),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    import_scip_bytes(root.path(), &index.write_to_bytes().expect("encode")).expect("import");
    let result = symbol_at(root.path(), Path::new("main.go"), 2, 6, 32).expect("resolve");
    assert_eq!(result.resolution, "ambiguous");
    assert_eq!(result.symbols.len(), 2);
    assert!(result.evidence.is_none());
    let capped = symbol_at(root.path(), Path::new("main.go"), 2, 6, 1).expect("cap");
    assert!(capped.truncated && capped.evidence.is_none());
    assert_eq!(capped.resolution, "incomplete");
}

#[test]
fn scip_utf8_utf16_utf32_ranges_and_unspecified_unicode_are_distinct() {
    let root = tempdir().expect("root");
    let source = "text = '😀'; target()\n";
    fs::write(root.path().join("main.py"), source).expect("source");
    let start = source.find("target").expect("token");
    for encoding in 0..=3 {
        let units = |s: &str| match encoding {
            2 => s.encode_utf16().count(),
            3 => s.chars().count(),
            _ => s.len(),
        };
        let index = Index {
            documents: vec![Document {
                relative_path: "main.py".into(),
                language: "python".into(),
                position_encoding: protobuf::EnumOrUnknown::from_i32(encoding),
                occurrences: vec![occurrence(
                    "target",
                    0,
                    units(&source[..start]) as i32,
                    units(&source[..start + 6]) as i32,
                    true,
                )],
                ..Default::default()
            }],
            ..Default::default()
        };
        import_scip_bytes(root.path(), &index.write_to_bytes().expect("encode")).expect("import");
        let found = symbol_at(root.path(), Path::new("main.py"), 1, start + 2, 32).expect("query");
        assert_eq!(
            found.resolution,
            if encoding == 0 {
                "incomplete"
            } else {
                "resolved"
            },
            "encoding={encoding}"
        );
        if encoding == 0 {
            assert!(found.evidence.is_none());
        }
        if encoding == 2 {
            let mut bad = index.clone();
            let emoji = source.find('😀').expect("emoji");
            bad.documents[0].occurrences[0].range[1] =
                (source[..emoji].encode_utf16().count() + 1) as i32;
            assert!(
                import_scip_bytes(root.path(), &bad.write_to_bytes().expect("encode invalid"))
                    .is_err()
            );
            assert_eq!(
                symbol_at(root.path(), Path::new("main.py"), 1, start + 2, 32)
                    .expect("previous snapshot")
                    .resolution,
                "resolved"
            );
        }
    }
}

#[test]
fn multiline_ranges_exclude_their_end_and_current_ignore_rules_win() {
    let root = tempdir().expect("root");
    fs::write(root.path().join("main.rs"), "fn target(\n) {}\n").expect("source");
    let index = Index {
        documents: vec![Document {
            relative_path: "main.rs".into(),
            language: "rust".into(),
            position_encoding: protobuf::EnumOrUnknown::from_i32(1),
            occurrences: vec![Occurrence {
                symbol: "target".into(),
                symbol_roles: SymbolRole::Definition as i32,
                range: vec![0, 3, 1, 1],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    import_scip_bytes(root.path(), &index.write_to_bytes().expect("encode")).expect("import");
    assert_eq!(
        symbol_at(root.path(), Path::new("main.rs"), 2, 0, 32)
            .expect("inside")
            .resolution,
        "resolved"
    );
    assert_eq!(
        symbol_at(root.path(), Path::new("main.rs"), 2, 1, 32)
            .expect("end")
            .resolution,
        "not_found"
    );
    fs::write(root.path().join(".gitignore"), "main.rs\n").expect("ignore");
    let ignored = symbol_at(root.path(), Path::new("main.rs"), 1, 4, 32).expect("ignored");
    assert_eq!(ignored.resolution, "stale");
    assert!(ignored.symbols.is_empty() && ignored.evidence.is_none());
}

#[test]
fn compiler_ranges_may_end_at_the_empty_line_after_a_final_newline() {
    let root = tempdir().expect("root");
    fs::write(root.path().join("main.rs"), "fn target() {}\n").expect("source");
    let index = Index {
        documents: vec![Document {
            relative_path: "main.rs".into(),
            language: "rust".into(),
            position_encoding: protobuf::EnumOrUnknown::from_i32(1),
            occurrences: vec![Occurrence {
                symbol: "module".into(),
                symbol_roles: SymbolRole::Definition as i32,
                range: vec![0, 0, 1, 0],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    import_scip_bytes(root.path(), &index.write_to_bytes().expect("encode"))
        .expect("valid EOF range");
    assert_eq!(
        symbol_at(root.path(), Path::new("main.rs"), 1, 4, 32)
            .expect("inside")
            .resolution,
        "resolved"
    );
    assert_eq!(
        symbol_at(root.path(), Path::new("main.rs"), 2, 0, 32)
            .expect("EOF end excluded")
            .resolution,
        "not_found"
    );
}

#[test]
fn token_ranges_take_precedence_over_enclosing_module_ranges() {
    let root = tempdir().expect("root");
    fs::write(root.path().join("main.rs"), "fn target() {}\n").expect("source");
    let index = Index {
        documents: vec![Document {
            relative_path: "main.rs".into(),
            language: "rust".into(),
            position_encoding: protobuf::EnumOrUnknown::from_i32(1),
            occurrences: vec![
                Occurrence {
                    symbol: "module".into(),
                    symbol_roles: SymbolRole::Definition as i32,
                    range: vec![0, 0, 1, 0],
                    ..Default::default()
                },
                occurrence("target", 0, 3, 9, true),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    import_scip_bytes(root.path(), &index.write_to_bytes().expect("encode")).expect("import");
    let found = symbol_at(root.path(), Path::new("main.rs"), 1, 4, 32).expect("query");
    assert_eq!(found.resolution, "resolved");
    assert_eq!(found.symbols, vec!["target"]);
}
