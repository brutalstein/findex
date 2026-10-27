//! Bounded, on-demand AST definition extraction.
//! This is structural discovery, NOT a precise LSP reference/call graph.

use findex_core::{is_sensitive_filename, MAX_SOURCE_BYTES};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Component, Path};
use tree_sitter::{Language, Node, Parser};

mod rust_receivers;
pub use rust_receivers::{receiver_hints as rust_receiver_hints, ReceiverHint};

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
        if let Some((kind, name_node)) = definition(label, node) {
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

/// Nearest ancestor kind, skipping wrappers that do not change scope.
fn enclosing_kind<'tree>(node: Node<'tree>, transparent: &[&str]) -> Option<&'tree str> {
    let mut parent = node.parent();
    while let Some(candidate) = parent {
        if !transparent.contains(&candidate.kind()) {
            return Some(candidate.kind());
        }
        parent = candidate.parent();
    }
    None
}

/// True when a C++ declarator (through pointer/reference wrappers) declares a function.
fn is_cpp_function_declarator(mut node: Node<'_>) -> bool {
    loop {
        match node.kind() {
            "function_declarator" => return true,
            "pointer_declarator" | "reference_declarator" => {
                match node.child_by_field_name("declarator") {
                    Some(inner) => node = inner,
                    None => return false,
                }
            }
            _ => return false,
        }
    }
}

fn is_function_value(node: Node<'_>) -> bool {
    node.child_by_field_name("value").is_some_and(|value| {
        matches!(
            value.kind(),
            "arrow_function" | "function_expression" | "function" | "generator_function"
        )
    })
}

/// Map a syntax node to a definition kind and its name node. Declarations
/// without bodies (C++ forward declarations, `struct stat st;`) are skipped.
fn definition<'tree>(language: &str, node: Node<'tree>) -> Option<(&'static str, Node<'tree>)> {
    let name = node.child_by_field_name("name");
    let kind = match (language, node.kind()) {
        ("Rust", "function_item") => {
            if enclosing_kind(node, &["declaration_list"])
                .is_some_and(|scope| matches!(scope, "impl_item" | "trait_item"))
            {
                "method"
            } else {
                "function"
            }
        }
        ("Rust", "function_signature_item") => "method",
        ("Rust", "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item") => "type",
        ("Rust", "mod_item") => "module",
        ("Rust", "macro_definition") => "macro",
        ("Rust", "const_item" | "static_item") => "constant",
        ("Rust", "impl_item") => {
            // `impl Trait for Type` is named by its implementing type.
            let target = node.child_by_field_name("type")?;
            let target = if target.kind() == "generic_type" {
                target.child_by_field_name("type")?
            } else {
                target
            };
            return Some(("impl", target));
        }
        ("Python", "function_definition") => {
            if enclosing_kind(node, &["block", "decorated_definition"]) == Some("class_definition")
            {
                "method"
            } else {
                "function"
            }
        }
        ("Python", "class_definition") => "class",
        (
            "TypeScript" | "JavaScript",
            "function_declaration" | "generator_function_declaration",
        ) => "function",
        ("TypeScript" | "JavaScript", "method_definition" | "abstract_method_signature") => {
            "method"
        }
        // Arrow functions bound to variables or class fields are definitions;
        // ordinary constants and fields are not.
        ("TypeScript" | "JavaScript", "variable_declarator") if is_function_value(node) => {
            "function"
        }
        ("TypeScript" | "JavaScript", "public_field_definition" | "field_definition")
            if is_function_value(node) =>
        {
            return node
                .child_by_field_name("name")
                .or_else(|| node.child_by_field_name("property"))
                .map(|name| ("method", name));
        }
        (
            "TypeScript" | "JavaScript",
            "interface_declaration" | "type_alias_declaration" | "enum_declaration",
        ) => "type",
        ("TypeScript" | "JavaScript", "class_declaration" | "abstract_class_declaration") => {
            "class"
        }
        ("C++", "function_definition") => {
            let declarator = node.child_by_field_name("declarator")?;
            let kind = if enclosing_kind(node, &["template_declaration"])
                == Some("field_declaration_list")
            {
                "method"
            } else {
                "function"
            };
            return cpp_declarator_name(declarator).map(|name| (kind, name));
        }
        // Method declarations inside class bodies, common in headers.
        ("C++", "field_declaration" | "declaration")
            if enclosing_kind(node, &["template_declaration"])
                == Some("field_declaration_list") =>
        {
            let declarator = node.child_by_field_name("declarator")?;
            if !is_cpp_function_declarator(declarator) {
                return None;
            }
            return cpp_declarator_name(declarator).map(|name| ("method", name));
        }
        ("C++", "class_specifier" | "struct_specifier" | "union_specifier")
            if node.child_by_field_name("body").is_some() =>
        {
            "class"
        }
        ("C++", "enum_specifier") if node.child_by_field_name("body").is_some() => "type",
        ("C++", "namespace_definition") => "module",
        _ => return None,
    };
    name.map(|name| (kind, name))
}

/// Expose the language capability gate without running an AST parser.
#[must_use]
pub fn supports_path(path: &Path) -> bool {
    language_for(path).is_some()
}

fn language_for(path: &Path) -> Option<(&'static str, Language)> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "rs" => Some(("Rust", tree_sitter_rust::LANGUAGE.into())),
        "py" | "pyi" => Some(("Python", tree_sitter_python::LANGUAGE.into())),
        "ts" | "mts" | "cts" => Some((
            "TypeScript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        )),
        "tsx" => Some(("TypeScript", tree_sitter_typescript::LANGUAGE_TSX.into())),
        // Plain JavaScript commonly contains JSX; the TSX grammar accepts both.
        "js" | "jsx" | "mjs" | "cjs" => {
            Some(("JavaScript", tree_sitter_typescript::LANGUAGE_TSX.into()))
        }
        "cc" | "cpp" | "cxx" | "hpp" | "hxx" | "hh" | "h" => {
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
    fn javascript_arrow_function_is_a_named_definition_not_a_constant() {
        let analysis = analyze(
            Path::new("app.ts"),
            "const execute = (x: number) => x + 1;\nconst setting = 10;\n",
        )
        .expect("parse")
        .expect("typescript");
        assert!(analysis
            .symbols
            .iter()
            .any(|s| s.name == "execute" && s.kind == "function"));
        assert!(!analysis.symbols.iter().any(|s| s.name == "setting"));
    }

    #[test]
    fn cpp_header_is_supported_for_definition_lookup() {
        let analysis = analyze(Path::new("engine.h"), "class Engine {};\n")
            .expect("parse")
            .expect("C++ header");
        assert!(analysis.symbols.iter().any(|s| s.name == "Engine"));
    }

    fn names(analysis: &Analysis) -> Vec<(&str, &str)> {
        analysis
            .symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), symbol.kind))
            .collect()
    }

    #[test]
    fn cpp_forward_declarations_and_elaborated_types_are_not_definitions() {
        let analysis = analyze(
            Path::new("io.cpp"),
            "class Widget;\nstruct Options;\nint size(const char* p) { struct stat st; return 0; }\n",
        )
        .expect("parse")
        .expect("cpp");
        assert_eq!(names(&analysis), vec![("size", "function")]);
    }

    #[test]
    fn cpp_namespaces_enums_and_declared_methods_are_indexed() {
        let analysis = analyze(
            Path::new("engine.hpp"),
            "namespace net {\nenum class Mode { A };\nclass Socket {\n public:\n  void open(int port);\n  template <typename T> T read();\n};\n}\n",
        )
        .expect("parse")
        .expect("cpp");
        let found = names(&analysis);
        for expected in [
            ("net", "module"),
            ("Mode", "type"),
            ("Socket", "class"),
            ("open", "method"),
            ("read", "method"),
        ] {
            assert!(
                found.contains(&expected),
                "{expected:?} missing in {found:?}"
            );
        }
    }

    #[test]
    fn rust_macros_constants_impls_and_trait_signatures_are_indexed() {
        let analysis = analyze(
            Path::new("lib.rs"),
            "macro_rules! ensure { () => {} }\nconst LIMIT: usize = 1;\nstatic NAME: &str = \"x\";\ntrait Store { fn get(&self) -> u8; }\nstruct Disk;\nimpl Store for Disk { fn get(&self) -> u8 { 0 } }\nunion Bits { a: u8 }\n",
        )
        .expect("parse")
        .expect("rust");
        let found = names(&analysis);
        for expected in [
            ("ensure", "macro"),
            ("LIMIT", "constant"),
            ("NAME", "constant"),
            ("Store", "type"),
            ("get", "method"),
            ("Disk", "type"),
            ("Bits", "type"),
        ] {
            assert!(
                found.contains(&expected),
                "{expected:?} missing in {found:?}"
            );
        }
        assert!(
            found
                .iter()
                .any(|(name, kind)| *kind == "impl" && *name == "Disk"),
            "impl block missing in {found:?}"
        );
    }

    #[test]
    fn python_methods_are_distinguished_from_module_functions() {
        let analysis = analyze(
            Path::new("app.py"),
            "def top():\n    pass\nclass Service:\n    @staticmethod\n    def build():\n        pass\n",
        )
        .expect("parse")
        .expect("python");
        let found = names(&analysis);
        assert!(found.contains(&("top", "function")));
        assert!(found.contains(&("build", "method")));
    }

    #[test]
    fn javascript_jsx_parses_without_syntax_errors() {
        let analysis = analyze(
            Path::new("view.js"),
            "export function View() { return <div className=\"x\">hi</div>; }\n",
        )
        .expect("parse")
        .expect("javascript");
        assert!(!analysis.has_syntax_errors);
        assert_eq!(names(&analysis), vec![("View", "function")]);
    }

    #[test]
    fn typescript_class_fields_with_arrow_functions_are_methods() {
        let analysis = analyze(
            Path::new("store.ts"),
            "export class Store {\n  load = async () => 1;\n  count = 0;\n}\n",
        )
        .expect("parse")
        .expect("ts");
        let found = names(&analysis);
        assert!(found.contains(&("load", "method")), "{found:?}");
        assert!(!found.iter().any(|(name, _)| *name == "count"));
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
