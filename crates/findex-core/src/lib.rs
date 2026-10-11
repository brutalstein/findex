//! Shared paths, project identity and policy for the Findex engine.
//! No network I/O and no credential access.

use std::env;
use std::io;
use std::path::{Path, PathBuf};

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

/// Per-user local index directory; explicitly avoid putting Findex data in source trees.
pub fn storage_dir(project: &Path) -> io::Result<PathBuf> {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))?;
    let digest = blake3::hash(project.to_string_lossy().as_bytes()).to_hex();
    Ok(base.join("Findex").join("indexes").join(digest.as_str()))
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
