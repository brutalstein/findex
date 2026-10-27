//! Local type annotations are syntax evidence, never compiler name resolution.
use tree_sitter::{Node, Parser};

// Bound repeated lexical-scope walks on large, adversarial source files.
const MAX_SCOPE_STEPS: usize = 200_000;

#[derive(Debug, PartialEq)]
pub struct ReceiverHint {
    /// Method-token line, even when the receiver starts on an earlier line.
    pub line: usize,
    /// Local annotation's last type component, not a resolved symbol identity.
    pub receiver_type: Option<String>,
}

pub fn receiver_hints(source: &str, method: &str) -> Result<Vec<ReceiverHint>, String> {
    if source.len() as u64 > findex_core::MAX_SOURCE_BYTES || source.as_bytes().contains(&0) {
        return Err("source exceeds size limit or contains binary data".into());
    }
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|error| error.to_string())?;
    let tree = parser
        .parse(source, None)
        .ok_or("Rust parser did not finish")?;
    if tree.root_node().has_error() {
        return Err("receiver annotations unavailable for incomplete Rust syntax".into());
    }
    let mut pending = vec![tree.root_node()];
    let mut hints = Vec::new();
    let mut scope_steps = 0;
    while let Some(node) = pending.pop() {
        if node.kind() == "call_expression" {
            let function = node.child_by_field_name("function");
            let function = function.and_then(|n| {
                if n.kind() == "generic_function" {
                    n.child_by_field_name("function")
                } else {
                    Some(n)
                }
            });
            if let Some(function) = function.filter(|n| n.kind() == "field_expression") {
                let field = function.child_by_field_name("field");
                let name = field.and_then(|n| n.utf8_text(source.as_bytes()).ok());
                if name == Some(method) {
                    let receiver = function.child_by_field_name("value");
                    let ty = receiver
                        .filter(|n| n.kind() == "identifier")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .and_then(|name| annotated_type(node, name, source, &mut scope_steps))
                        .filter(|ty| !type_parameter(node, ty, source));
                    if scope_steps > MAX_SCOPE_STEPS {
                        return Err("receiver scope analysis budget exceeded".into());
                    }
                    hints.push(ReceiverHint {
                        line: field.map_or(node.start_position().row, |n| n.start_position().row)
                            + 1,
                        receiver_type: ty,
                    });
                    if hints.len() > 2048 {
                        return Err("receiver hint limit exceeded".into());
                    }
                }
            }
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    hints.sort_by_key(|hint| hint.line);
    Ok(hints)
}

fn binds(pattern: Node<'_>, name: &str, source: &str) -> bool {
    let mut pending = vec![pattern];
    while let Some(node) = pending.pop() {
        if node.kind() == "identifier" && node.utf8_text(source.as_bytes()).ok() == Some(name) {
            return true;
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    false
}

fn type_name(mut node: Node<'_>, source: &str) -> Option<String> {
    while matches!(node.kind(), "reference_type" | "generic_type") {
        node = node.child_by_field_name("type")?;
    }
    if node.kind() == "scoped_type_identifier" {
        node = node.child_by_field_name("name")?;
    }
    if !matches!(node.kind(), "type_identifier" | "primitive_type") {
        return None;
    }
    let name = node.utf8_text(source.as_bytes()).ok()?;
    // A one-letter type parameter is not a resolved concrete receiver.
    (name.len() > 1).then(|| name.to_owned())
}

fn type_parameter(call: Node<'_>, name: &str, source: &str) -> bool {
    let mut ancestor = Some(call);
    while let Some(node) = ancestor {
        if let Some(parameters) = node.child_by_field_name("type_parameters") {
            let mut cursor = parameters.walk();
            if parameters.named_children(&mut cursor).any(|p| {
                p.kind() == "type_parameter"
                    && p.child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        == Some(name)
            }) {
                return true;
            }
        }
        ancestor = node.parent();
    }
    false
}

fn binding_type(node: Node<'_>, source: &str) -> Option<String> {
    let mut pattern = node.child_by_field_name("pattern")?;
    if matches!(pattern.kind(), "mut_pattern" | "ref_pattern") {
        pattern = pattern.named_child(0)?;
    }
    if pattern.kind() != "identifier" {
        return None;
    }
    node.child_by_field_name("type")
        .and_then(|n| type_name(n, source))
}

fn condition_binds(condition: Node<'_>, name: &str, source: &str) -> bool {
    let mut pending = vec![condition];
    while let Some(node) = pending.pop() {
        if node.kind() == "let_condition"
            && node
                .child_by_field_name("pattern")
                .is_some_and(|p| binds(p, name, source))
        {
            return true;
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    false
}

fn annotated_type(call: Node<'_>, name: &str, source: &str, steps: &mut usize) -> Option<String> {
    let mut ancestor = call.parent();
    while let Some(scope) = ancestor {
        *steps += 1;
        if *steps > MAX_SCOPE_STEPS {
            return None;
        }
        if scope.kind() == "block" {
            let mut cursor = scope.walk();
            let mut binding = None;
            for declaration in scope.named_children(&mut cursor) {
                *steps += 1;
                if *steps > MAX_SCOPE_STEPS {
                    return None;
                }
                if declaration.start_byte() >= call.start_byte() {
                    break;
                }
                if declaration.kind() == "let_declaration"
                    && declaration.end_byte() <= call.start_byte()
                    && declaration
                        .child_by_field_name("pattern")
                        .is_some_and(|p| binds(p, name, source))
                {
                    binding = Some(binding_type(declaration, source));
                }
            }
            if let Some(ty) = binding {
                return ty;
            }
        }
        let capture = match scope.kind() {
            "match_arm" => scope.child_by_field_name("pattern"),
            "for_expression" => scope
                .child_by_field_name("body")
                .filter(|body| {
                    body.start_byte() <= call.start_byte() && call.end_byte() <= body.end_byte()
                })
                .and_then(|_| scope.child_by_field_name("pattern")),
            "if_expression" | "while_expression" => scope
                .child_by_field_name("consequence")
                .or_else(|| scope.child_by_field_name("body"))
                .filter(|body| {
                    body.start_byte() <= call.start_byte() && call.end_byte() <= body.end_byte()
                })
                .and_then(|_| scope.child_by_field_name("condition"))
                .filter(|condition| condition_binds(*condition, name, source)),
            _ => None,
        };
        if capture.is_some_and(|pattern| binds(pattern, name, source)) {
            return None;
        }
        if matches!(scope.kind(), "function_item" | "closure_expression") {
            if let Some(parameters) = scope.child_by_field_name("parameters") {
                let mut cursor = parameters.walk();
                for parameter in parameters.named_children(&mut cursor) {
                    let pattern = parameter
                        .child_by_field_name("pattern")
                        .unwrap_or(parameter);
                    if binds(pattern, name, source) {
                        return binding_type(parameter, source);
                    }
                }
            }
            if scope.kind() == "function_item" {
                return None;
            }
        }
        ancestor = scope.parent();
    }
    None
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn annotations_respect_function_block_closure_and_initializer_scope() {
        let source = "fn run(x: &Disk, socket: Socket) {\n x.flush();\n socket.flush();\n { let x = unknown();\n x.flush(); }\n x.flush();\n let c = |x| { x.flush(); };\n let x = x.flush();\n x.flush();\n}\nfn other(x: Network) { x.flush(); }\n";
        assert_eq!(
            receiver_hints(source, "flush").expect("parse"),
            vec![
                ReceiverHint {
                    line: 2,
                    receiver_type: Some("Disk".into())
                },
                ReceiverHint {
                    line: 3,
                    receiver_type: Some("Socket".into())
                },
                ReceiverHint {
                    line: 5,
                    receiver_type: None
                },
                ReceiverHint {
                    line: 6,
                    receiver_type: Some("Disk".into())
                },
                ReceiverHint {
                    line: 7,
                    receiver_type: None
                },
                ReceiverHint {
                    line: 8,
                    receiver_type: Some("Disk".into())
                },
                ReceiverHint {
                    line: 9,
                    receiver_type: None
                },
                ReceiverHint {
                    line: 11,
                    receiver_type: Some("Network".into())
                },
            ]
        );
    }

    #[test]
    fn no_type_is_invented_from_receiver_names_field_chains_or_strings() {
        let source = "fn run(disk: Unknown) {\n self.disk.flush();\n get_disk().flush();\n let s = \"disk.flush()\";\n disk.flush();\n}\n";
        let found = receiver_hints(source, "flush").expect("parse");
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].receiver_type, None);
        assert_eq!(found[1].receiver_type, None);
        assert_eq!(found[2].receiver_type.as_deref(), Some("Unknown"));
    }

    #[test]
    fn generic_parameters_and_destructuring_are_not_concrete_receiver_types() {
        let source = "fn run<Disk>(x: &Disk) { x.flush(); }\nfn other() { let Holder { value: x }: Holder = make(); x.flush(); }\n";
        let hints = receiver_hints(source, "flush").expect("parse");
        assert_eq!(hints.len(), 2);
        assert!(hints.iter().all(|hint| hint.receiver_type.is_none()));
    }

    #[test]
    fn loop_match_and_conditional_bindings_mask_outer_annotations() {
        let source = "fn run(x: &Disk) {\nfor x in items { x.flush(); }\nif let Some(x) = item { x.flush(); } else { x.flush(); }\nmatch item { Some(x) => x.flush(), _ => x.flush() };\nx.flush();\n}";
        let hints = receiver_hints(source, "flush").expect("parse");
        assert_eq!(hints.len(), 6);
        let mut types: Vec<_> = hints.iter().map(|h| h.receiver_type.as_deref()).collect();
        types.sort();
        assert_eq!(
            types,
            vec![None, None, None, Some("Disk"), Some("Disk"), Some("Disk")]
        );
    }

    #[test]
    fn invalid_oversized_and_over_budget_sources_refuse_receiver_certainty() {
        assert!(receiver_hints("fn broken(", "flush").is_err());
        assert!(receiver_hints("fn run() {}\0", "flush").is_err());
        assert!(receiver_hints(
            &" ".repeat(findex_core::MAX_SOURCE_BYTES as usize + 1),
            "flush"
        )
        .is_err());
        let source = format!("fn run(x: &Disk) {{ {} }}", "x.flush();\n".repeat(2049));
        assert!(receiver_hints(&source, "flush").is_err());
    }

    #[test]
    fn method_token_line_and_generic_annotations_are_preserved() {
        let hints = receiver_hints(
            "fn run(x: &crate::Disk<u8>) {\n x\n .flush::<u8>();\n}",
            "flush",
        )
        .expect("parse");
        assert_eq!(
            hints,
            vec![ReceiverHint {
                line: 3,
                receiver_type: Some("Disk".into())
            }]
        );
    }
}
