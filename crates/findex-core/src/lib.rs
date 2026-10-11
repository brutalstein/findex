//! Shared paths, project identity and policy for the Findex engine.
//! No network I/O and no credential access.

use std::env;
use std::io;
use std::path::{Component, Path, PathBuf};

pub const MAX_SOURCE_BYTES: u64 = 512 * 1024;
pub const MAX_FILES_PER_SYNC: usize = 100_000;

/// Resolve a working directory to the nearest Git workspace or to itself.
/// A `.git` *file* is accepted to support linked worktrees.
pub fn project_root(working_directory: &Path) -> io::Result<PathBuf> {
    let directory = working_directory.canonicalize()?;
    if !directory.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "project path is not a directory",
        ));
    }
    for ancestor in directory.ancestors() {
        if ancestor.join(".git").exists() {
            return Ok(ancestor.to_path_buf());
        }
    }
    Ok(directory)
}

/// Construct a stable, workspace-relative SQLite key using real OS path
/// components. Windows accepts repeated separators on disk, but the database
/// cannot treat `src\\\\main.rs` and `src\\main.rs` as different files.
pub fn normalized_relative_key(relative: &Path) -> io::Result<String> {
    let mut key = String::new();
    for part in relative.components() {
        let Component::Normal(name) = part else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path must have only normal relative components",
            ));
        };
        if !key.is_empty() {
            key.push('/');
        }
        key.push_str(&name.to_string_lossy());
    }
    if key.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty project-relative path",
        ));
    }
    Ok(key)
}

/// Per-user local index directory; explicitly avoid putting Findex data in source trees.
pub fn storage_dir(project: &Path) -> io::Result<PathBuf> {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))?;
    let digest = blake3::hash(project.to_string_lossy().as_bytes()).to_hex();
    Ok(base.join("Findex").join("indexes").join(digest.as_str()))
}

/// Static scan-pruned components. Shared by the directory scanner and native
/// watcher to avoid rescanning the whole workspace for irrelevant build churn.
pub fn is_excluded_scan_entry(value: &std::ffi::OsStr) -> bool {
    let name = value.to_string_lossy().to_ascii_lowercase();
    matches!(
        name.as_str(),
        ".git"
            | ".findex"
            | "node_modules"
            | "target"
            | "vendor"
            | "dist"
            | "build"
            | ".venv"
            | "__pycache__"
    )
}

/// Skip well-known credentials and key material even when tracked in Git.
/// This does not guarantee secret-free snippets: secret scanning is a later milestone.
pub fn is_sensitive_filename(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || name == "id_rsa"
        || name == "id_ed25519"
        || name == "credentials.json"
        || name == "secrets.json"
        || name.ends_with(".pem")
        || name.ends_with(".p12")
        || name.ends_with(".pfx")
        || name.ends_with(".key")
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn repeated_windows_separators_share_a_single_index_key() {
        assert_eq!(
            normalized_relative_key(Path::new(r"src\\\\nested\\file.rs")).expect("normalize"),
            "src/nested/file.rs"
        );
        assert!(normalized_relative_key(Path::new(r"..\\outside.rs")).is_err());
        assert!(normalized_relative_key(Path::new(r"C:\\external.rs")).is_err());
    }

    #[test]
    fn sensitive_filename_rules() {
        for value in [".env", ".env.production", "ID_RSA", "tls.pem", "prod.PFX"] {
            assert!(is_sensitive_filename(Path::new(value)), "{value}");
        }
        assert!(!is_sensitive_filename(Path::new("src/main.rs")));
    }

    #[test]
    fn project_root_accepts_worktree_gitfile() {
        let tmp = std::env::temp_dir().join(format!("findex-core-root-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("sub")).expect("create fixture");
        std::fs::write(tmp.join(".git"), "gitdir: unused").expect("write fixture");
        assert_eq!(
            project_root(&tmp.join("sub")).expect("root"),
            tmp.canonicalize().expect("canonical")
        );
        let _ = std::fs::remove_dir_all(tmp);
    }
}
