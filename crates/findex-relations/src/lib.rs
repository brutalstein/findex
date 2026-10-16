//! Explicit, bounded semantic-reference import and evidence-backed file impact.
//! No guessed call edges, hidden language server execution or external network calls.

mod lsp;
mod scip_import;

use findex_core::{is_sensitive_filename, normalized_relative_key, storage_dir, MAX_SOURCE_BYTES};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
pub use lsp::import_lsp_snapshot;
pub use scip_import::{import_scip, import_scip_bytes};

fn language_for_path(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    match extension.as_str() {
        "rs" => "rust",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "h" | "c" => "cpp",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" => "javascript",
        "py" | "pyi" => "python",
        "go" => "go",
        "java" => "java",
        "kt" => "kotlin",
        "swift" => "swift",
        "cs" => "csharp",
        _ => "unknown",
    }
}

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENTS: usize = 1_000;
const MAX_OCCURRENCES: usize = 100_000;
const MAX_SYMBOL_LENGTH: usize = 2_048;
const MAX_IMPACT_FILES: usize = 64;
const MAX_DEPTH: usize = 3;
/// Relations schema is independent of the lexical index schema.
const RELATIONS_SCHEMA: i64 = 2;
const MAX_TYPED_RESULTS: usize = 128;

#[derive(Debug)]
struct Evidence {
    path: String,
    digest: String,
    lines: usize,
}
#[derive(Clone, Debug)]
struct Occurrence {
    symbol: String,
    path: String,
    line: usize,
    definition: bool,
}
/// A source-positioned semantic observation, never an inferred call edge.
#[derive(Debug)]
struct SemanticSite {
    symbol: String,
    path: String,
    line: usize,
    column: usize,
    language: String,
    kind: &'static str,
}
/// SCIP SymbolInformation.Relationship, not an invented call relationship.
#[derive(Debug)]
struct SemanticLink {
    owner: String,
    target: String,
    kind: &'static str,
    path: String,
    language: String,
}
#[derive(Debug, Serialize)]
pub struct SymbolSite {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub language: String,
    pub kind: String,
    pub provider: String,
}
#[derive(Debug, Serialize)]
pub struct SymbolLink {
    pub target_symbol: String,
    pub kind: String,
    pub path: String,
    pub language: String,
    pub provider: String,
}
#[derive(Debug, Serialize)]
pub struct SymbolEvidenceReport {
    pub symbol: String,
    pub sites: Vec<SymbolSite>,
    pub relationships: Vec<SymbolLink>,
    pub stale_evidence_discarded: usize,
    pub truncated: bool,
    pub interpretation: &'static str,
}
#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub source: &'static str,
    pub documents: usize,
    pub definitions: usize,
    pub references: usize,
    pub storage: &'static str,
    pub typed_sites: usize,
    pub typed_relationships: usize,
    pub schema_version: i64,
}
#[derive(Debug, Serialize)]
pub struct ImpactHit {
    pub path: String,
    /// Direct is proven reference evidence. Transitive is a potential file impact.
    pub depth: usize,
    pub via_symbol: String,
    pub referenced_at_line: usize,
    pub definition_path: String,
    pub definition_line: usize,
    pub provenance: String,
    pub interpretation: &'static str,
}
#[derive(Debug, Serialize)]
pub struct ImpactReport {
    pub changed_file: String,
    pub source: String,
    pub hits: Vec<ImpactHit>,
    pub stale_evidence_discarded: usize,
    pub truncated: bool,
    pub interpretation: &'static str,
}
fn database(project: &Path) -> Result<PathBuf> {
    Ok(storage_dir(project)?.join("relations.sqlite3"))
}
/// Schema setup can race other processes creating the same new database;
/// SQLite reports those lock-order conflicts as immediate SQLITE_BUSY without
/// consulting the busy handler, so retry like the lexical index does.
fn open(project: &Path) -> Result<Connection> {
    let db = database(project)?;
    if let Some(parent) = db.parent() {
        fs::create_dir_all(parent)?;
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match try_open(&db) {
            Err(rusqlite::Error::SqliteFailure(failure, _))
                if matches!(
                    failure.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            other => return Ok(other?),
        }
    }
}
fn try_open(db: &Path) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(db)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    // Never alter or interpret an index created by a newer binary.
    if version > RELATIONS_SCHEMA {
        return Err(rusqlite::Error::InvalidQuery);
    }
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA secure_delete=ON;")?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let version: i64 = tx.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > RELATIONS_SCHEMA {
        return Err(rusqlite::Error::InvalidQuery);
    }
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS snapshot(
            id INTEGER PRIMARY KEY CHECK(id = 1), source TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS evidence_files(
            path TEXT PRIMARY KEY, digest TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS occurrences(
            symbol TEXT NOT NULL, path TEXT NOT NULL, line INTEGER NOT NULL,
            is_definition INTEGER NOT NULL CHECK(is_definition IN (0,1)),
            PRIMARY KEY(symbol,path,line,is_definition)
        );
        CREATE INDEX IF NOT EXISTS occ_target ON occurrences(path,is_definition,symbol);
        CREATE INDEX IF NOT EXISTS occ_symbol ON occurrences(symbol,is_definition,path);
        CREATE TABLE IF NOT EXISTS symbol_provenance(
            symbol TEXT PRIMARY KEY, source TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS semantic_sites(
            symbol TEXT NOT NULL, path TEXT NOT NULL, line INTEGER NOT NULL,
            column INTEGER NOT NULL, language TEXT NOT NULL,
            kind TEXT NOT NULL CHECK(kind IN ('definition','import','reference')),
            PRIMARY KEY(symbol,path,line,column,kind)
        );
        CREATE INDEX IF NOT EXISTS semantic_sites_symbol
          ON semantic_sites(symbol,path,line);
        CREATE TABLE IF NOT EXISTS semantic_links(
            owner_symbol TEXT NOT NULL, target_symbol TEXT NOT NULL,
            kind TEXT NOT NULL CHECK(kind IN
              ('implements','reference_equivalence','type_definition','definition_alias')),
            path TEXT NOT NULL, language TEXT NOT NULL,
            PRIMARY KEY(owner_symbol,target_symbol,kind,path)
        );
        CREATE INDEX IF NOT EXISTS semantic_links_owner ON semantic_links(owner_symbol);"
    )?;
    // A schema-0/legacy snapshot cannot provide typed information until
    // explicitly reimported. Its existing untyped impact evidence is preserved.
    if version < RELATIONS_SCHEMA {
        tx.execute_batch(&format!("PRAGMA user_version = {RELATIONS_SCHEMA};"))?;
    }
    tx.commit()?;
    Ok(conn)
}
fn project_file(project: &Path, relative: &Path) -> Result<(String, PathBuf)> {
    let path = normalized_relative_key(relative)?;
    let root = project.canonicalize()?;
    let full = root.join(relative);
    if is_sensitive_filename(&full) {
        return Err("secret-named files are excluded".into());
    }
    let meta = fs::symlink_metadata(&full)?;
    if !meta.file_type().is_file() || meta.len() > MAX_SOURCE_BYTES {
        return Err("expected non-symlink source file within the size budget".into());
    }
    if !full.canonicalize()?.starts_with(&root) {
        return Err("file escapes workspace".into());
    }
    Ok((path, full))
}
/// Restrict imported paths to the exact same eligible and indexed sources as
/// the lexical engine; never trust a SCIP producer's document paths or content.
fn admitted(project: &Path, db: &Connection, raw: &str) -> Result<Evidence> {
    if raw.is_empty() || raw.len() > 1024 || raw.chars().any(char::is_control) {
        return Err("invalid document relative path".into());
    }
    let relative = Path::new(raw);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("document has absolute/traversal path".into());
    }
    let (path, full) = project_file(project, relative)?;
    let old: Option<String> = db
        .query_row(
            "SELECT hash FROM files WHERE path = ?1",
            params![path],
            |r| r.get(0),
        )
        .optional()?;
    let Some(indexed_hash) = old else {
        return Err(format!("document not admitted by scanner: {path}").into());
    };
    let bytes = fs::read(full)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES || bytes.contains(&0) {
        return Err("invalid source file bytes".into());
    }
    let source = std::str::from_utf8(&bytes)?;
    let digest = blake3::hash(&bytes).to_hex().to_string();
    if digest != indexed_hash {
        return Err("source changed after index reconciliation".into());
    }
    Ok(Evidence {
        path,
        digest,
        lines: source.lines().count().max(1),
    })
}
fn prepare(project: &Path) -> Result<Connection> {
    // Scan performs .gitignore / hidden / binary / secrets checks consistently.
    findex_index::synchronize(project)?;
    let index = storage_dir(project)?.join("index.sqlite3");
    Ok(Connection::open_with_flags(
        index,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?)
}
fn persist(
    project: &Path,
    origin: &'static str,
    files: &[Evidence],
    refs: &[Occurrence],
    sites: &[SemanticSite],
    links: &[SemanticLink],
) -> Result<ImportReport> {
    let mut conn = open(project)?;
    // IMMEDIATE takes the write lock up front: a deferred read-then-write
    // transaction fails with SQLITE_BUSY at once when another writer committed.
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    // Complete SCIP indexes replace the graph; individual LSP captures merge
    // by their stable definition-anchor identity, never deleting unrelated symbols.
    let incremental_lsp = origin == "lsp-captured-references";
    let incoming: HashSet<&str> = refs
        .iter()
        .map(|reference| reference.symbol.as_str())
        .chain(links.iter().map(|link| link.owner.as_str()))
        .collect();
    let previous_source: Option<String> = if incremental_lsp {
        tx.query_row("SELECT source FROM snapshot WHERE id=1", [], |row| {
            row.get(0)
        })
        .optional()?
    } else {
        None
    };
    if incremental_lsp {
        // An imported file that changed invalidates *every* old relation
        // mentioning that file, including symbols defined in other files.
        // Otherwise updating its digest would accidentally revive stale edges.
        for file in files {
            let old: Option<String> = tx
                .query_row(
                    "SELECT digest FROM evidence_files WHERE path=?1",
                    params![file.path],
                    |row| row.get(0),
                )
                .optional()?;
            if old.is_some_and(|digest| digest != file.digest) {
                tx.execute("DELETE FROM semantic_links WHERE path=?1", params![file.path])?;
                tx.execute("DELETE FROM semantic_sites WHERE path=?1", params![file.path])?;
                let mut stmt =
                    tx.prepare("SELECT DISTINCT symbol FROM occurrences WHERE path=?1")?;
                let affected = stmt
                    .query_map(params![file.path], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for symbol in affected {
                    tx.execute("DELETE FROM semantic_sites WHERE symbol=?1", params![symbol])?;
                    tx.execute("DELETE FROM semantic_links WHERE owner_symbol=?1", params![symbol])?;
                    tx.execute("DELETE FROM occurrences WHERE symbol=?1", params![symbol])?;
                    tx.execute(
                        "DELETE FROM symbol_provenance WHERE symbol=?1",
                        params![symbol],
                    )?;
                }
            }
        }
        for symbol in &incoming {
            tx.execute("DELETE FROM semantic_sites WHERE symbol=?1", params![symbol])?;
            tx.execute("DELETE FROM semantic_links WHERE owner_symbol=?1", params![symbol])?;
            tx.execute("DELETE FROM occurrences WHERE symbol=?1", params![symbol])?;
            tx.execute(
                "DELETE FROM symbol_provenance WHERE symbol=?1",
                params![symbol],
            )?;
        }
    } else {
        tx.execute("DELETE FROM semantic_sites", [])?;
        tx.execute("DELETE FROM semantic_links", [])?;
        tx.execute("DELETE FROM occurrences", [])?;
        tx.execute("DELETE FROM evidence_files", [])?;
        tx.execute("DELETE FROM symbol_provenance", [])?;
    }
    for file in files {
        tx.execute(
            "INSERT INTO evidence_files(path,digest) VALUES(?1,?2)
             ON CONFLICT(path) DO UPDATE SET digest=excluded.digest",
            params![file.path, file.digest],
        )?;
    }
    let mut definitions = 0;
    let mut references = 0;
    {
        let mut insert = tx.prepare(
            "INSERT OR IGNORE INTO occurrences(symbol,path,line,is_definition)
             VALUES (?1,?2,?3,?4)",
        )?;
        for occurrence in refs {
            let inserted = insert.execute(params![
                occurrence.symbol,
                occurrence.path,
                occurrence.line as i64,
                i64::from(occurrence.definition)
            ])?;
            if inserted != 0 {
                if occurrence.definition {
                    definitions += 1;
                } else {
                    references += 1;
                }
            }
        }
    }
    {
        let mut insert = tx.prepare(
            "INSERT OR IGNORE INTO semantic_sites(symbol,path,line,column,language,kind)
             VALUES(?1,?2,?3,?4,?5,?6)",
        )?;
        for site in sites {
            insert.execute(params![
                site.symbol, site.path, site.line as i64, site.column as i64,
                site.language, site.kind,
            ])?;
        }
    }
    {
        let mut insert = tx.prepare(
            "INSERT OR IGNORE INTO semantic_links(
                 owner_symbol,target_symbol,kind,path,language
             ) VALUES(?1,?2,?3,?4,?5)",
        )?;
        for link in links {
            insert.execute(params![
                link.owner, link.target, link.kind, link.path, link.language,
            ])?;
        }
    }
    for symbol in incoming {
        tx.execute(
            "INSERT INTO symbol_provenance(symbol,source) VALUES(?1,?2)
             ON CONFLICT(symbol) DO UPDATE SET source=excluded.source",
            params![symbol, origin],
        )?;
    }
    tx.execute(
        "DELETE FROM evidence_files WHERE path NOT IN (SELECT DISTINCT path FROM occurrences)
           AND path NOT IN (SELECT DISTINCT path FROM semantic_sites)
           AND path NOT IN (SELECT DISTINCT path FROM semantic_links)",
        [],
    )?;
    let snapshot_source = if incremental_lsp
        && previous_source
            .as_deref()
            .is_some_and(|source| source != origin)
    {
        "mixed-scip-lsp-evidence"
    } else {
        origin
    };
    tx.execute(
        "INSERT INTO snapshot(id,source) VALUES(1,?1)
         ON CONFLICT(id) DO UPDATE SET source=excluded.source",
        params![snapshot_source],
    )?;
    tx.commit()?;
    Ok(ImportReport {
        source: origin,
        documents: files.len(),
        definitions,
        references,
        storage: "sqlite-wal-atomic-snapshot",
        typed_sites: sites.len(),
        typed_relationships: links.len(),
        schema_version: RELATIONS_SCHEMA,
    })
}
fn valid_occurrence(
    file: &Evidence,
    symbol: &str,
    line: usize,
    definition: bool,
) -> Result<Occurrence> {
    if symbol.is_empty()
        || symbol.len() > MAX_SYMBOL_LENGTH
        || symbol.chars().any(char::is_control)
        || line == 0
        || line > file.lines
    {
        return Err("invalid symbol or zero-based source location".into());
    }
    Ok(Occurrence {
        symbol: symbol.to_owned(),
        path: file.path.clone(),
        line,
        definition,
    })
}
fn currently_admitted(index: &Connection, path: &str, digest: &str) -> bool {
    index
        .query_row(
            "SELECT hash FROM files WHERE path = ?1",
            params![path],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
        .is_some_and(|indexed| indexed == digest)
}

fn verify_path(project: &Path, path: &str, digest: &str) -> bool {
    let Ok((_, full)) = project_file(project, Path::new(path)) else {
        return false;
    };
    fs::read(full).ok().is_some_and(|bytes| {
        bytes.len() as u64 <= MAX_SOURCE_BYTES && blake3::hash(&bytes).to_hex().as_str() == digest
    })
}

/// Safe exact-symbol lookup. All returned paths are rechecked against both the
/// scanner's admission digest and live source bytes, including link-only paths.
/// Reference observations never imply CALLS_RESOLVED.
pub fn symbol_evidence(project: &Path, symbol: &str, limit: usize) -> Result<SymbolEvidenceReport> {
    if symbol.is_empty()
        || symbol.len() > MAX_SYMBOL_LENGTH
        || symbol.chars().any(char::is_control)
    {
        return Err("invalid semantic symbol identity".into());
    }
    let indexed = prepare(project)?;
    let conn = open(project)?;
    let max = limit.clamp(1, MAX_TYPED_RESULTS);
    let mut report = SymbolEvidenceReport {
        symbol: symbol.to_owned(),
        sites: Vec::new(),
        relationships: Vec::new(),
        stale_evidence_discarded: 0,
        truncated: false,
        interpretation: "SCIP/LSP source-backed observations; references are not proven function calls",
    };
    let mut stmt = conn.prepare(
        "SELECT s.path, s.line, s.column, s.language, s.kind,
                e.digest, COALESCE(p.source, 'unattributed')
         FROM semantic_sites s
         JOIN evidence_files e ON e.path=s.path
         LEFT JOIN symbol_provenance p ON p.symbol=s.symbol
         WHERE s.symbol=?1
         ORDER BY s.path, s.line, s.column, s.kind LIMIT ?2",
    )?;
    let entries = stmt.query_map(params![symbol, (max + 1) as i64], |r| {
        Ok((
            r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
        ))
    })?;
    for row in entries {
        let (path, line, column, language, kind, digest, provider) = row?;
        if report.sites.len() >= max {
            report.truncated = true;
            break;
        }
        if !currently_admitted(&indexed, &path, &digest)
            || !verify_path(project, &path, &digest)
        {
            report.stale_evidence_discarded += 1;
            continue;
        }
        report.sites.push(SymbolSite {
            path, line: usize::try_from(line)?, column: usize::try_from(column)?,
            language, kind, provider,
        });
    }
    let mut rel_stmt = conn.prepare(
        "SELECT r.target_symbol, r.kind, r.path, r.language, e.digest,
                COALESCE(p.source, 'unattributed')
         FROM semantic_links r
         JOIN evidence_files e ON e.path=r.path
         LEFT JOIN symbol_provenance p ON p.symbol=r.owner_symbol
         WHERE r.owner_symbol=?1
         ORDER BY r.kind,r.path,r.target_symbol LIMIT ?2",
    )?;
    let links = rel_stmt.query_map(params![symbol, (max + 1) as i64], |r| {
        Ok((
            r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
            r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?,
        ))
    })?;
    for row in links {
        let (target_symbol, kind, path, language, digest, provider) = row?;
        if report.relationships.len() >= max {
            report.truncated = true;
            break;
        }
        if !currently_admitted(&indexed, &path, &digest)
            || !verify_path(project, &path, &digest)
        {
            report.stale_evidence_discarded += 1;
            continue;
        }
        report.relationships.push(SymbolLink {
            target_symbol, kind, path, language, provider,
        });
    }
    Ok(report)
}

/// Breadth-first *file* impact. A link means "this file references a symbol
/// defined in that file"; no exact invocation, execution, or impact guarantee.
pub fn impact(project: &Path, relative: &Path, depth: usize, limit: usize) -> Result<ImpactReport> {
    // Reapply current scanner admission, even when ignore rules changed since import.
    let accepted_files = prepare(project)?;
    let (changed, _) = project_file(project, relative)?;
    let conn = open(project)?;
    let origin: Option<String> = conn
        .query_row("SELECT source FROM snapshot WHERE id=1", [], |r| r.get(0))
        .optional()?;
    let source =
        origin.ok_or("no semantic snapshot: import SCIP or captured LSP references first")?;
    let mut report = ImpactReport {
        changed_file: changed.clone(),
        source,
        hits: Vec::new(),
        stale_evidence_discarded: 0,
        truncated: false,
        interpretation: "direct-reference-evidence;transitive-potential-impact-not-call-graph",
    };
    let source_hashes: HashMap<String, String> = {
        let mut stmt = conn.prepare("SELECT path,digest FROM evidence_files")?;
        let results = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        results
    };
    let Some(digest) = source_hashes.get(&changed) else {
        return Err("target path has no semantic snapshot evidence".into());
    };
    if !currently_admitted(&accepted_files, &changed, digest)
        || !verify_path(project, &changed, digest)
    {
        return Err("target changed or is no longer admitted; reimport evidence".into());
    }
    let mut visited = HashSet::from([changed.clone()]);
    let mut queue = VecDeque::from([(changed, 0_usize)]);
    let max_depth = depth.clamp(1, MAX_DEPTH);
    let max_hits = limit.clamp(1, MAX_IMPACT_FILES);
    let mut checked = HashMap::<String, bool>::new();

    while let Some((definition_file, current_depth)) = queue.pop_front() {
        if current_depth >= max_depth {
            continue;
        }
        // The query is intentionally keyed by the exact semantic symbol ID.
        // No name matching, fuzzy AST guesses or generated call edges.
        let mut stmt = conn.prepare(
            "SELECT DISTINCT refs.path,refs.line,defs.symbol,defs.line,
                    COALESCE(sp.source,?2)
             FROM occurrences AS defs
             JOIN occurrences AS refs
               ON refs.symbol=defs.symbol AND refs.is_definition=0
             LEFT JOIN symbol_provenance AS sp ON sp.symbol=defs.symbol
             WHERE defs.path=?1 AND defs.is_definition=1 AND refs.path<>defs.path
             ORDER BY refs.path,refs.line,defs.symbol LIMIT 4097",
        )?;
        let rows = stmt.query_map(params![definition_file, report.source], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        for (candidate_index, row) in rows.enumerate() {
            // Never present a silently incomplete graph when the SQL
            // expansion budget is exhausted.
            if candidate_index >= 4096 {
                report.truncated = true;
                return Ok(report);
            }
            let (ref_path, ref_line, symbol, def_line, provenance) = row?;
            if visited.contains(&ref_path) {
                continue;
            }
            let Some(current_hash) = source_hashes.get(&ref_path) else {
                continue;
            };
            let verified = *checked.entry(ref_path.clone()).or_insert_with(|| {
                currently_admitted(&accepted_files, &ref_path, current_hash)
                    && verify_path(project, &ref_path, current_hash)
            });
            if !verified {
                report.stale_evidence_discarded += 1;
                continue;
            }
            visited.insert(ref_path.clone());
            let next_depth = current_depth + 1;
            report.hits.push(ImpactHit {
                path: ref_path.clone(),
                depth: next_depth,
                via_symbol: symbol,
                referenced_at_line: usize::try_from(ref_line)?,
                definition_path: definition_file.clone(),
                definition_line: usize::try_from(def_line)?,
                provenance,
                interpretation: if next_depth == 1 {
                    "verified-semantic-reference"
                } else {
                    "potential-transitive-file-impact"
                },
            });
            queue.push_back((ref_path, next_depth));
            if report.hits.len() >= max_hits {
                report.truncated = true;
                return Ok(report);
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fixture() -> (tempfile::TempDir, String) {
        let root = tempdir().expect("root");
        fs::write(root.path().join("a.rs"), "fn target() {}\n").expect("a");
        fs::write(root.path().join("b.rs"), "fn use_target() { target(); }\n").expect("b");
        fs::write(root.path().join(".gitignore"), "hidden.rs\n").expect("ignore");
        fs::write(root.path().join("hidden.rs"), "fn ignored() {}\n").expect("ignored");
        let data = r#"{"definition":{"path":"a.rs","line":0,"character":3},"references":[{"path":"b.rs","line":0,"character":18}]}"#;
        (root, data.to_owned())
    }

    #[test]
    fn future_relation_schema_is_rejected_without_mutation() {
        let root = tempdir().expect("workspace");
        let conn = open(root.path()).expect("create");
        conn.execute_batch("PRAGMA user_version=99;").expect("newer");
        drop(conn);
        assert!(open(root.path()).is_err());
        let raw = Connection::open(database(root.path()).expect("db")).expect("inspect");
        let version: i64 = raw.query_row("PRAGMA user_version", [], |r| r.get(0)).expect("version");
        assert_eq!(version, 99);
    }

    #[test]
    fn legacy_relation_schema_upgrades_preserving_reference_impact() {
        let (workspace, lsp) = fixture();
        import_lsp_snapshot(workspace.path(), &lsp).expect("initial");
        {
            let conn = open(workspace.path()).expect("open");
            conn.execute_batch(
                "DROP TABLE semantic_sites; DROP TABLE semantic_links; PRAGMA user_version=0;"
            ).expect("simulate prior version");
        }
        assert_eq!(
            impact(workspace.path(), Path::new("a.rs"), 1, 8)
                .expect("unchanged reference graph").hits.len(), 1
        );
        let newer = symbol_evidence(workspace.path(), "lsp:a.rs:0:3", 8).expect("typed");
        assert!(newer.sites.is_empty(), "old snapshot never fabricated typed evidence");
    }

    #[test]
    fn concurrent_first_opens_of_a_new_database_all_succeed() {
        for _ in 0..20 {
            let root = tempdir().expect("root");
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
            let workers: Vec<_> = (0..8)
                .map(|_| {
                    let (path, barrier) = (root.path().to_path_buf(), barrier.clone());
                    std::thread::spawn(move || {
                        barrier.wait();
                        open(&path).map(drop).map_err(|e| e.to_string())
                    })
                })
                .collect();
            for worker in workers {
                worker.join().expect("thread").expect("open");
            }
        }
    }

    #[test]
    fn lsp_snapshot_generates_direct_file_impact_and_detects_stale_sources() {
        let (workspace, lsp) = fixture();
        let imported = import_lsp_snapshot(workspace.path(), &lsp).expect("import");
        assert_eq!(imported.definitions, 1);
        assert_eq!(imported.references, 1);
        let report = impact(workspace.path(), Path::new("a.rs"), 2, 8).expect("impact");
        assert_eq!(report.hits.len(), 1);
        assert_eq!(report.hits[0].path, "b.rs");
        assert_eq!(report.hits[0].depth, 1);
        fs::write(workspace.path().join("b.rs"), "fn changed() {}").expect("change");
        let stale = impact(workspace.path(), Path::new("a.rs"), 2, 8).expect("stale");
        assert_eq!(stale.hits.len(), 0);
        assert_eq!(stale.stale_evidence_discarded, 1);
        fs::write(workspace.path().join("a.rs"), "fn newer() {}").expect("change");
        assert!(impact(workspace.path(), Path::new("a.rs"), 2, 8).is_err());
    }

    #[test]
    fn rejects_ignored_and_traversal_sources_before_persisting() {
        let (workspace, _) = fixture();
        let ignored =
            r#"{"definition":{"path":"hidden.rs","line":0,"character":0},"references":[]}"#;
        assert!(import_lsp_snapshot(workspace.path(), ignored).is_err());
        let malicious =
            r#"{"definition":{"path":"../outside.rs","line":0,"character":0},"references":[]}"#;
        assert!(import_lsp_snapshot(workspace.path(), malicious).is_err());
        assert!(impact(workspace.path(), Path::new("a.rs"), 1, 10).is_err());
    }

    #[test]
    fn changed_ignore_rules_invalidate_previously_imported_paths() {
        let (workspace, snapshot) = fixture();
        import_lsp_snapshot(workspace.path(), &snapshot).expect("initial import");
        fs::write(workspace.path().join(".gitignore"), "b.rs\n").expect("ignore referencing file");
        let report =
            impact(workspace.path(), Path::new("a.rs"), 1, 10).expect("impact after ignore");
        assert!(report.hits.is_empty());
        assert_eq!(report.stale_evidence_discarded, 1);
        fs::write(workspace.path().join(".gitignore"), "a.rs\n").expect("ignore target");
        assert!(impact(workspace.path(), Path::new("a.rs"), 1, 10).is_err());
    }

    #[test]
    fn lsp_duplicate_references_are_deduplicated_in_snapshot() {
        let (workspace, _) = fixture();
        let json = r#"{"definition":{"path":"a.rs","line":0,"character":3},"references":[{"path":"b.rs","line":0,"character":18},{"path":"b.rs","line":0,"character":18}]}"#;
        let report = import_lsp_snapshot(workspace.path(), json).expect("import");
        assert_eq!(report.definitions, 1);
        assert_eq!(report.references, 1);
        assert_eq!(
            impact(workspace.path(), Path::new("a.rs"), 1, 8)
                .expect("impact")
                .hits
                .len(),
            1
        );
    }

    #[test]
    fn malformed_lsp_positions_and_unknown_fields_keep_previous_snapshot() {
        let (workspace, good) = fixture();
        import_lsp_snapshot(workspace.path(), &good).expect("baseline");
        let cases = [
            r#"{"definition":{"path":"a.rs","line":9999,"character":0},"references":[]}"#,
            r#"{"definition":{"path":"a.rs","line":0,"character":1000001},"references":[]}"#,
            r#"{"definition":{"path":"a.rs","line":0,"character":999},"references":[]}"#,
            r#"{"definition":{"path":"a.rs","line":18446744073709551615,"character":0},"references":[]}"#,
            r#"{"definition":{"path":"a.rs","line":0,"character":0,"extra":true},"references":[]}"#,
            r#"{"definition":{"path":"a.rs","line":0,"character":0},"references":[{"path":"b.rs","line":500,"character":0}]}"#,
        ];
        for malformed in cases {
            assert!(import_lsp_snapshot(workspace.path(), malformed).is_err());
            assert_eq!(
                impact(workspace.path(), Path::new("a.rs"), 1, 8)
                    .expect("preserved snapshot")
                    .hits
                    .len(),
                1
            );
        }
    }

    #[test]
    fn import_rejects_sensitive_or_unindexed_paths_without_replacing_snapshot() {
        let (workspace, baseline) = fixture();
        import_lsp_snapshot(workspace.path(), &baseline).expect("baseline");
        fs::write(workspace.path().join("private.key"), "fn secret() {}").expect("secret fixture");
        let cases = [
            r#"{"definition":{"path":"private.key","line":0,"character":0},"references":[]}"#,
            r#"{"definition":{"path":".gitignore","line":0,"character":0},"references":[]}"#,
            r#"{"definition":{"path":"a.rs","line":0,"character":0},"references":[{"path":"hidden.rs","line":0,"character":0}]}"#,
            r#"{"definition":{"path":"/absolute.rs","line":0,"character":0},"references":[]}"#,
            r#"{"definition":{"path":"a.rs/../b.rs","line":0,"character":0},"references":[]}"#,
        ];
        for malicious in cases {
            assert!(import_lsp_snapshot(workspace.path(), malicious).is_err());
            assert_eq!(
                impact(workspace.path(), Path::new("a.rs"), 1, 8)
                    .expect("preserved snapshot")
                    .hits
                    .len(),
                1
            );
        }
    }

    #[test]
    fn reimport_replaces_old_reference_snapshot_atomically() {
        let (workspace, original) = fixture();
        import_lsp_snapshot(workspace.path(), &original).expect("initial");
        let no_refs = r#"{"definition":{"path":"a.rs","line":0,"character":3},"references":[]}"#;
        let replacement = import_lsp_snapshot(workspace.path(), no_refs).expect("replacement");
        assert_eq!(replacement.references, 0);
        assert!(impact(workspace.path(), Path::new("a.rs"), 3, 64)
            .expect("empty graph")
            .hits
            .is_empty());
    }

    #[test]
    fn result_limit_does_not_bypass_source_verification() {
        let (workspace, _) = fixture();
        fs::write(workspace.path().join("c.rs"), "fn secondary() {}").expect("third file");
        let json = r#"{"definition":{"path":"a.rs","line":0,"character":3},"references":[{"path":"b.rs","line":0,"character":18},{"path":"c.rs","line":0,"character":3}]}"#;
        import_lsp_snapshot(workspace.path(), json).expect("import");
        let limited = impact(workspace.path(), Path::new("a.rs"), 1, 1).expect("limited");
        assert_eq!(limited.hits.len(), 1);
        assert!(limited.truncated);
        fs::write(workspace.path().join("b.rs"), "fn edited() {}").expect("edit");
        let after = impact(workspace.path(), Path::new("a.rs"), 1, 8).expect("stale check");
        assert_eq!(after.hits.len(), 1);
        assert_eq!(after.hits[0].path, "c.rs");
        assert_eq!(after.stale_evidence_discarded, 1);
    }

    #[test]
    fn separately_imported_lsp_symbols_coexist_and_update_independently() {
        let workspace = tempdir().expect("workspace");
        for (name, body) in [
            ("a.rs", "fn alpha() {}\n"),
            ("b.rs", "fn caller_a() { alpha(); }\n"),
            ("c.rs", "fn gamma() {}\n"),
            ("d.rs", "fn caller_c() { gamma(); }\n"),
        ] {
            fs::write(workspace.path().join(name), body).expect("fixture");
        }
        let first = r#"{"definition":{"path":"a.rs","line":0,"character":3},"references":[{"path":"b.rs","line":0,"character":16}]}"#;
        let second = r#"{"definition":{"path":"c.rs","line":0,"character":3},"references":[{"path":"d.rs","line":0,"character":16}]}"#;
        import_lsp_snapshot(workspace.path(), first).expect("first");
        import_lsp_snapshot(workspace.path(), second).expect("second");
        assert_eq!(
            impact(workspace.path(), Path::new("a.rs"), 1, 8)
                .expect("a")
                .hits
                .len(),
            1
        );
        assert_eq!(
            impact(workspace.path(), Path::new("c.rs"), 1, 8)
                .expect("c")
                .hits
                .len(),
            1
        );
        let replace_second =
            r#"{"definition":{"path":"c.rs","line":0,"character":3},"references":[]}"#;
        import_lsp_snapshot(workspace.path(), replace_second).expect("update only second");
        assert_eq!(
            impact(workspace.path(), Path::new("a.rs"), 1, 8)
                .expect("a remains")
                .hits
                .len(),
            1
        );
        assert!(impact(workspace.path(), Path::new("c.rs"), 1, 8)
            .expect("c cleared")
            .hits
            .is_empty());
    }

    #[test]
    fn lsp_batch_imports_multiple_symbols_without_reconciling_per_symbol() {
        let workspace = tempdir().expect("workspace");
        for (name, code) in [
            ("a.rs", "fn alpha() {}\n"),
            ("b.rs", "fn caller_a() { alpha(); }\n"),
            ("c.rs", "fn gamma() {}\n"),
            ("d.rs", "fn caller_c() { gamma(); }\n"),
        ] {
            fs::write(workspace.path().join(name), code).expect("fixture");
        }
        let batch = r#"[{"definition":{"path":"a.rs","line":0,"character":3},"references":[{"path":"b.rs","line":0,"character":16}]},{"definition":{"path":"c.rs","line":0,"character":3},"references":[{"path":"d.rs","line":0,"character":16}]}]"#;
        let imported = import_lsp_snapshot(workspace.path(), batch).expect("batch");
        assert_eq!(imported.definitions, 2);
        assert_eq!(imported.references, 2);
        assert_eq!(
            impact(workspace.path(), Path::new("a.rs"), 1, 8)
                .expect("a")
                .hits
                .len(),
            1
        );
        assert_eq!(
            impact(workspace.path(), Path::new("c.rs"), 1, 8)
                .expect("c")
                .hits
                .len(),
            1
        );
        assert!(import_lsp_snapshot(workspace.path(), "[]").is_err());
        let duplicate = r#"[{"definition":{"path":"a.rs","line":0,"character":3},"references":[]},{"definition":{"path":"a.rs","line":0,"character":3},"references":[]}]"#;
        assert!(import_lsp_snapshot(workspace.path(), duplicate).is_err());
    }

    #[test]
    fn changed_imported_file_does_not_revalidate_unrelated_stale_edges() {
        let (workspace, first) = fixture();
        import_lsp_snapshot(workspace.path(), &first).expect("initial");
        fs::write(workspace.path().join("b.rs"), "fn changed() {}\n").expect("edit");
        let new_symbol = r#"{"definition":{"path":"b.rs","line":0,"character":3},"references":[]}"#;
        import_lsp_snapshot(workspace.path(), new_symbol).expect("incremental import");
        assert!(impact(workspace.path(), Path::new("a.rs"), 1, 8).is_err());
        assert!(impact(workspace.path(), Path::new("b.rs"), 1, 8)
            .expect("new symbol")
            .hits
            .is_empty());
    }

    #[test]
    fn invalid_import_does_not_replace_valid_snapshot() {
        let (workspace, good) = fixture();
        import_lsp_snapshot(workspace.path(), &good).expect("good");
        assert!(import_lsp_snapshot(workspace.path(), "{bad").is_err());
        assert_eq!(
            impact(workspace.path(), Path::new("a.rs"), 1, 10)
                .expect("previous evidence")
                .hits
                .len(),
            1
        );
    }
}
