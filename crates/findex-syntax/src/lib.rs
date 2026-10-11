//! Bounded, on-demand AST definition extraction.
//! This is structural discovery, NOT a precise LSP reference/call graph.

use findex_core::{is_sensitive_filename, MAX_SOURCE_BYTES};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Component, Path};
use tree_sitter::{Language, Node, Parser};

const MAX_SYMBOLS: usize = 2048;

#[derive(Clone, Debug, Serialize)]
pub struct Symbol {
    pub name: String,
    pub kind: &'static str,
    /// 1-based source line of this definition.
    pub start_line: usize,
    /// Inclusive, 1-based end line.
    pub end_line: usize,
}

#[derive(Debug, Serialize)]
pub struct Analysis {
    pub language: &'static str,
    pub has_syntax_errors: bool,
    pub symbols: Vec<Symbol>,
    /// Parsing is best-effort; name resolution and references require LSP/SCIP.
    pub capability: &'static str,
    pub truncated: bool,
}

/// Inspect exactly one explicitly requested file, rather than walking the repo.
/// Paths must be relative to the canonical workspace, with no traversals.
pub fn inspect_file(project: &Path, relative: &Path) -> io::Result<Option<Analysis>> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a normal project-relative file path",
        ));
    }
    let root = project.canonicalize()?;
    let candidate = root.join(relative);
    if is_sensitive_filename(&candidate) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "credential and private-key files are excluded",
        ));
    }
    let meta = fs::symlink_metadata(&candidate)?;
    if !meta.file_type().is_file() || meta.len() > MAX_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not an eligible source file, or larger than the source budget",
        ));
    }
    if !candidate.canonicalize()?.starts_with(&root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "file escapes the workspace",
        ));
    }
    // Unknown extensions are not parsed or sent to an external service.
    if language_for(&candidate).is_none() {
        return Ok(None);
    }
    let bytes = fs::read(&candidate)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES || bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized or binary source file",
        ));
    }
    let source = std::str::from_utf8(&bytes).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("source is not UTF-8: {err}"),
        )
    })?;
    analyze(&candidate, source).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

/// A source-only analyzer, useful for deterministic tests and later index updates.
pub fn analyze(path: &Path, source: &str) -> Result<Option<Analysis>, String> {
    let Some((label, language)) = language_for(path) else {
        return Ok(None);
    };
    if source.len() as u64 > MAX_SOURCE_BYTES || source.as_bytes().contains(&0) {
        return Err("source exceeds size limit or contains binary data".into());
    }
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|err| format!("incompatible grammar: {err}"))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "Tree-sitter parser did not finish".to_owned())?;
    let mut symbols = Vec::new();
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        if let Some(kind) = symbol_kind(label, node.kind()) {
            let name_node = if label == "C++" && node.kind() == "function_definition" {
                node.child_by_field_name("declarator")
                    .and_then(cpp_declarator_name)
            } else {
                node.child_by_field_name("name")
            };
            if let Some(name_node) = name_node {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    if !name.is_empty() && name.len() <= 256 {
                        let start_line = node.start_position().row + 1;
                        let end = node.end_position();
                        let end_line = if end.column == 0 && end.row + 1 > start_line {
                            end.row
                        } else {
                            end.row + 1
                        };
                        symbols.push(Symbol {
                            name: name.to_owned(),
                            kind,
                            start_line,
                            end_line,
                        });
                        if symbols.len() >= MAX_SYMBOLS {
                            return Ok(Some(Analysis {
                                language: label,
                                has_syntax_errors: tree.root_node().has_error(),
                                symbols,
                                capability: "syntax-definitions-only",
                                truncated: true,
                            }));
                        }
                    }
                }
            }
        }
        let mut cursor = node.walk();
        let children = node.named_children(&mut cursor).collect::<Vec<_>>();
        pending.extend(children.into_iter().rev());
    }
    Ok(Some(Analysis {
        language: label,
        has_syntax_errors: tree.root_node().has_error(),
        symbols,
        capability: "syntax-definitions-only",
        truncated: false,
    }))
}

fn cpp_declarator_name<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    if matches!(
        node.kind(),
        "identifier" | "field_identifier" | "operator_name" | "destructor_name"
    ) {
        return Some(node);
    }
    if let Some(child) = node
        .child_by_field_name("declarator")
        .or_else(|| node.child_by_field_name("name"))
    {
        return cpp_declarator_name(child);
    }
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    for child in children.into_iter().rev() {
        if child.kind().contains("parameter") {
            continue;
        }
        if let Some(name) = cpp_declarator_name(child) {
            return Some(name);
        }
    }
    None
}

fn symbol_kind(language: &str, kind: &str) -> Option<&'static str> {
    match (language, kind) {
        ("Rust", "function_item")
        | ("Python", "function_definition")
        | ("TypeScript", "function_declaration")
        | ("TypeScript", "generator_function_declaration")
        | ("C++", "function_definition") => Some("function"),
        ("TypeScript", "method_definition") => Some("method"),
        ("Rust", "struct_item")
        | ("Rust", "enum_item")
        | ("Rust", "trait_item")
        | ("Rust", "type_item")
        | ("TypeScript", "interface_declaration")
        | ("TypeScript", "type_alias_declaration")
        | ("TypeScript", "enum_declaration") => Some("type"),
        ("Python", "class_definition")
        | ("TypeScript", "class_declaration")
        | ("TypeScript", "abstract_class_declaration")
        | ("C++", "class_specifier")
        | ("C++", "struct_specifier") => Some("class"),
        ("Rust", "mod_item") => Some("module"),
        _ => None,
    }
}

fn language_for(path: &Path) -> Option<(&'static str, Language)> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "rs" => Some(("Rust", tree_sitter_rust::LANGUAGE.into())),
        "py" | "pyi" => Some(("Python", tree_sitter_python::LANGUAGE.into())),
        "ts" | "mts" | "cts" | "js" | "mjs" | "cjs" => Some((
            "TypeScript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        )),
        "tsx" | "jsx" => Some(("TypeScript", tree_sitter_typescript::LANGUAGE_TSX.into())),
        "cc" | "cpp" | "cxx" | "hpp" | "hxx" | "hh" => {
            Some(("C++", tree_sitter_cpp::LANGUAGE.into()))
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rust_definitions_and_line_ranges() {
        let result = analyze(
            Path::new("lib.rs"),
            "struct Engine;\nfn search() {\n  println!(\"ok\");\n}\n",
        )
        .expect("parse")
        .expect("rust");
        assert_eq!(result.language, "Rust");
        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].name, "Engine");
        assert_eq!(result.symbols[1].name, "search");
        assert_eq!(result.symbols[1].start_line, 2);
        assert_eq!(result.symbols[1].end_line, 4);
        assert!(!result.has_syntax_errors);
    }

    #[test]
    fn python_class_and_method() {
        let result = analyze(
            Path::new("app.py"),
            "class Service:\n    def connect(self):\n        pass\n",
        )
        .expect("parse")
        .expect("python");
        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].name, "Service");
        assert_eq!(result.symbols[1].name, "connect");
    }

    #[test]
    fn typescript_interface_and_function() {
        let result = analyze(
            Path::new("app.ts"),
            "interface User { id: string }\nfunction connect(): void {}\n",
        )
        .expect("parse")
        .expect("ts");
        assert_eq!(result.symbols.len(), 2);
        assert_eq!(result.symbols[0].name, "User");
        assert_eq!(result.symbols[1].name, "connect");
    }

    #[test]
    fn cpp_class_and_free_function() {
        let result = analyze(
            Path::new("app.cpp"),
            "class Driver {};\nint connect(int code) { return code; }\n",
        )
        .expect("parse")
        .expect("cpp");
        assert!(result.symbols.iter().any(|symbol| symbol.name == "Driver"));
        assert!(result.symbols.iter().any(|symbol| symbol.name == "connect"));
    }

    #[test]
    fn unsupported_extension_and_size_limit() {
        assert!(analyze(Path::new("README.md"), "# hi")
            .expect("unsupported")
            .is_none());
        assert!(analyze(
            Path::new("main.rs"),
            &"x".repeat(MAX_SOURCE_BYTES as usize + 1)
        )
        .is_err());
    }

    #[test]
    fn file_access_rejects_traversal_and_secrets() {
        let workspace = tempdir().expect("workspace");
        assert!(inspect_file(workspace.path(), Path::new("../outside.rs")).is_err());
        std::fs::write(workspace.path().join(".env"), "secret").expect("write");
        assert!(inspect_file(workspace.path(), Path::new(".env")).is_err());
        std::fs::write(workspace.path().join("app.rs"), "fn start() {}").expect("write");
        let result = inspect_file(workspace.path(), Path::new("app.rs"))
            .expect("inspect")
            .expect("rust");
        assert_eq!(result.symbols[0].name, "start");
    }

    #[test]
    fn parser_reports_syntax_errors_without_claiming_type_resolution() {
        let parsed = analyze(Path::new("bad.py"), "def broken(:\n    pass\n")
            .expect("parser")
            .expect("python");
        assert!(parsed.has_syntax_errors);
        assert_eq!(parsed.capability, "syntax-definitions-only");
    }
}
