#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(not(windows))]
compile_error!("Findex is a native Windows-only application. Build on the Windows MSVC target.");

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

fn main() {
    if let Err(err) = run() {
        eprintln!("findex: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut args = env::args_os().skip(1);
    let command = args.next().unwrap_or_else(|| OsString::from("help"));
    let cwd = env::current_dir()?;
    let project = findex_core::project_root(&cwd)?;
    match command.to_string_lossy().as_ref() {
        "help" | "--help" | "-h" => print_help(),
        "--version" | "version" => println!("findex {}", env!("CARGO_PKG_VERSION")),
        "claude" => launch_claude(args.collect())?,
        "index" => {
            let stats = findex_index::synchronize(&project)?;
            println!(
                "Indexed {} changed, {} unchanged, {} removed, {} skipped (examined {})",
                stats.indexed, stats.unchanged, stats.removed, stats.skipped, stats.examined
            );
        }
        "refresh" => {
            let paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
            if paths.is_empty() {
                return Err("usage: findex refresh <known-relative-path> ...".into());
            }
            let report = findex_index::refresh_known_paths(&project, &paths)?;
            println!(
                "Refreshed {} changed, {} unchanged, {} removed; {} unknown (full index required)",
                report.sync.indexed,
                report.sync.unchanged,
                report.sync.removed,
                report.unknown_paths
            );
        }
        "symbols" => {
            let relative = args
                .next()
                .ok_or("usage: findex symbols <project-relative-file>")?;
            if args.next().is_some() {
                return Err("usage: findex symbols <project-relative-file>".into());
            }
            let relative = PathBuf::from(relative);
            match findex_index::inspect_indexed_file(&project, &relative)? {
                Some(result) => {
                    println!(
                        "Language: {}; syntax errors: {}; truncated: {}",
                        result.language, result.has_syntax_errors, result.truncated
                    );
                    for symbol in result.symbols {
                        println!(
                            "{}:{}-{} [{}] {}",
                            relative.display(),
                            symbol.start_line,
                            symbol.end_line,
                            symbol.kind,
                            symbol.name
                        );
                    }
                }
                None => println!("Syntax extraction unavailable for this file type"),
            }
        }
        "find-symbol" => {
            let name = args
                .next()
                .ok_or("usage: findex find-symbol <exact-name>")?
                .to_string_lossy()
                .into_owned();
            if name.is_empty() || args.next().is_some() {
                return Err("usage: findex find-symbol <exact-name>".into());
            }
            let result = findex_index::search_symbols(&project, &name, 20)?;
            for symbol in &result.hits {
                println!(
                    "{}:{}-{} [{}] {} ({})",
                    symbol.path,
                    symbol.start_line,
                    symbol.end_line,
                    symbol.kind,
                    symbol.name,
                    symbol.language
                );
            }
            println!(
                "{} definitions; {} stale discarded",
                result.hits.len(),
                result.stale_results_discarded
            );
        }
        "search" => {
            let query = args
                .map(|a| a.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ");
            if query.trim().is_empty() {
                return Err("usage: findex search <search terms>".into());
            }
            let result = findex_index::search(&project, &query, 10)?;
            for hit in &result.hits {
                println!("{}:{}  {}", hit.path, hit.line, hit.excerpt);
            }
            println!(
                "{} hits; {} changed; {} stale discarded",
                result.hits.len(),
                result.sync.indexed,
                result.stale_results_discarded
            );
        }
        "context" => {
            let mut budget = 2000;
            let mut as_json = false;
            let mut words = Vec::new();
            while let Some(arg) = args.next() {
                match arg.to_string_lossy().as_ref() {
                    "--budget" => {
                        budget = args
                            .next()
                            .ok_or("--budget needs a number")?
                            .to_string_lossy()
                            .parse()?;
                    }
                    "--json" => as_json = true,
                    word => words.push(word.to_string()),
                }
            }
            let task = words.join(" ");
            if task.trim().is_empty() {
                return Err("usage: findex context [--budget N] [--json] <task>".into());
            }
            let report = findex_index::context(&project, &task, budget)?;
            if as_json {
                println!("{}", serde_json::to_string(&report)?);
            } else {
                print!("{}", report.render());
            }
        }
        "graph-import-scip" => {
            let file = args
                .next()
                .ok_or("usage: findex graph-import-scip <index.scip>")?;
            if args.next().is_some() {
                return Err("usage: findex graph-import-scip <index.scip>".into());
            }
            let imported = findex_relations::import_scip(&project, Path::new(&file))?;
            println!(
                "Imported {} files, {} definitions, {} references ({})",
                imported.documents, imported.definitions, imported.references, imported.source
            );
        }
        "graph-import-lsp" => {
            let file = args
                .next()
                .ok_or("usage: findex graph-import-lsp <captured.json>")?;
            if args.next().is_some() {
                return Err("usage: findex graph-import-lsp <captured.json>".into());
            }
            let metadata = std::fs::metadata(&file)?;
            if metadata.len() > 16 * 1024 * 1024 {
                return Err("LSP capture exceeds 16 MiB".into());
            }
            let json = std::fs::read_to_string(file)?;
            let imported = findex_relations::import_lsp_snapshot(&project, &json)?;
            println!(
                "Imported {} files, {} definitions, {} references ({})",
                imported.documents, imported.definitions, imported.references, imported.source
            );
        }
        "impact" => {
            let file = args
                .next()
                .ok_or("usage: findex impact <relative-file> [depth:1-3]")?;
            let depth = match args.next() {
                Some(s) => s.to_string_lossy().parse::<usize>()?,
                None => 1,
            };
            if depth == 0 || depth > 3 || args.next().is_some() {
                return Err("usage: findex impact <relative-file> [depth:1-3]".into());
            }
            let found = findex_relations::impact(&project, Path::new(&file), depth, 32)?;
            println!(
                "{} reference-based impacted files from {}",
                found.hits.len(),
                found.source
            );
            for hit in &found.hits {
                println!(
                    "{}:{} <= {}:{} [depth {}] {}",
                    hit.path,
                    hit.referenced_at_line,
                    hit.definition_path,
                    hit.definition_line,
                    hit.depth,
                    hit.interpretation
                );
            }
            if found.truncated {
                eprintln!("Impact results were truncated");
            }
            if found.stale_evidence_discarded > 0 {
                eprintln!(
                    "Discarded {} stale reference files",
                    found.stale_evidence_discarded
                );
            }
        }
        "doctor" => doctor(&project),
        "status" | "stats" => {
            println!("Project: {}", project.display());
            println!("Index: {}", findex_core::storage_dir(&project)?.display());
            println!("Token savings: unavailable (requires a measured baseline, not implemented in v0.1)");
        }
        "mcp" => findex_mcp::serve(&project)?,
        _ => {
            return Err(format!(
                "unknown command: {} (use findex help)",
                command.to_string_lossy()
            )
            .into())
        }
    }
    Ok(())
}

fn launch_claude(user_args: Vec<OsString>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if env::var_os("FINDEX_CLAUDE_CHILD").is_some() {
        return Err("nested Findex/Claude launch detected; refusing recursion".into());
    }
    // Invoke Claude itself, not an unofficial client, proxy or API-auth emulator.
    // Native Claude Code installation with claude.exe on PATH is required in v0.1.
    let directory = env::current_dir()?;
    let project = findex_core::project_root(&directory)?;
    let native_claude = find_native_claude(&project).ok_or(
        "official native claude.exe not found on PATH outside the project; run findex doctor",
    )?;
    let mut command = Command::new(native_claude);
    if let Some(plugin_dir) = bundled_plugin_dir() {
        command.arg("--plugin-dir").arg(plugin_dir);
    }
    // Ensure the bundled plugin's MCP command finds this same Findex binary,
    // even when Findex was invoked using an absolute path rather than via PATH.
    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            let mut paths = vec![parent.to_path_buf()];
            if let Some(path_value) = env::var_os("PATH") {
                paths.extend(env::split_paths(&path_value));
            }
            if let Ok(path_value) = env::join_paths(paths) {
                command.env("PATH", path_value);
            }
        }
    }
    command.args(user_args).env("FINDEX_CLAUDE_CHILD", "1");
    let status = command.status().map_err(|err| format!("cannot launch native claude.exe: {err}; install official Claude Code and ensure claude.exe is on PATH"))?;
    process::exit(status.code().unwrap_or(1));
}

fn bundled_plugin_dir() -> Option<std::path::PathBuf> {
    let exe = env::current_exe().ok()?;
    let directory = exe.parent()?.join("plugin");
    if directory
        .join(".claude-plugin")
        .join("plugin.json")
        .is_file()
    {
        Some(directory)
    } else {
        None
    }
}

fn doctor(project: &Path) {
    println!("Findex native Windows diagnostics");
    println!("Project root: {}", project.display());
    println!(
        "Local index storage: {}",
        findex_core::storage_dir(project)
            .map(|v| v.display().to_string())
            .unwrap_or_else(|_| "unavailable".into())
    );
    println!(
        "Plugin beside executable: {}",
        if bundled_plugin_dir().is_some() {
            "present"
        } else {
            "missing (Claude runs without Findex tools)"
        }
    );
    let available = find_native_claude(project).is_some();
    println!(
        "Native Claude CLI (claude.exe): {}",
        if available { "found" } else { "not found" }
    );
    if env::var_os("ANTHROPIC_API_KEY").is_some() {
        eprintln!("WARNING: ANTHROPIC_API_KEY is set and may override subscription authentication. Value intentionally hidden.");
    }
    println!(
        "Index engine: SQLite/FTS5, Tree-sitter symbols, optional imported SCIP/LSP references"
    );
    println!(
        "Limitations: no live LSP service; context routing is rule-based, not semantic; no measured token savings"
    );
}

fn print_help() {
    println!(
        "Findex v{} - native, local-first code retrieval for Claude Code",
        env!("CARGO_PKG_VERSION")
    );
    println!("  findex claude [CLAUDE_ARGS...]  Start official Claude Code in this terminal");
    println!("  findex index                    Reconcile project lexical index");
    println!("  findex search <terms...>        Search matching code and line evidence");
    println!("  findex symbols <relative-file>  Parse definitions with Tree-sitter");
    println!("  findex find-symbol <exact-name>  Search persistent project definitions");
    println!("  findex graph-import-scip <file> Import bounded SCIP protobuf references");
    println!("  findex graph-import-lsp <file>  Import normalized captured LSP references");
    println!("  findex impact <relative-file> [depth:1-3]  Find reference-backed file impact");
    println!("  findex refresh <paths...>       Refresh known relative files without a full scan");
    println!("  findex doctor                   Check local integration and system");
    println!("  findex status                   Show current project and index location");
    println!("  findex mcp                      Start MCP stdio server (for Claude plugin)");
    println!("  findex version                  Print build version");
}

/// Search explicit, absolute PATH directories only. Windows' default executable search
/// includes the current working directory, which may be an untrusted source repository.
/// Never launch a project-local `claude.exe` just because it shadows an official binary.
fn find_native_claude(project: &Path) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    let this_exe = env::current_exe().ok().and_then(|p| p.canonicalize().ok());
    let canonical_project = project.canonicalize().ok()?;
    for directory in env::split_paths(&path) {
        if !directory.is_absolute() {
            continue;
        }
        let candidate = directory.join("claude.exe");
        let Ok(resolved) = candidate.canonicalize() else {
            continue;
        };
        if !resolved.is_file() || resolved.starts_with(&canonical_project) {
            continue;
        }
        if this_exe.as_ref() == Some(&resolved) {
            continue;
        }
        return Some(resolved);
    }
    None
}
