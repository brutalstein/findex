//! Bounded, deterministic first-contact map. It is a file *inventory*, not
//! an LLM-generated architectural conclusion or a resolved call graph.
use super::{connect, live_source, synchronize, Result, SyncStats};
use findex_core::storage_dir;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

const MAX_MODULES: usize = 16;
const MAX_LANDMARKS: usize = 20;

#[derive(Debug, Clone, Serialize)]
pub struct InventoryCount {
    pub name: String,
    pub files: usize,
}

#[derive(Debug, Serialize)]
pub struct ProjectOverview {
    pub indexed_files: usize,
    pub probable_test_files: usize,
    pub languages_by_extension: Vec<InventoryCount>,
    pub top_level_modules: Vec<InventoryCount>,
    /// Only landmarks still matching a source hash are returned.
    pub manifest_candidates: Vec<String>,
    pub entrypoint_candidates: Vec<String>,
    pub stale_landmarks_discarded: usize,
    pub truncated: bool,
    pub sync: SyncStats,
    pub evidence_scope: &'static str,
}

fn language(path: &str) -> &'static str {
    let extension = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" => "javascript",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "h" | "c" => "cpp_or_c",
        "java" => "java",
        "go" => "go",
        "cs" => "csharp",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "rb" => "ruby",
        "toml" => "toml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" | "mdx" => "markdown",
        _ => "other_or_no_extension",
    }
}

fn module(path: &str) -> String {
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() < 2 {
        return "(root)".into();
    }
    if matches!(
        parts[0],
        "crates" | "packages" | "apps" | "services" | "modules"
    ) && parts.len() > 2
    {
        return format!("{}/{}", parts[0], parts[1]);
    }
    parts[0].to_string()
}

fn landmarks(path: &str) -> (bool, bool) {
    let lower = path.to_ascii_lowercase();
    let filename = lower.rsplit('/').next().unwrap_or_default();
    let manifest = matches!(
        filename,
        "cargo.toml"
            | "package.json"
            | "pyproject.toml"
            | "cmakelists.txt"
            | "go.mod"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "meson.build"
            | "requirements.txt"
            | "composer.json"
    );
    let entry = matches!(
        filename,
        "main.rs"
            | "main.py"
            | "main.ts"
            | "main.tsx"
            | "main.cpp"
            | "main.cc"
            | "main.c"
            | "app.py"
            | "app.ts"
            | "index.ts"
            | "index.tsx"
            | "index.js"
            | "program.cs"
            | "manage.py"
    );
    (manifest, entry)
}

fn is_test(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or_default();
    lower.starts_with("tests/")
        || lower.contains("/tests/")
        || lower.contains("/__tests__/")
        || name.starts_with("test_")
        || name.ends_with("_test.rs")
        || name.contains(".test.")
        || name.contains(".spec.")
}

fn top(counts: BTreeMap<String, usize>, cap: usize) -> (Vec<InventoryCount>, bool) {
    let truncated = counts.len() > cap;
    let mut ordered: Vec<_> = counts
        .into_iter()
        .map(|(name, files)| InventoryCount { name, files })
        .collect();
    ordered.sort_by(|a, b| b.files.cmp(&a.files).then_with(|| a.name.cmp(&b.name)));
    ordered.truncate(cap);
    (ordered, truncated)
}

/// Refresh once on explicit request; do not schedule scans in a hook or for
/// every Claude message. Any read/scan failure aborts this inventory.
pub fn overview(project: &Path) -> Result<ProjectOverview> {
    let sync = synchronize(project)?;
    let database = storage_dir(project)?.join("index.sqlite3");
    overview_in_db(project, &database, sync)
}

pub(crate) fn overview_in_db(
    project: &Path,
    database: &Path,
    sync: SyncStats,
) -> Result<ProjectOverview> {
    let root = project.canonicalize()?;
    let conn = connect(database)?;
    let mut languages = BTreeMap::<String, usize>::new();
    let mut modules = BTreeMap::<String, usize>::new();
    let mut candidates = Vec::<(String, String, bool, bool)>::new();
    let mut tests = 0;
    let mut total = 0;
    let mut statement = conn.prepare("SELECT path, hash FROM files ORDER BY path")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (path, digest) = row?;
        total += 1;
        *languages.entry(language(&path).to_string()).or_default() += 1;
        *modules.entry(module(&path)).or_default() += 1;
        if is_test(&path) {
            tests += 1;
        }
        let (manifest, entry) = landmarks(&path);
        if manifest || entry {
            candidates.push((path, digest, manifest, entry));
        }
    }
    drop(statement);
    drop(conn);
    // Verify only bounded candidate source bytes. Counts explicitly describe
    // the indexed snapshot and are not claims of a live exhaustive filesystem.
    let (languages_by_extension, language_cut) = top(languages, MAX_MODULES);
    let (top_level_modules, module_cut) = top(modules, MAX_MODULES);
    let mut manifest_candidates = Vec::new();
    let mut entrypoint_candidates = Vec::new();
    let mut stale = 0;
    let mut truncated = language_cut || module_cut;
    for (path, digest, manifest, entry) in candidates {
        if manifest && manifest_candidates.len() >= MAX_LANDMARKS
            || entry && entrypoint_candidates.len() >= MAX_LANDMARKS
        {
            truncated = true;
            continue;
        }
        if live_source(&root, &path, &digest).is_none() {
            stale += 1;
            continue;
        }
        if manifest {
            manifest_candidates.push(path.clone());
        }
        if entry {
            entrypoint_candidates.push(path);
        }
    }
    Ok(ProjectOverview {
        indexed_files: total,
        probable_test_files: tests,
        languages_by_extension,
        top_level_modules,
        manifest_candidates,
        entrypoint_candidates,
        stale_landmarks_discarded: stale,
        truncated,
        sync,
        evidence_scope: "Indexed/admitted files only. Directory/language counts are index-snapshot hints; landmarks are hash-verified candidates, not proven runtime entry points. Ignored/oversized/unavailable source is not assessed. Read source before conclusions.",
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::synchronize_into;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn multilingual_first_contact_map_is_bounded_and_source_verified() {
        let project = tempdir().expect("project");
        let state = tempdir().expect("state");
        let files = [
            (
                "Cargo.toml",
                "[package]\nname=\"demo\"\nversion=\"0.1.0\"\n",
            ),
            ("crates/core/src/main.rs", "fn main() {}\n"),
            ("crates/core/src/lib.rs", "pub fn start() {}\n"),
            ("packages/web/package.json", "{\"name\":\"demo\"}\n"),
            ("packages/web/src/index.ts", "export function start() {}\n"),
            ("tests/test_engine.py", "def test_engine(): pass\n"),
        ];
        for (path, body) in files {
            let destination = project.path().join(path);
            fs::create_dir_all(destination.parent().expect("parent")).expect("mkdir");
            fs::write(destination, body).expect("write");
        }
        fs::write(
            project.path().join(".env"),
            "PRIVATE_TOKEN_SHOULD_NOT_APPEAR",
        )
        .expect("private");
        let db = state.path().join("db.sqlite3");
        synchronize_into(project.path(), &db).expect("sync");
        let report = overview_in_db(project.path(), &db, SyncStats::default()).expect("overview");
        assert_eq!(report.indexed_files, files.len());
        assert_eq!(report.probable_test_files, 1);
        assert!(report
            .manifest_candidates
            .contains(&"Cargo.toml".to_string()));
        assert!(report
            .entrypoint_candidates
            .contains(&"crates/core/src/main.rs".to_string()));
        assert!(report
            .entrypoint_candidates
            .contains(&"packages/web/src/index.ts".to_string()));
        assert!(!report
            .manifest_candidates
            .iter()
            .any(|x| x.contains(".env")));
        assert!(!report.truncated);
        let modules: Vec<_> = report
            .top_level_modules
            .iter()
            .map(|x| x.name.as_str())
            .collect();
        assert!(modules.contains(&"crates/core"));
        assert!(modules.contains(&"packages/web"));

        fs::write(
            project.path().join("crates/core/src/main.rs"),
            "fn different() {}\n",
        )
        .expect("change");
        let stale =
            overview_in_db(project.path(), &db, SyncStats::default()).expect("second overview");
        assert!(!stale
            .entrypoint_candidates
            .contains(&"crates/core/src/main.rs".to_string()));
        assert_eq!(stale.stale_landmarks_discarded, 1);
    }
}
