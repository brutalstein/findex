//! Task-oriented context assembly: one call returns the minimum source-backed
//! evidence for a coding task (definitions, call sites, tests, concept hits)
//! within a token budget, and states what is missing or truncated.
//!
//! Relationship kinds are named for what was actually checked. Call sites are
//! lexical name matches followed by a call token, not type-resolved calls.

use crate::symbols::{containing, definitions_by_identifier};
use crate::{connect, excerpt_of, live_source, search_in_db, synchronize, Result, SyncStats};
use findex_core::storage_dir;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

/// Files scanned for sites of one identifier. More candidates are reported
/// as truncation instead of being read.
const MAX_SITE_FILES: usize = 400;
const MAX_IDENTIFIERS: usize = 3;
const MAX_BUDGET: usize = 8000;
/// Lines shown from a definition body when the task asks for the definition.
const MAX_BODY_LINES: usize = 40;

#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    /// definition | call | reference | import | match | mention (non-code file)
    pub kind: &'static str,
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    /// Innermost syntax definition containing `line`, if any.
    pub enclosing: Option<String>,
    pub in_test: bool,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct ContextReport {
    /// definition | callers | tests | impact | concept
    pub intent: &'static str,
    pub identifiers: Vec<String>,
    pub evidence: Vec<Evidence>,
    /// Requested information that has no supporting evidence.
    pub missing: Vec<String>,
    /// True when evidence was left out because of the budget or a scan cap.
    pub truncated: bool,
    pub budget_tokens: usize,
    /// Estimated as rendered characters / 4; not a model tokenizer count.
    pub used_tokens: usize,
    pub stale_results_discarded: usize,
    pub sync: SyncStats,
}

/// Standalone entry point: reconcile the project, then answer.
pub fn context(project: &Path, task: &str, budget_tokens: usize) -> Result<ContextReport> {
    let sync = synchronize(project)?;
    let database = storage_dir(project)?.join("index.sqlite3");
    context_in_db(project, &database, task, budget_tokens, sync)
}

pub fn context_in_db(
    project: &Path,
    database: &Path,
    task: &str,
    budget_tokens: usize,
    sync: SyncStats,
) -> Result<ContextReport> {
    let words = words(task);
    let identifiers = identifiers(task);
    let intent = match classify(&words) {
        "definition" if identifiers.is_empty() => "concept",
        intent => intent,
    };
    let mut report = ContextReport {
        intent,
        identifiers,
        evidence: Vec::new(),
        missing: Vec::new(),
        truncated: false,
        budget_tokens: budget_tokens.clamp(200, MAX_BUDGET),
        used_tokens: 0,
        stale_results_discarded: 0,
        sync,
    };
    let root = project.canonicalize()?;
    let conn = connect(database)?;
    let mut candidates = Vec::new();
    if report.identifiers.is_empty() {
        concept(&root, database, &words, &conn, &mut report, &mut candidates)?;
    } else {
        for identifier in report.identifiers.clone() {
            let found = definitions(&root, &conn, &identifier, &mut report)?;
            let mut sites = Vec::new();
            if report.intent != "definition" || found.is_empty() {
                sites = sites_of(&root, &conn, &identifier, &found, &mut report)?;
            }
            if report.intent == "definition" && sites.iter().any(|s| s.kind == "definition") {
                sites.retain(|s| s.kind == "definition");
            }
            if found.is_empty() && !sites.iter().any(|s| s.kind == "definition") {
                report.missing.push(format!(
                    "no definition of `{identifier}` found (syntax or `fn`/`def`/`class` text)"
                ));
            }
            if report.intent != "definition" {
                let free_rust_function = !found.is_empty()
                    && !sites.iter().any(|s| s.kind == "definition")
                    && found.iter().all(|d| {
                        d.path.ends_with(".rs")
                            && d.enclosing
                                .as_deref()
                                .is_some_and(|e| e.starts_with("function "))
                    });
                if free_rust_function {
                    // A Rust free function cannot be called as `.name(`; such
                    // lines call some other type's method of the same name.
                    let before = sites.len();
                    sites.retain(|s| {
                        !(s.path.ends_with(".rs") && only_method_syntax(&s.text, &identifier))
                    });
                    if sites.len() < before {
                        report.missing.push(format!(
                            "excluded {} method-syntax `.{identifier}(` lines: a Rust free function cannot be called that way",
                            before - sites.len()
                        ));
                    }
                }
                let defined = found.len() + sites.iter().filter(|s| s.kind == "definition").count();
                if defined > 1 {
                    report.missing.push(format!(
                        "`{identifier}` has {defined} definitions; call sites are name matches, not type-resolved, and may belong to any of them"
                    ));
                }
                if sites.iter().all(|s| s.kind == "definition") {
                    report.missing.push(format!(
                        "no code mentions `{identifier}` outside its definition"
                    ));
                } else if report.intent == "tests" && !sites.iter().any(|s| s.in_test) {
                    report
                        .missing
                        .push(format!("no test code mentions `{identifier}`"));
                }
            }
            candidates.extend(found);
            candidates.extend(sites);
        }
    }
    pack(candidates, &mut report);
    Ok(report)
}

const STOPWORDS: &[&str] = &[
    "a",
    "an",
    "and",
    "any",
    "are",
    "as",
    "at",
    "be",
    "by",
    "can",
    "code",
    "codebase",
    "do",
    "does",
    "explain",
    "file",
    "files",
    "find",
    "for",
    "from",
    "function",
    "functions",
    "get",
    "how",
    "i",
    "if",
    "in",
    "is",
    "it",
    "its",
    "list",
    "locate",
    "me",
    "method",
    "methods",
    "of",
    "on",
    "or",
    "please",
    "repo",
    "repository",
    "show",
    "so",
    "that",
    "the",
    "their",
    "there",
    "this",
    "to",
    "we",
    "what",
    "when",
    "where",
    "which",
    "who",
    "why",
    "with",
    "you",
];
const CALLER_WORDS: &[&str] = &[
    "call",
    "called",
    "caller",
    "callers",
    "calls",
    "invoke",
    "invoked",
    "invokes",
    "reference",
    "referenced",
    "references",
    "usage",
    "usages",
    "use",
    "used",
    "uses",
];
const TEST_WORDS: &[&str] = &["spec", "specs", "test", "tested", "testing", "tests"];
const IMPACT_WORDS: &[&str] = &[
    "affect", "affected", "affects", "break", "breaks", "change", "changing", "impact", "modify",
    "refactor", "rename", "ripple",
];

fn words(task: &str) -> Vec<String> {
    task.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn classify(words: &[String]) -> &'static str {
    let has = |set: &[&str]| words.iter().any(|w| set.contains(&w.as_str()));
    if has(IMPACT_WORDS) {
        "impact"
    } else if has(TEST_WORDS) {
        "tests"
    } else if has(CALLER_WORDS) {
        "callers"
    } else {
        "definition"
    }
}

/// Code identifiers named in the task: backticked names, snake_case and
/// camelCase/PascalCase tokens, and the last segment of `a::b` / `a.b`.
/// Plain English words are never treated as identifiers.
fn identifiers(task: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut push = |name: &str| {
        let name = name.trim_matches(|c: char| !(c.is_alphanumeric() || c == '_'));
        if name.chars().count() >= 3
            && name
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
            && !found.iter().any(|f| f == name)
            && found.len() < MAX_IDENTIFIERS
        {
            found.push(name.to_string());
        }
    };
    for (index, part) in task.split('`').enumerate() {
        if index % 2 == 1 {
            if let Some(last) = part
                .trim_end_matches("()")
                .rsplit(|c: char| c == ':' || c == '.' || c.is_whitespace())
                .find(|s| !s.is_empty())
            {
                push(last);
            }
        }
    }
    for token in task.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        // camelCase / PascalCase need a lower-to-upper step; `SQLite` and
        // `HTTP` are words.
        let chars: Vec<char> = token.chars().collect();
        let inner_upper = chars
            .windows(2)
            .any(|pair| pair[0].is_lowercase() && pair[1].is_uppercase());
        let snake = token.contains('_') && token.chars().any(char::is_alphabetic);
        if inner_upper || snake {
            push(token);
        }
    }
    found
}

fn definitions(
    root: &Path,
    conn: &Connection,
    identifier: &str,
    report: &mut ContextReport,
) -> Result<Vec<Evidence>> {
    let key = identifier.to_lowercase().replace('_', "");
    let mut rows = definitions_by_identifier(conn, &key, 16)?;
    // Exact spelling first, so `poll_proceed` outranks a `PollProceed` type.
    rows.sort_by(|a, b| {
        (a.name != identifier, &a.path, a.start_line).cmp(&(
            b.name != identifier,
            &b.path,
            b.start_line,
        ))
    });
    let mut found = Vec::new();
    for row in rows {
        let Some(text) = live_source(root, &row.path, &row.hash) else {
            report.stale_results_discarded += 1;
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        let start = row.start_line.clamp(1, lines.len().max(1));
        let end = row.end_line.clamp(start, lines.len().max(start));
        let shown_end = if report.intent == "definition" {
            end.min(start + MAX_BODY_LINES - 1)
        } else {
            start
        };
        let body = lines
            .get(start - 1..shown_end.min(lines.len()))
            .unwrap_or_default()
            .iter()
            .map(|line| findex_core::redact_secrets(line).into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        found.push(Evidence {
            kind: "definition",
            enclosing: Some(format!("{} {}", row.kind, row.name)),
            in_test: is_test_path(&row.path),
            path: row.path,
            line: start,
            end_line: end,
            text: body,
        });
    }
    Ok(found)
}

/// Every word-boundary occurrence of `identifier` in files the FTS index
/// says contain it, classified by the token that follows. Definition lines
/// and comment lines are skipped.
fn sites_of(
    root: &Path,
    conn: &Connection,
    identifier: &str,
    definitions: &[Evidence],
    report: &mut ContextReport,
) -> Result<Vec<Evidence>> {
    let defined: BTreeSet<(&str, usize)> = definitions
        .iter()
        .map(|d| (d.path.as_str(), d.line))
        .collect();
    // A syntax span may start at an attribute line; its `fn name` line is
    // the same definition, not a second one.
    let inside_definition = |path: &str, line: usize| {
        definitions
            .iter()
            .any(|d| d.path == path && d.line <= line && line <= d.end_line)
    };
    let phrase = format!("body : \"{}\"", identifier.replace('"', "\"\""));
    let mut stmt = conn.prepare(
        "SELECT docs.path, files.hash FROM docs JOIN files ON files.id = docs.rowid
         WHERE docs MATCH ?1 ORDER BY docs.path LIMIT ?2",
    )?;
    let files = stmt
        .query_map(params![phrase, (MAX_SITE_FILES + 1) as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if files.len() > MAX_SITE_FILES {
        report.truncated = true;
        report.missing.push(format!(
            "more than {MAX_SITE_FILES} files mention `{identifier}`; only the first {MAX_SITE_FILES} by path were scanned"
        ));
    }
    let mut found = Vec::new();
    for (path, hash) in files.into_iter().take(MAX_SITE_FILES) {
        let Some(text) = live_source(root, &path, &hash) else {
            report.stale_results_discarded += 1;
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            let number = index + 1;
            if defined.contains(&(path.as_str(), number)) {
                continue;
            }
            let Some(kind) = classify_line(line, identifier) else {
                continue;
            };
            if kind == "definition" && inside_definition(&path, number) {
                continue;
            }
            // Prose and config files can name an identifier but never call it.
            let kind = if is_code_path(&path) { kind } else { "mention" };
            let spans = containing(conn, &path, &hash, number)?;
            let enclosing = spans
                .iter()
                .find(|(name, _)| name != identifier)
                .map(|(name, _)| name.clone());
            let in_test = is_test_path(&path)
                || spans
                    .iter()
                    .any(|(name, _)| name == "tests" || name.starts_with("test"));
            found.push(Evidence {
                kind,
                path: path.clone(),
                line: number,
                end_line: number,
                enclosing,
                in_test,
                text: excerpt_of(line),
            });
        }
    }
    Ok(found)
}

/// `call`, `reference` or `import` for the first code occurrence of
/// `identifier` on the line; `None` when absent or only in a comment.
fn classify_line(line: &str, identifier: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    let is_import = [
        "use ", "pub use ", "import ", "from ", "#include", "require(",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix));
    let comment = ["//", "#", "/*", "*"]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix));
    if comment && !is_import {
        return None;
    }
    let bytes = line.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut from = 0;
    while let Some(offset) = line[from..].find(identifier) {
        let start = from + offset;
        let end = start + identifier.len();
        from = end;
        if (start > 0 && word(bytes[start - 1])) || (end < bytes.len() && word(bytes[end])) {
            continue;
        }
        if line[..start].contains("//") {
            return None;
        }
        if is_import {
            return Some("import");
        }
        // `fn name`, `def name`, `class name`: a definition Tree-sitter did not
        // extract (macro bodies, unsupported languages).
        let before = line[..start].trim_end();
        if DEFINITION_KEYWORDS.iter().any(|keyword| {
            before.ends_with(keyword)
                && !before[..before.len() - keyword.len()]
                    .ends_with(|c: char| c.is_alphanumeric() || c == '_')
        }) {
            return Some("definition");
        }
        let rest = line[end..].trim_start();
        let macro_call = rest
            .strip_prefix('!')
            .is_some_and(|after| after.trim_start().starts_with(['(', '[', '{']));
        let call = rest.starts_with('(') || rest.starts_with("::<") || macro_call;
        return Some(if call { "call" } else { "reference" });
    }
    None
}

/// Every occurrence of `identifier` on the line is a `.identifier` member access.
fn only_method_syntax(line: &str, identifier: &str) -> bool {
    let all = line.matches(identifier).count();
    all > 0 && line.matches(&format!(".{identifier}")).count() == all
}

const CODE_EXTENSIONS: &[&str] = &[
    "c", "cc", "cjs", "cpp", "cs", "cxx", "go", "h", "hh", "hpp", "hxx", "java", "js", "jsx", "kt",
    "lua", "m", "mjs", "mm", "php", "ps1", "py", "pyi", "rb", "rs", "scala", "sh", "swift", "ts",
    "tsx",
];

fn is_code_path(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| CODE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

const DEFINITION_KEYWORDS: &[&str] = &[
    "fn",
    "def",
    "class",
    "struct",
    "enum",
    "trait",
    "type",
    "interface",
    "function",
    "macro_rules!",
];

fn is_test_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    lower.split('/').any(|part| {
        matches!(
            part,
            "test" | "tests" | "__tests__" | "spec" | "specs" | "testing"
        )
    }) || name.starts_with("test_")
        || [".test.", ".spec.", "_test.", "_tests."]
            .iter()
            .any(|marker| name.contains(marker))
}

/// Natural-language task: lexical retrieval over the content words only,
/// each hit shown inside its enclosing definition with two lines of context.
fn concept(
    root: &Path,
    database: &Path,
    words: &[String],
    conn: &Connection,
    report: &mut ContextReport,
    candidates: &mut Vec<Evidence>,
) -> Result<()> {
    let content: Vec<&str> = words
        .iter()
        .map(String::as_str)
        .filter(|w| !STOPWORDS.contains(w) && !CALLER_WORDS.contains(w) && !TEST_WORDS.contains(w))
        .collect();
    if content.is_empty() {
        report
            .missing
            .push("the task names no code identifiers or content words".to_string());
        return Ok(());
    }
    let found = search_in_db(root, database, &content.join(" "), 12, SyncStats::default())?;
    report.stale_results_discarded += found.stale_results_discarded;
    let mut hash_of = conn.prepare_cached("SELECT hash FROM files WHERE path = ?1")?;
    for hit in found.hits {
        let Ok(hash) = hash_of.query_row(params![hit.path], |row| row.get::<_, String>(0)) else {
            continue;
        };
        let Some(text) = live_source(root, &hit.path, &hash) else {
            report.stale_results_discarded += 1;
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        let first = hit.line.saturating_sub(2).max(1);
        let last = (hit.line + 2).min(lines.len()).max(first);
        let snippet = lines
            .get(first - 1..last)
            .unwrap_or_default()
            .iter()
            .map(|line| findex_core::redact_secrets(line).into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        let enclosing = containing(conn, &hit.path, &hash, hit.line)?
            .into_iter()
            .next()
            .map(|(name, _)| name);
        candidates.push(Evidence {
            kind: if hit.match_kind == "definition" {
                "definition"
            } else {
                "match"
            },
            in_test: is_test_path(&hit.path),
            path: hit.path,
            line: first,
            end_line: last,
            enclosing,
            text: snippet,
        });
    }
    if candidates.is_empty() {
        report
            .missing
            .push(format!("no indexed text matches: {}", content.join(" ")));
    }
    // Agents act on source; keep the lexical rank within source and prose.
    candidates.sort_by_key(|e| !is_code_path(&e.path));
    Ok(())
}

/// Greedy packing in priority order under the token budget: definitions,
/// then test sites (tests intent), calls and concept matches, references,
/// imports. Order inside a group is preserved (ranked, or by path and line).
fn pack(mut candidates: Vec<Evidence>, report: &mut ContextReport) {
    let tests_first = report.intent == "tests";
    candidates.sort_by_key(|e| match (e.kind, tests_first && e.in_test) {
        ("definition", _) => 0,
        (_, true) => 1,
        ("call" | "match", _) => 2,
        ("reference", _) => 3,
        ("import", _) => 4,
        _ => 5,
    });
    let mut used = render_header(report).len().div_ceil(4);
    for evidence in candidates {
        let needed = render_evidence(&evidence).len().div_ceil(4);
        if used + needed > report.budget_tokens {
            report.truncated = true;
            continue;
        }
        used += needed;
        report.evidence.push(evidence);
    }
    report.used_tokens = used;
}

fn render_header(report: &ContextReport) -> String {
    let named = if report.identifiers.is_empty() {
        String::new()
    } else {
        format!(" `{}`", report.identifiers.join("`, `"))
    };
    format!(
        "intent: {}{named}\nkinds: definition (Tree-sitter); call/reference/import are name matches, not type-resolved; files hash-verified when read\n",
        report.intent
    )
}

fn render_evidence(evidence: &Evidence) -> String {
    let span = if evidence.end_line > evidence.line {
        format!("{}-{}", evidence.line, evidence.end_line)
    } else {
        evidence.line.to_string()
    };
    let enclosing = evidence
        .enclosing
        .as_deref()
        .map(|e| format!(" [{e}]"))
        .unwrap_or_default();
    let test = if evidence.in_test && evidence.kind != "definition" {
        " test"
    } else {
        ""
    };
    let separator = if evidence.text.contains('\n') {
        "\n"
    } else {
        " "
    };
    format!(
        "{}{test} {}:{span}{enclosing}{separator}{}\n",
        evidence.kind, evidence.path, evidence.text
    )
}

impl ContextReport {
    /// Compact text for an agent: one line per site, multi-line only for bodies.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = render_header(self);
        for evidence in &self.evidence {
            out.push_str(&render_evidence(evidence));
        }
        for missing in &self.missing {
            out.push_str(&format!("missing: {missing}\n"));
        }
        let count = |kind: &str| self.evidence.iter().filter(|e| e.kind == kind).count();
        out.push_str(&format!(
            "shown: {} definitions, {} calls, {} references, {} imports, {} matches, {} prose mentions; ~{}/{} tokens; {}\n",
            count("definition"),
            count("call"),
            count("reference"),
            count("import"),
            count("match"),
            count("mention"),
            self.used_tokens,
            self.budget_tokens,
            if self.truncated {
                "TRUNCATED: raise budget or narrow the task"
            } else {
                "complete for the indexed files"
            },
        ));
        out
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{synchronize_into, SyncStats};
    use std::fs;
    use tempfile::{tempdir, TempDir};

    fn project(files: &[(&str, &str)]) -> (TempDir, TempDir, std::path::PathBuf) {
        let root = tempdir().expect("project");
        for (path, text) in files {
            let full = root.path().join(path);
            fs::create_dir_all(full.parent().expect("parent")).expect("dirs");
            fs::write(full, text).expect("write");
        }
        let state = tempdir().expect("state");
        let db = state.path().join("index.sqlite3");
        synchronize_into(root.path(), &db).expect("sync");
        (root, state, db)
    }

    fn ask(root: &TempDir, db: &std::path::Path, task: &str, budget: usize) -> ContextReport {
        context_in_db(root.path(), db, task, budget, SyncStats::default()).expect("context")
    }

    const LIB: &str = "pub fn poll_proceed(cx: u32) -> u32 {\n    cx + 1\n}\n\npub fn other() {}\n";
    const USER: &str = "use crate::lib::poll_proceed;\n\nfn poll_read() -> u32 {\n    // poll_proceed is cheap\n    let v = poll_proceed(3);\n    v\n}\n\nfn register() {\n    let f = poll_proceed;\n    let _ = f;\n}\n";
    const TEST: &str = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn proceeds() {\n        assert_eq!(crate::lib::poll_proceed(1), 2);\n    }\n}\n";

    #[test]
    fn callers_task_lists_call_sites_with_enclosing_functions() {
        let (root, _state, db) = project(&[
            ("src/lib.rs", LIB),
            ("src/user.rs", USER),
            ("src/check.rs", TEST),
        ]);
        let report = ask(&root, &db, "Which functions call `poll_proceed`?", 2000);
        assert_eq!(report.intent, "callers");
        assert_eq!(report.identifiers, vec!["poll_proceed".to_string()]);
        let definition = report
            .evidence
            .iter()
            .find(|e| e.kind == "definition")
            .expect("definition");
        assert_eq!(
            (definition.path.as_str(), definition.line),
            ("src/lib.rs", 1)
        );
        let calls: Vec<_> = report
            .evidence
            .iter()
            .filter(|e| e.kind == "call")
            .map(|e| (e.path.as_str(), e.line, e.enclosing.as_deref()))
            .collect();
        assert!(
            calls.contains(&("src/user.rs", 5, Some("poll_read"))),
            "{calls:?}"
        );
        assert!(
            calls.contains(&("src/check.rs", 5, Some("proceeds"))),
            "{calls:?}"
        );
        // The comment, the import and the definition itself are not calls.
        assert!(
            !calls.iter().any(|c| c.1 == 4 || (c.0 == "src/lib.rs")),
            "{calls:?}"
        );
        let kinds: Vec<_> = report
            .evidence
            .iter()
            .filter(|e| e.path == "src/user.rs")
            .map(|e| (e.line, e.kind))
            .collect();
        assert!(kinds.contains(&(1, "import")), "{kinds:?}");
        assert!(kinds.contains(&(10, "reference")), "{kinds:?}");
        let text = report.render();
        assert!(text.contains("src/user.rs:5"), "{text}");
        assert!(text.contains("not type-resolved"), "{text}");
    }

    #[test]
    fn definitions_hidden_from_tree_sitter_are_found_lexically() {
        // tokio wraps items in `cfg_*! { ... }` macros that Tree-sitter keeps opaque.
        let hidden =
            "cfg_coop! {\n    pub(crate) fn poll_proceed(cx: u32) -> u32 {\n        cx\n    }\n}\n";
        let (root, _state, db) = project(&[("src/coop.rs", hidden), ("src/user.rs", USER)]);
        let callers = ask(&root, &db, "callers of `poll_proceed`", 2000);
        let site = |line| {
            callers
                .evidence
                .iter()
                .find(|e| e.path == "src/coop.rs" && e.line == line)
                .map(|e| e.kind)
        };
        assert_eq!(site(2), Some("definition"), "{:?}", callers.evidence);
        assert!(
            !callers
                .missing
                .iter()
                .any(|m| m.contains("no syntax definition")),
            "{:?}",
            callers.missing
        );
        let defined = ask(&root, &db, "where is `poll_proceed` defined", 2000);
        assert_eq!(defined.evidence.len(), 1, "{:?}", defined.evidence);
        assert_eq!(
            (defined.evidence[0].path.as_str(), defined.evidence[0].line),
            ("src/coop.rs", 2)
        );
    }

    #[test]
    fn acronyms_are_words_and_prose_mentions_are_not_references() {
        assert_eq!(
            identifiers("Which SQLite query runs for HTTP requests?"),
            Vec::<String>::new()
        );
        assert_eq!(
            identifiers("who calls parseConfig or `Engine`"),
            vec!["Engine", "parseConfig"]
        );
        let (root, _state, db) = project(&[
            ("src/lib.rs", LIB),
            ("src/user.rs", USER),
            ("README.md", "Call `poll_proceed` before polling.\n"),
        ]);
        let report = ask(&root, &db, "callers of `poll_proceed`", 2000);
        let readme = report
            .evidence
            .iter()
            .find(|e| e.path == "README.md")
            .expect("mention kept");
        assert_eq!(readme.kind, "mention");
        assert_eq!(
            report.evidence.last().map(|e| e.kind),
            Some("mention"),
            "prose ranks last"
        );
    }

    #[test]
    fn concept_task_lists_source_before_prose() {
        let (root, _state, db) = project(&[
            ("HISTORY.md", "Authorization header removed on redirect (CVE).\n"),
            ("src/redirect.py", "def should_strip(old, new):\n    # Authorization header removed on redirect\n    return True\n"),
        ]);
        let report = ask(&root, &db, "authorization header removed on redirect", 2000);
        assert_eq!(
            report.evidence[0].path, "src/redirect.py",
            "{:?}",
            report.evidence
        );
        assert_eq!(report.evidence[1].path, "HISTORY.md");
    }

    #[test]
    fn rust_free_function_callers_exclude_method_syntax() {
        let other = "fn use_vec(v: Vec<u8>) -> usize {\n    v.iter().count() + walk_items()\n}\n";
        let lib = "pub fn iter() -> u32 {\n    1\n}\n\npub fn walk_items() -> usize {\n    iter() as usize\n}\n";
        let (root, _state, db) = project(&[("src/lib.rs", lib), ("src/other.rs", other)]);
        let report = ask(&root, &db, "callers of `iter`", 2000);
        let calls: Vec<_> = report
            .evidence
            .iter()
            .filter(|e| e.kind == "call")
            .map(|e| (e.path.as_str(), e.line))
            .collect();
        assert_eq!(calls, vec![("src/lib.rs", 6)], "{:?}", report.evidence);
        assert!(
            report.missing.iter().any(|m| m.contains("method-syntax")),
            "{:?}",
            report.missing
        );
    }

    #[test]
    fn shared_names_are_disclosed_as_ambiguous() {
        let a = "pub struct A;\nimpl A {\n    pub fn build(&self) {}\n}\n";
        let b = "pub struct B;\nimpl B {\n    pub fn build(&self) {}\n}\nfn go(a: crate::a::A) {\n    a.build();\n}\n";
        let (root, _state, db) = project(&[("src/a.rs", a), ("src/b.rs", b)]);
        let report = ask(&root, &db, "who calls `build`", 2000);
        assert!(
            report
                .missing
                .iter()
                .any(|m| m.contains("2 definitions") && m.contains("not type-resolved")),
            "{:?}",
            report.missing
        );
    }

    #[test]
    fn test_sites_are_marked_as_tests() {
        let (root, _state, db) = project(&[("src/lib.rs", LIB), ("src/check.rs", TEST)]);
        let report = ask(&root, &db, "which tests cover poll_proceed", 2000);
        assert_eq!(report.intent, "tests");
        assert!(report
            .evidence
            .iter()
            .any(|e| e.path == "src/check.rs" && e.line == 5 && e.in_test));
    }

    #[test]
    fn definition_task_returns_the_body_within_budget() {
        let (root, _state, db) = project(&[("src/lib.rs", LIB), ("src/user.rs", USER)]);
        let report = ask(&root, &db, "where is poll_proceed defined", 2000);
        assert_eq!(report.intent, "definition");
        let first = &report.evidence[0];
        assert_eq!(
            (first.kind, first.path.as_str(), first.line, first.end_line),
            ("definition", "src/lib.rs", 1, 3)
        );
        assert!(first.text.contains("cx + 1"), "{}", first.text);
    }

    #[test]
    fn unknown_identifier_is_reported_missing_not_guessed() {
        let (root, _state, db) = project(&[("src/lib.rs", LIB)]);
        let report = ask(&root, &db, "who calls `does_not_exist_anywhere`", 2000);
        assert!(report.evidence.is_empty(), "{:?}", report.evidence);
        assert!(
            report
                .missing
                .iter()
                .any(|m| m.contains("does_not_exist_anywhere")),
            "{:?}",
            report.missing
        );
    }

    #[test]
    fn budget_bounds_the_rendered_evidence() {
        let mut files = vec![("src/lib.rs".to_string(), LIB.to_string())];
        for i in 0..40 {
            files.push((
                format!("src/u{i}.rs"),
                format!("fn caller_{i}() {{\n    poll_proceed({i});\n}}\n"),
            ));
        }
        let borrowed: Vec<(&str, &str)> = files
            .iter()
            .map(|(p, t)| (p.as_str(), t.as_str()))
            .collect();
        let (root, _state, db) = project(&borrowed);
        let report = ask(&root, &db, "callers of poll_proceed", 300);
        assert!(report.truncated);
        assert!(report.used_tokens <= 300, "{}", report.used_tokens);
        assert!(report.render().len() / 4 <= 300 + 120, "header stays small");
        let all = ask(&root, &db, "callers of poll_proceed", 6000);
        assert_eq!(all.evidence.iter().filter(|e| e.kind == "call").count(), 40);
        assert!(!all.truncated);
    }

    #[test]
    fn edited_files_are_not_cited_from_stale_index_rows() {
        let (root, _state, db) = project(&[("src/lib.rs", LIB), ("src/user.rs", USER)]);
        fs::write(root.path().join("src/user.rs"), "fn nothing() {}\n").expect("edit");
        let report = ask(&root, &db, "callers of poll_proceed", 2000);
        assert!(!report.evidence.iter().any(|e| e.path == "src/user.rs"));
        assert!(report.stale_results_discarded >= 1);
    }

    #[test]
    fn concept_task_ignores_question_words() {
        let (root, _state, db) = project(&[
            ("src/redirect.py", "def rebuild_auth(prepared):\n    # strip the Authorization header on cross-host redirect\n    del prepared.headers['Authorization']\n"),
            ("src/where.py", "def where_is_it():\n    return 'where is the thing how does it'\n"),
        ]);
        let report = ask(
            &root,
            &db,
            "where is the authorization header stripped on redirect?",
            2000,
        );
        assert_eq!(report.intent, "concept");
        assert_eq!(report.evidence[0].path, "src/redirect.py");
        assert_eq!(
            report.evidence[0].enclosing.as_deref(),
            Some("rebuild_auth")
        );
    }
}
