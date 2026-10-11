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
            println!("Indexed {} changed, {} unchanged, {} removed, {} skipped (examined {})", stats.indexed, stats.unchanged, stats.removed, stats.skipped, stats.examined);
        }
        "search" => {
            let query = args.map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ");
            if query.trim().is_empty() { return Err("usage: findex search <search terms>".into()); }
            let result = findex_index::search(&project, &query, 10)?;
            for hit in &result.hits {
                println!("{}:{}  {}", hit.path, hit.line, hit.excerpt);
            }
            println!("{} hits; {} changed; {} stale discarded", result.hits.len(), result.sync.indexed, result.stale_results_discarded);
        }
        "doctor" => doctor(&project),
        "status" | "stats" => {
            println!("Project: {}", project.display());
            println!("Index: {}", findex_core::storage_dir(&project)?.display());
            println!("Token savings: unavailable (requires a measured baseline, not implemented in v0.1)");
        }
        "mcp" => findex_mcp::serve(&project)?,
        _ => return Err(format!("unknown command: {} (use findex help)", command.to_string_lossy()).into()),
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
    let native_claude = find_native_claude(&project).ok_or("official native claude.exe not found on PATH outside the project; run findex doctor")?;
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
    if directory.join(".claude-plugin").join("plugin.json").is_file() {
        Some(directory)
    } else {
        None
    }
}

fn doctor(project: &Path) {
    println!("Findex native Windows diagnostics");
    println!("Project root: {}", project.display());
    println!("Local index storage: {}", findex_core::storage_dir(project).map(|v|v.display().to_string()).unwrap_or_else(|_| "unavailable".into()));
    println!("Plugin beside executable: {}", if bundled_plugin_dir().is_some() { "present" } else { "missing (Claude runs without Findex tools)" });
    let available = find_native_claude(project).is_some();
    println!("Native Claude CLI (claude.exe): {}", if available { "found" } else { "not found" });
    if env::var_os("ANTHROPIC_API_KEY").is_some() {
        eprintln!("WARNING: ANTHROPIC_API_KEY is set and may override subscription authentication. Value intentionally hidden.");
    }
    println!("Index engine: SQLite/FTS5 lexical; no semantic graph, token accounting, or LSP in v0.1");
}

fn print_help() {
    println!("Findex v{} - native, local-first code retrieval for Claude Code", env!("CARGO_PKG_VERSION"));
    println!("  findex claude [CLAUDE_ARGS...]  Start official Claude Code in this terminal");
    println!("  findex index                    Reconcile project lexical index");
    println!("  findex search <terms...>        Search matching code and line evidence");
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
        let Ok(resolved) = candidate.canonicalize() else { continue; };
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
