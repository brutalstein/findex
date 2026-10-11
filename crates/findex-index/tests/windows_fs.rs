//! Real Windows filesystem behaviour: no mocks, no injected events.
#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use findex_index::{inspect_indexed_file_in_db, search_in_db, synchronize_into, SyncStats};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn search(project: &Path, db: &Path, query: &str) -> findex_index::SearchReport {
    search_in_db(project, db, query, 8, SyncStats::default()).expect("search")
}

#[test]
fn unicode_file_names_are_indexed_with_normalized_keys() {
    let project = tempdir().expect("project");
    let state = tempdir().expect("state");
    let db = state.path().join("index.sqlite3");
    let dir = project.path().join("модуль");
    fs::create_dir(&dir).expect("dir");
    fs::write(dir.join("ünïcode_файл.rs"), "fn unicode_marker_fn() {}\n").expect("file");
    synchronize_into(project.path(), &db).expect("sync");
    let found = search(project.path(), &db, "unicode_marker_fn");
    assert_eq!(found.hits.len(), 1);
    assert_eq!(found.hits[0].path, "модуль/ünïcode_файл.rs");
}

#[test]
fn paths_longer_than_max_path_are_indexed() {
    let project = tempdir().expect("project");
    let state = tempdir().expect("state");
    let db = state.path().join("index.sqlite3");
    let mut dir = project.path().to_path_buf();
    while dir.as_os_str().len() < 300 {
        dir.push("a_rather_long_directory_name_segment");
    }
    fs::create_dir_all(&dir).expect("long dir");
    fs::write(dir.join("deep.rs"), "fn long_path_marker() {}\n").expect("long file");
    assert!(dir.join("deep.rs").as_os_str().len() > 260);
    synchronize_into(project.path(), &db).expect("sync");
    assert_eq!(
        search(project.path(), &db, "long_path_marker").hits.len(),
        1
    );
}

#[test]
fn differently_cased_project_paths_share_one_index_location() {
    let project = tempdir().expect("project");
    let canonical = findex_core::project_root(project.path()).expect("root");
    let upper = PathBuf::from(project.path().to_string_lossy().to_uppercase());
    let other = findex_core::project_root(&upper).expect("root via other case");
    assert_eq!(
        findex_core::storage_dir(&canonical).expect("storage"),
        findex_core::storage_dir(&other).expect("storage")
    );
}

#[test]
fn ntfs_junction_to_outside_directory_is_not_followed() {
    let project = tempdir().expect("project");
    let outside = tempdir().expect("outside");
    let state = tempdir().expect("state");
    let db = state.path().join("index.sqlite3");
    fs::write(
        outside.path().join("leak.rs"),
        "fn junction_escape_marker() {}\n",
    )
    .expect("outside");
    fs::write(project.path().join("inside.rs"), "fn inside_marker() {}\n").expect("inside");
    let link = project.path().join("linked");
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .output()
        .expect("mklink");
    assert!(
        status.status.success(),
        "junction creation failed: {status:?}"
    );
    synchronize_into(project.path(), &db).expect("sync");
    assert!(search(project.path(), &db, "junction_escape_marker")
        .hits
        .is_empty());
    assert!(
        inspect_indexed_file_in_db(project.path(), &db, Path::new("linked/leak.rs"))
            .expect("inspect")
            .is_none()
    );
    assert_eq!(search(project.path(), &db, "inside_marker").hits.len(), 1);
}

#[test]
fn invalid_utf8_oversized_and_binary_files_are_skipped_not_indexed() {
    let project = tempdir().expect("project");
    let state = tempdir().expect("state");
    let db = state.path().join("index.sqlite3");
    fs::write(
        project.path().join("latin1.rs"),
        b"fn invalid_utf8_marker() {} // \xff\xfe",
    )
    .expect("latin1");
    fs::write(
        project.path().join("huge.rs"),
        format!("fn huge_marker() {{}}\n{}", "x".repeat(600 * 1024)),
    )
    .expect("huge");
    fs::write(project.path().join("bin.rs"), b"fn binary_marker() {}\0").expect("binary");
    let stats = synchronize_into(project.path(), &db).expect("sync");
    assert_eq!(stats.indexed, 0);
    assert_eq!(stats.skipped, 3);
    for marker in ["invalid_utf8_marker", "huge_marker", "binary_marker"] {
        assert!(
            search(project.path(), &db, marker).hits.is_empty(),
            "{marker}"
        );
    }
}

#[test]
fn exclusively_locked_file_is_dropped_instead_of_served_stale() {
    use std::os::windows::fs::OpenOptionsExt;
    let project = tempdir().expect("project");
    let state = tempdir().expect("state");
    let db = state.path().join("index.sqlite3");
    let path = project.path().join("locked.rs");
    fs::write(&path, "fn locked_marker() {}\n").expect("file");
    synchronize_into(project.path(), &db).expect("sync");
    assert_eq!(search(project.path(), &db, "locked_marker").hits.len(), 1);
    // share_mode(0): no other handle may read the file while this one is open.
    let guard = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&path)
        .expect("exclusive open");
    let stats = synchronize_into(project.path(), &db).expect("sync while locked");
    assert_eq!(stats.removed, 1);
    assert!(search(project.path(), &db, "locked_marker").hits.is_empty());
    drop(guard);
    synchronize_into(project.path(), &db).expect("sync after unlock");
    assert_eq!(search(project.path(), &db, "locked_marker").hits.len(), 1);
}

#[test]
fn alternate_windows_path_forms_cannot_reach_unadmitted_files() {
    let project = tempdir().expect("project");
    let state = tempdir().expect("state");
    let db = state.path().join("index.sqlite3");
    fs::write(project.path().join("lib.rs"), "fn admitted() {}\n").expect("file");
    fs::write(
        project.path().join("id_rsa"),
        "fn private_key_marker() {}\n",
    )
    .expect("key");
    synchronize_into(project.path(), &db).expect("sync");
    for (raw, must_error) in [
        (r"..\outside.rs", true),
        (r"C:\Windows\win.ini", true),
        (r"\\?\C:\Windows\win.ini", true),
        (r"\\server\share\x.rs", true),
        ("lib.rs:hidden_stream", false),
        ("id_rsa.", false),
        ("LIB.RS", false),
        ("CON", false),
    ] {
        let result = inspect_indexed_file_in_db(project.path(), &db, Path::new(raw));
        if must_error {
            assert!(result.is_err(), "{raw} must be rejected");
        } else {
            assert!(
                result.as_ref().map_or(true, Option::is_none),
                "{raw} must not return evidence: {result:?}"
            );
        }
    }
}

#[test]
fn concurrent_reconciliations_of_one_database_both_succeed() {
    let project = tempdir().expect("project");
    let state = tempdir().expect("state");
    let db = state.path().join("index.sqlite3");
    for number in 0..300 {
        fs::write(
            project.path().join(format!("f{number}.rs")),
            format!("fn concurrent_marker_{number}() {{}}\n"),
        )
        .expect("file");
    }
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let project = project.path().to_path_buf();
            let db = db.clone();
            std::thread::spawn(move || {
                synchronize_into(&project, &db).map_err(|e| format!("{e:?}"))
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("join").expect("sync under contention");
    }
    assert_eq!(
        search(project.path(), &db, "concurrent_marker_299")
            .hits
            .len(),
        1
    );
}

#[test]
fn hidden_and_build_directories_are_excluded() {
    let project = tempdir().expect("project");
    let state = tempdir().expect("state");
    let db = state.path().join("index.sqlite3");
    for dir in [".hidden", "node_modules", "target", "build"] {
        fs::create_dir(project.path().join(dir)).expect("dir");
        fs::write(
            project.path().join(dir).join("x.rs"),
            "fn excluded_dir_marker() {}\n",
        )
        .expect("file");
    }
    synchronize_into(project.path(), &db).expect("sync");
    assert!(search(project.path(), &db, "excluded_dir_marker")
        .hits
        .is_empty());
}
