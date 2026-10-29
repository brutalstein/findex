#![allow(clippy::expect_used)]

use std::process::Command;
use tempfile::TempDir;

fn findex_bin() -> std::path::PathBuf {
    std::env::var("FINDEX_CLI_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::Path::new(env!("CARGO_BIN_EXE_findex")).to_path_buf())
}

fn run_findex(args: &[&str], cwd: &std::path::Path) -> std::process::Output {
    // Isolate the index store so tests never touch the real user profile.
    let state = std::env::temp_dir().join(format!("findex-cli-tests-{}", std::process::id()));
    Command::new(findex_bin())
        .args(args)
        .current_dir(cwd)
        .env("LOCALAPPDATA", state)
        .output()
        .expect("failed to run findex")
}

#[test]
fn help_prints_usage() {
    let tmp = TempDir::new().expect("temp dir");
    let out = run_findex(&["help"], tmp.path());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("Findex v"));
    assert!(stdout.contains("findex index"));
    assert!(stdout.contains("findex search"));
}

#[test]
fn version_prints_version() {
    let tmp = TempDir::new().expect("temp dir");
    let out = run_findex(&["version"], tmp.path());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_command_errors() {
    let tmp = TempDir::new().expect("temp dir");
    let out = run_findex(&["notacommand"], tmp.path());
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown command"));
}

#[test]
fn index_creates_database() {
    let tmp = TempDir::new().expect("temp dir");
    // Write a small Rust file to index
    std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
    std::fs::write(
        tmp.path().join("src/lib.rs"),
        "pub fn hello() -> u32 {\n    42\n}\n",
    )
    .expect("write file");
    // Initialize git repo so project_root works
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .expect("git init");
    let out = run_findex(&["index"], tmp.path());
    assert!(out.status.success(), "index failed: {:?}", out);
}

#[test]
fn search_returns_hits() {
    let tmp = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
    std::fs::write(
        tmp.path().join("src/main.rs"),
        "pub fn poll_proceed(cx: u32) -> u32 {\n    cx + 1\n}\n",
    )
    .expect("write file");
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .expect("git init");
    run_findex(&["index"], tmp.path());
    let out = run_findex(&["search", "poll_proceed"], tmp.path());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("poll_proceed"));
}

#[test]
fn find_symbol_returns_definition() {
    let tmp = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
    std::fs::write(
        tmp.path().join("src/main.rs"),
        "pub struct Analysis {\n    pub field: u32,\n}\n",
    )
    .expect("write file");
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .expect("git init");
    run_findex(&["index"], tmp.path());
    let out = run_findex(&["find-symbol", "Analysis"], tmp.path());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("Analysis"));
}

#[test]
fn context_returns_evidence() {
    let tmp = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
    std::fs::write(
        tmp.path().join("src/main.rs"),
        "pub fn poll_proceed(cx: u32) -> u32 {\n    cx + 1\n}\n",
    )
    .expect("write file");
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .expect("git init");
    run_findex(&["index"], tmp.path());
    let out = run_findex(&["context", "who calls poll_proceed"], tmp.path());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(stdout.contains("intent:"));
}

#[test]
fn context_json_output() {
    let tmp = TempDir::new().expect("temp dir");
    std::fs::create_dir_all(tmp.path().join("src")).expect("create src");
    std::fs::write(
        tmp.path().join("src/main.rs"),
        "pub fn poll_proceed(cx: u32) -> u32 {\n    cx + 1\n}\n",
    )
    .expect("write file");
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .expect("git init");
    run_findex(&["index"], tmp.path());
    let out = run_findex(&["context", "--json", "poll_proceed"], tmp.path());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(parsed.get("intent").is_some());
    assert!(parsed.get("evidence").is_some());
}

#[test]
fn status_prints_project_info() {
    let tmp = TempDir::new().expect("temp dir");
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .output()
        .expect("git init");
    let out = run_findex(&["status"], tmp.path());
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Project:"));
}
