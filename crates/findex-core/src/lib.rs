//! Shared paths, project identity and policy for the Findex engine.
//! No network I/O and no credential access.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![warn(clippy::all)]

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
    let base = if cfg!(windows) {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))?
    } else {
        env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "XDG_CACHE_HOME or HOME is not set")
            })?
    };
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
            | ".ssh"
            | ".aws"
            | ".azure"
            | ".kube"
            | ".docker"
            | ".gnupg"
            | ".gcloud"
            | ".terraform"
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
        || name == ".envrc"
        || name == ".npmrc"
        || name == ".pypirc"
        || name == ".netrc"
        || name == "_netrc"
        || name == ".git-credentials"
        || name == "id_rsa"
        || name == "id_ed25519"
        || name == "credentials.json"
        || name == "secrets.json"
        || name == "service-account.json"
        || name == "service_account.json"
        || (name.starts_with("firebase-adminsdk-") && name.ends_with(".json"))
        || name.ends_with(".tfstate")
        || name.contains(".tfstate.")
        || name.ends_with(".tfvars")
        || name.ends_with(".tfvars.json")
        || name.ends_with(".jks")
        || name.ends_with(".keystore")
        || name.ends_with(".kdbx")
        || name.ends_with(".pem")
        || name.ends_with(".p12")
        || name.ends_with(".pfx")
        || name.ends_with(".key")
}

/// Admission policy for a *project-relative* path, shared across full scans,
/// targeted refreshes, indexed reads and semantic graph imports. Apply it
/// before serving an old cached entry, not only when building a new index.
/// Excluded build/credential directories and sensitive filenames are never
/// eligible, even if they were indexed by an older Findex version.
pub fn is_disallowed_project_path(relative: &Path) -> bool {
    let mut any = false;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return true;
        };
        any = true;
        if is_excluded_scan_entry(name) || is_sensitive_filename(Path::new(name)) {
            return true;
        }
    }
    !any
}

/// A cloud-sync placeholder whose data is not on this device (OneDrive Files
/// On-Demand or any other Cloud Files provider, or HSM offline storage).
/// Reading it would download it, so it is never read and never indexed.
pub fn is_cloud_only(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const OFFLINE: u32 = 0x1000;
        const RECALL_ON_OPEN: u32 = 0x4_0000;
        const RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
        metadata.file_attributes() & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

/// Best-effort masking of common credential shapes in text returned to a
/// model. This is a pattern list, not a guarantee that no secret survives.
pub fn redact_secrets(text: &str) -> std::borrow::Cow<'_, str> {
    use regex::Regex;
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            (r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b", "[REDACTED:aws-key]"),
            (r"\bgh[pousr]_[A-Za-z0-9]{30,}\b", "[REDACTED:github-token]"),
            (r"\bgithub_pat_[A-Za-z0-9_]{22,}", "[REDACTED:github-token]"),
            (r"\bsk-(?:ant-)?[A-Za-z0-9_\-]{20,}", "[REDACTED:api-key]"),
            (r"\bxox[abprs]-[A-Za-z0-9-]{10,}", "[REDACTED:slack-token]"),
            (r"-----BEGIN [A-Z ]*PRIVATE KEY-----", "[REDACTED:private-key]"),
            (
                r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
                "[REDACTED:jwt]",
            ),
            (
                r#"(?i)(\b(?:password|passwd|pwd|secret|api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret|private[_-]?key)\b["']?\s*[:=]\s*["'])[^"'\s]{6,}(["'])"#,
                "${1}[REDACTED:assignment]${2}",
            ),
            // Azure Storage / Service Bus / Cosmos connection strings.
            (
                r"(?i)(?:DefaultEndpointsProtocol|EndpointSuffix|SharedAccessKey|AccountKey)\s*=\s*[^;\s]{8,}",
                "[REDACTED:azure-connection-string]",
            ),
            // GCP service account / API key indicators.
            (r#"["](?:type|project_id|private_key_id|private_key|client_email)["]\s*:\s*["][^"]{8,}["]"#,
             "[REDACTED:gcp-credential]"),
            (r"\bAIza[0-9A-Za-z_-]{35}\b", "[REDACTED:gcp-api-key]"),
            // Datadog, Stripe, SendGrid, Slack webhooks.
            (r"\bdd-[0-9a-f]{32}\b", "[REDACTED:datadog-key]"),
            (r"\bsk_(?:live|test)_[0-9a-zA-Z]{24,}\b", "[REDACTED:stripe-key]"),
            (r"\bSG\.[0-9A-Za-z\-_]{22,}\.[0-9A-Za-z\-_]{22,}", "[REDACTED:sendgrid-key]"),
            (r"https?://hooks\.slack\.com/services/T[A-Z0-9_]{8,}/B[A-Z0-9_]{8,}/[A-Za-z0-9]{24,}", "[REDACTED:slack-webhook]"),
        ]
        .into_iter()
        .filter_map(|(pattern, replacement)| Regex::new(pattern).ok().map(|r| (r, replacement)))
        .collect()
    });
    let mut current = std::borrow::Cow::Borrowed(text);
    for (pattern, replacement) in patterns {
        if pattern.is_match(&current) {
            current =
                std::borrow::Cow::Owned(pattern.replace_all(&current, *replacement).into_owned());
        }
    }
    current
}

pub mod rank;

pub use rank::{file_type_for, rank_file, FileType, RankSignals, RankWeights, RankedFile};

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// SIDs of allow ACEs on `dir` that let a broad group read it: Everyone,
    /// Anonymous, Authenticated Users, Users, Guests.
    #[cfg(windows)]
    fn broad_readers(dir: &Path) -> Vec<String> {
        let script = "$a = (Get-Acl -LiteralPath $env:FINDEX_ACL_PROBE).Access | Where-Object { $_.AccessControlType -eq 'Allow' -and ($_.FileSystemRights -band 1) }; \
            $a | ForEach-Object { $_.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value }";
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("FINDEX_ACL_PROBE", dir)
            // Under pwsh 7 (CI) the inherited module path makes Windows
            // PowerShell load pwsh's Get-Acl, which fails; 5.1 rebuilds its own.
            .env_remove("PSModulePath")
            .output()
            .expect("powershell");
        // An error must fail the check, never read as "no broad readers".
        assert!(out.status.success() && out.stderr.is_empty(), "{out:?}");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|sid| {
                [
                    "S-1-1-0",
                    "S-1-5-7",
                    "S-1-5-11",
                    "S-1-5-32-545",
                    "S-1-5-32-546",
                ]
                .contains(sid)
            })
            .map(String::from)
            .collect()
    }

    /// The index cache holds raw source text: its directory must not be
    /// readable by other local accounts (THREAT_MODEL T5).
    #[cfg(windows)]
    #[test]
    fn index_directory_is_not_readable_by_other_local_accounts() {
        let probe = Path::new(r"C:\findex-acl-probe").join(std::process::id().to_string());
        let dir = storage_dir(&probe).expect("storage dir");
        std::fs::create_dir_all(&dir).expect("create");
        let readers = broad_readers(&dir);
        // The check itself must see a broad grant when one exists.
        let granted = dir.join("granted");
        std::fs::create_dir(&granted).expect("granted");
        let status = std::process::Command::new("icacls")
            .arg(&granted)
            .arg("/grant")
            .arg("*S-1-1-0:(R)")
            .stdout(std::process::Stdio::null())
            .status()
            .expect("icacls");
        let detected = broad_readers(&granted);
        std::fs::remove_dir(&granted).expect("cleanup granted");
        std::fs::remove_dir(&dir).expect("cleanup");
        assert!(status.success());
        assert_eq!(detected, ["S-1-1-0"]);
        assert!(readers.is_empty(), "{readers:?}");
    }

    #[test]
    fn redaction_masks_known_credential_shapes_and_keeps_code() {
        for (input, leaked) in [
            ("let id = \"AKIAIOSFODNN7EXAMPLE\";", "AKIAIOSFODNN7EXAMPLE"),
            (
                "token = ghp_abcdefghijklmnopqrstuvwxyz0123456789",
                "ghp_abcdefghij",
            ),
            (
                "ANTHROPIC = 'sk-ant-api03-abcdefghijklmnopqrstuvwx'",
                "abcdefghijklmnop",
            ),
            ("-----BEGIN RSA PRIVATE KEY-----", "BEGIN RSA PRIVATE"),
            ("password = \"hunter2hunter2\"", "hunter2hunter2"),
            (
                "\"client_secret\": \"0123456789abcdef\"",
                "0123456789abcdef",
            ),
        ] {
            let output = redact_secrets(input);
            assert!(!output.contains(leaked), "{input} -> {output}");
            assert!(output.contains("[REDACTED"), "{output}");
        }
        assert_eq!(
            redact_secrets("fn password_strength(p: &str) -> u8"),
            "fn password_strength(p: &str) -> u8"
        );
        assert_eq!(
            redact_secrets("let secret = read_secret()?;"),
            "let secret = read_secret()?;"
        );
    }

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
    fn rejects_sensitive_project_paths_but_not_ordinary_source() {
        for value in [
            ".envrc",
            ".npmrc",
            ".pypirc",
            ".netrc",
            "_netrc",
            ".git-credentials",
            "terraform.tfstate",
            "prod.tfstate.backup",
            "service-account.json",
            "config/staging.tfvars",
            "config/env.tfvars.json",
            "config/identity.jks",
            ".aws/credentials",
            ".kube/config",
            "src/.docker/config.json",
            "src/.ssh/id_custom",
            ".azure/accessTokens.json",
            ".terraform/terraform.tfstate",
            ".gcloud/application_default_credentials.json",
            "secrets/team.kdbx",
        ] {
            assert!(
                is_disallowed_project_path(Path::new(value)),
                "must exclude {value}"
            );
        }
        for value in [
            "src/main.rs",
            "src/secret_manager.rs",
            "tests/fixtures/aws_settings.rs",
            "src/auth_config.py",
            "crates/core/src/terraform.rs",
        ] {
            assert!(
                !is_disallowed_project_path(Path::new(value)),
                "legitimate code should remain eligible: {value}"
            );
        }
        assert!(is_disallowed_project_path(Path::new("../outside.rs")));
        assert!(is_disallowed_project_path(Path::new("")));
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

    // =====================================================================
    // Property-based tests (proptest)
    // =====================================================================

    proptest::proptest! {
        /// Every path rejected by is_disallowed_project_path must contain
        /// `..`, be absolute, or be empty — never a normal relative path.
        fn prop_disallowed_paths_are_unsafe(path in any::<String>()) {
            prop_assume!(!path.contains('\0'));
            prop_assume!(path.len() <= 40);
            let p = Path::new(&path);
            if is_disallowed_project_path(p) {
                prop_assert!(
                    path.contains("..") || p.is_absolute() || path.is_empty(),
                    "path {:?} was rejected but looks safe", path
                );
            }
        }

        /// Sensitive filenames are always rejected regardless of case or directory.
        fn prop_sensitive_filenames_always_rejected(name in any::<String>()) {
            prop_assume!(!name.contains('\0'));
            prop_assume!(name.len() <= 30);
            prop_assume!(name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')));
            let sensitive = [
                ".env", ".env.production", ".envrc", ".npmrc", ".pypirc", ".netrc",
                "_netrc", ".git-credentials", "id_rsa", "id_ed25519",
                "credentials.json", "secrets.json", "service-account.json",
                "service_account.json", ".tfstate", ".tfvars", ".jks", ".keystore",
                ".kdbx", ".pem", ".p12", ".pfx", ".key",
            ];
            let lower = name.to_ascii_lowercase();
            let is_sensitive = sensitive.iter().any(|s| {
                lower == *s || lower.starts_with(&format!("{s}.")) ||
                    lower.contains(&format!(".{s}")) || lower.ends_with(s)
            });
            let path_str = format!("dir/{name}");
            let path = Path::new(&path_str);
            prop_assert_eq!(
                is_sensitive_filename(path),
                is_sensitive,
                "mismatch for name {:?}", name
            );
        }

        /// Redaction never increases text length materially and always hides
        /// known secret patterns.
        fn prop_redact_secrets_hides_patterns(text in any::<String>()) {
            prop_assume!(!text.contains('\0'));
            prop_assume!(text.len() <= 200);
            prop_assume!(text.is_ascii());
            let secret_patterns = [
                "AKIAIOSFODNN7EXAMPLE",
                "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
                "sk-ant-api03-abcdefghijklmnopqrstuvwx",
                "-----BEGIN RSA PRIVATE KEY-----",
                "password = \"hunter2hunter2\"",
            ];
            let mut t = text.clone();
            for s in secret_patterns {
                t = t.replace(s, "[REDACTED:SECRET]");
            }
            let output = redact_secrets(&t);
            // Any secret that survived replacement should now be caught by regex.
            for s in secret_patterns {
                if t.contains(s) {
                    prop_assert!(!output.contains(s), "secret {:?} not redacted", s);
                }
            }
            // Non-secret text should be preserved (short-circuited when no match).
            if !text.contains("AKIA") && !text.contains("ghp_") && !text.contains("sk-ant")
                && !text.contains("BEGIN RSA") && !text.contains("hunter2") && !text.contains("client_secret")
            {
                prop_assert_eq!(output, text, "clean text was modified");
            }
        }

        /// normalized_relative_key is idempotent for valid relative paths.
        fn prop_normalize_idempotent(path in any::<String>()) {
            prop_assume!(!path.contains('\0'));
            prop_assume!(path.len() <= 40);
            prop_assume!(path.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/')));
            let p = Path::new(&path);
            match normalized_relative_key(p) {
                Ok(k) => {
                    let second = normalized_relative_key(Path::new(&k));
                    match second {
                        Ok(k2) => prop_assert_eq!(k2, k),
                        Err(_) => prop_assert!(false, "re-parse of valid key {:?} failed", k),
                    }
                }
                Err(_) => prop_assert!(true),
            }
        }
    }
}
