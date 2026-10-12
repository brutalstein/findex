//! Windows native filesystem-event index, used only by the long-lived MCP process.
//! Events are hints, never a source of truth. Overflow and uncertain events
//! trigger full reconciliation; retrieved code is still verified by its hash.

use findex_core::{is_excluded_scan_entry, storage_dir};
use findex_index::{ContextReport, SearchReport, SymbolSearchReport, SyncStats};
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

const MAX_QUEUED_EVENTS: usize = 2048;
const MAX_BATCH_PATHS: usize = 1024;
const RECONCILE_INTERVAL: Duration = Duration::from_secs(120);

/// On watcher failure, transparently revert to the correctness-first scanner.
pub enum SearchBackend {
    Watched(Box<WatchedIndex>),
    Scanning(PathBuf),
}

impl SearchBackend {
    #[must_use]
    pub fn new(project: &Path) -> Self {
        match WatchedIndex::new(project) {
            Ok(index) => Self::Watched(Box::new(index)),
            Err(err) => {
                eprintln!("findex: watcher unavailable ({err}); using full-scan fallback");
                Self::Scanning(project.to_path_buf())
            }
        }
    }

    pub fn search_symbols(&mut self, name: &str, limit: usize) -> Result<SymbolSearchReport> {
        match self {
            Self::Watched(index) => match index.search_symbols(name, limit) {
                Ok(report) => Ok(report),
                Err(err) => {
                    let project = index.project.clone();
                    eprintln!("findex: native watcher symbol lookup failed ({err}); switching to full scans");
                    *self = Self::Scanning(project.clone());
                    findex_index::search_symbols(&project, name, limit)
                }
            },
            Self::Scanning(project) => findex_index::search_symbols(project, name, limit),
        }
    }

    pub fn inspect_file(&mut self, relative: &Path) -> Result<Option<findex_syntax::Analysis>> {
        match self {
            Self::Watched(index) => match index.inspect_file(relative) {
                Ok(analysis) => Ok(analysis),
                Err(err) => {
                    let project = index.project.clone();
                    eprintln!("findex: native watcher file inspection failed ({err}); switching to full scans");
                    *self = Self::Scanning(project.clone());
                    findex_index::inspect_indexed_file(&project, relative)
                }
            },
            Self::Scanning(project) => findex_index::inspect_indexed_file(project, relative),
        }
    }

    pub fn context(&mut self, task: &str, budget_tokens: usize) -> Result<ContextReport> {
        match self {
            Self::Watched(index) => match index.context(task, budget_tokens) {
                Ok(report) => Ok(report),
                Err(err) => {
                    let project = index.project.clone();
                    eprintln!("findex: native watcher context lookup failed ({err}); switching to full scans");
                    *self = Self::Scanning(project.clone());
                    findex_index::context(&project, task, budget_tokens)
                }
            },
            Self::Scanning(project) => findex_index::context(project, task, budget_tokens),
        }
    }

    pub fn search(&mut self, query: &str, limit: usize) -> Result<SearchReport> {
        match self {
            Self::Watched(index) => match index.search(query, limit) {
                Ok(report) => Ok(report),
                Err(err) => {
                    // Event streams can fail after a successful initial watch.
                    // Never strand Claude with an unusable MCP search tool.
                    let project = index.project.clone();
                    eprintln!("findex: native watcher failed ({err}); switching to full scans");
                    *self = Self::Scanning(project.clone());
                    findex_index::search(&project, query, limit)
                }
            },
            Self::Scanning(project) => findex_index::search(project, query, limit),
        }
    }
}

/// Owns a native Windows watcher for exactly one canonical workspace.
/// There is no background indexing worker or externally exposed network port.
pub struct WatchedIndex {
    _watcher: notify::RecommendedWatcher,
    changes: Receiver<notify::Result<Event>>,
    overflow: Arc<AtomicBool>,
    project: PathBuf,
    database: PathBuf,
    last_reconciliation: Instant,
}

impl WatchedIndex {
    pub fn new(project: &Path) -> Result<Self> {
        let project = project.canonicalize()?;
        let database = storage_dir(&project)?.join("index.sqlite3");
        let (tx, changes) = mpsc::sync_channel(MAX_QUEUED_EVENTS);
        let overflow = Arc::new(AtomicBool::new(false));
        let dropped = Arc::clone(&overflow);
        // Never block a filesystem watcher callback on the indexing thread.
        let mut watcher = notify::recommended_watcher(move |event| match tx.try_send(event) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                dropped.store(true, Ordering::Release);
            }
        })?;
        // Install the watcher *before* reconciliation so concurrent writes are
        // queued and inspected before the next search.
        watcher.watch(&project, RecursiveMode::Recursive)?;
        findex_index::synchronize(&project)?;
        Ok(Self {
            _watcher: watcher,
            changes,
            overflow,
            project,
            database,
            last_reconciliation: Instant::now(),
        })
    }

    pub fn search(&mut self, query: &str, limit: usize) -> Result<SearchReport> {
        let sync = self.refresh_before_search()?;
        let mut report =
            findex_index::search_in_db(&self.project, &self.database, query, limit, sync)?;
        report.retrieval = "sqlite-fts5-bm25-native-watch";
        Ok(report)
    }

    pub fn search_symbols(&mut self, name: &str, limit: usize) -> Result<SymbolSearchReport> {
        let sync = self.refresh_before_search()?;
        let mut report =
            findex_index::search_symbols_in_db(&self.project, &self.database, name, limit)?;
        report.sync = sync;
        report.retrieval = "sqlite-symbol-exact-hash-native-watch";
        Ok(report)
    }

    pub fn context(&mut self, task: &str, budget_tokens: usize) -> Result<ContextReport> {
        let sync = self.refresh_before_search()?;
        findex_index::context_in_db(&self.project, &self.database, task, budget_tokens, sync)
    }

    pub fn inspect_file(&mut self, relative: &Path) -> Result<Option<findex_syntax::Analysis>> {
        self.refresh_before_search()?;
        findex_index::inspect_indexed_file_in_db(&self.project, &self.database, relative)
    }

    fn refresh_before_search(&mut self) -> Result<SyncStats> {
        let mut full = self.overflow.swap(false, Ordering::AcqRel)
            || self.last_reconciliation.elapsed() >= RECONCILE_INTERVAL;
        let mut changed = BTreeSet::new();
        let mut received = 0;

        loop {
            match self.changes.try_recv() {
                Ok(Ok(event)) => {
                    received += 1;
                    classify_event(&self.project, &event, &mut changed, &mut full);
                    if received >= MAX_QUEUED_EVENTS || changed.len() > MAX_BATCH_PATHS {
                        full = true;
                        break;
                    }
                }
                Ok(Err(_)) => {
                    full = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // A disconnected watcher is no longer trustworthy.
                    return Err("filesystem watcher disconnected; cannot trust index".into());
                }
            }
        }

        if full {
            let stats = findex_index::synchronize(&self.project)?;
            self.last_reconciliation = Instant::now();
            return Ok(stats);
        }
        if changed.is_empty() {
            return Ok(SyncStats::default());
        }

        let paths = changed.into_iter().collect::<Vec<_>>();
        let report = findex_index::refresh_known_paths(&self.project, &paths)?;
        if report.unknown_paths != 0 {
            let stats = findex_index::synchronize(&self.project)?;
            self.last_reconciliation = Instant::now();
            return Ok(stats);
        }
        Ok(report.sync)
    }
}

/// Conservative classification. New files, rename events and root/config
/// changes need full reconciliation because their ignore eligibility may vary.
fn classify_event(root: &Path, event: &Event, changed: &mut BTreeSet<PathBuf>, full: &mut bool) {
    if event.need_rescan() {
        *full = true;
        return;
    }
    // Build outputs and dependency folders are never eligible for indexing.
    // Their create/rename/delete storms must not force O(repository) scans.
    // The .git directory is deliberately *not* ignored: Git state changes
    // require reconciliation even though the scanner never indexes .git.
    if !event.paths.is_empty()
        && event.paths.iter().all(|path| {
            let Ok(relative) = path.strip_prefix(root) else {
                return false;
            };
            relative.components().any(|component| {
                if let Component::Normal(name) = component {
                    !name.eq_ignore_ascii_case(".git") && is_excluded_scan_entry(name)
                } else {
                    false
                }
            })
        })
    {
        return;
    }
    match event.kind {
        EventKind::Access(_) => return,
        EventKind::Modify(ModifyKind::Name(_))
        | EventKind::Create(_)
        | EventKind::Remove(_)
        | EventKind::Any
        | EventKind::Other => {
            *full = true;
            return;
        }
        EventKind::Modify(_) => {}
    }
    if event.paths.is_empty() {
        *full = true;
        return;
    }
    for path in &event.paths {
        let Ok(relative) = path.strip_prefix(root) else {
            *full = true;
            return;
        };
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            *full = true;
            return;
        }
        if relative.components().any(|component| {
            let name = component.as_os_str().to_string_lossy();
            name.eq_ignore_ascii_case(".gitignore")
                || name.eq_ignore_ascii_case(".ignore")
                || name.eq_ignore_ascii_case(".git")
                || name.eq_ignore_ascii_case(".gitattributes")
        }) || path.is_dir()
        {
            *full = true;
            return;
        }
        changed.insert(relative.to_path_buf());
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, DataChange, Flag};

    #[test]
    fn changed_file_is_targeted_but_new_file_forces_reconciliation() {
        let root = Path::new(r"C:\source");
        let mut changed = BTreeSet::new();
        let mut full = false;
        let edit = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Any)))
            .add_path(root.join("src").join("main.rs"));
        classify_event(root, &edit, &mut changed, &mut full);
        assert!(!full);
        assert!(changed.contains(&PathBuf::from("src").join("main.rs")));

        let created = Event::new(EventKind::Create(CreateKind::File))
            .add_path(root.join("src").join("second.rs"));
        classify_event(root, &created, &mut changed, &mut full);
        assert!(full);
    }

    #[test]
    fn event_loss_ignore_rule_and_outside_path_are_conservative() {
        let root = Path::new(r"C:\source");
        for event in [
            Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.join(".gitignore")),
            Event::new(EventKind::Modify(ModifyKind::Any))
                .add_path(PathBuf::from(r"C:\outside\secret.rs")),
            Event::new(EventKind::Modify(ModifyKind::Any)).set_flag(Flag::Rescan),
        ] {
            let mut changed = BTreeSet::new();
            let mut full = false;
            classify_event(root, &event, &mut changed, &mut full);
            assert!(full);
        }
    }

    #[test]
    fn read_access_does_not_trigger_index_writes() {
        let root = Path::new(r"C:\source");
        let event = Event::new(EventKind::Access(AccessKind::Any)).add_path(root.join("readme.rs"));
        let mut changed = BTreeSet::new();
        let mut full = false;
        classify_event(root, &event, &mut changed, &mut full);
        assert!(changed.is_empty());
        assert!(!full);
    }

    #[test]
    fn persistent_symbol_lookup_skips_warm_full_reconciliation() {
        let project = tempfile::tempdir().expect("workspace");
        let path = project.path().join("app.rs");
        std::fs::write(&path, "fn stable_symbol() {}").expect("fixture");
        let mut backend = SearchBackend::new(project.path());
        let first = backend.search_symbols("stable_symbol", 8).expect("first");
        assert_eq!(first.hits.len(), 1);
        let second = backend.search_symbols("stable_symbol", 8).expect("second");
        assert_eq!(second.hits.len(), 1);
        if matches!(backend, SearchBackend::Watched(_)) {
            assert_eq!(second.sync.examined, 0);
            assert_eq!(second.retrieval, "sqlite-symbol-exact-hash-native-watch");
        }
    }

    #[test]
    fn injected_change_updates_symbol_names_and_respects_admission() {
        let project = tempfile::tempdir().expect("workspace");
        let path = project.path().join("app.rs");
        std::fs::write(project.path().join(".gitignore"), "ignored.rs\n").expect("ignore");
        std::fs::write(project.path().join("ignored.rs"), "fn rejected() {}").expect("ignored");
        std::fs::write(&path, "fn before() {}").expect("initial");
        let mut watched = WatchedIndex::new(project.path()).expect("native watcher");
        assert!(watched
            .inspect_file(Path::new("ignored.rs"))
            .expect("ignore")
            .is_none());

        std::fs::write(&path, "fn after() {}").expect("edit");
        let (sender, receiver) = mpsc::sync_channel(8);
        sender
            .send(Ok(Event::new(EventKind::Modify(ModifyKind::Data(
                DataChange::Any,
            )))
            .add_path(
                project.path().canonicalize().expect("root").join("app.rs"),
            )))
            .expect("injected event");
        watched.changes = receiver;
        let updated = watched.search_symbols("after", 8).expect("updated");
        assert_eq!(updated.hits.len(), 1);
        assert_eq!(updated.sync.indexed, 1);
        assert_eq!(updated.sync.symbols_parsed, 1);
        assert!(watched
            .search_symbols("before", 8)
            .expect("old")
            .hits
            .is_empty());
    }

    #[test]
    fn disconnected_symbol_watcher_switches_to_scanning() {
        let project = tempfile::tempdir().expect("workspace");
        std::fs::write(project.path().join("app.rs"), "fn fallback_symbol() {}").expect("write");
        let watched = WatchedIndex::new(project.path()).expect("watch");
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(sender);
        let mut watched = watched;
        watched.changes = receiver;
        let mut backend = SearchBackend::Watched(Box::new(watched));
        let result = backend
            .search_symbols("fallback_symbol", 8)
            .expect("recovered");
        assert_eq!(result.hits.len(), 1);
        assert!(matches!(backend, SearchBackend::Scanning(_)));
    }

    #[test]
    fn late_watcher_disconnection_falls_back_to_verified_full_scan() {
        let workspace = tempfile::tempdir().expect("workspace");
        std::fs::write(workspace.path().join("app.rs"), "fn fallback_marker() {}").expect("write");
        let mut watched = WatchedIndex::new(workspace.path()).expect("watcher");
        let (sender, disconnected_receiver) = mpsc::sync_channel(1);
        drop(sender);
        watched.changes = disconnected_receiver;

        let mut backend = SearchBackend::Watched(Box::new(watched));
        let found = backend.search("fallback_marker", 5).expect("safe fallback");
        assert_eq!(found.hits.len(), 1);
        assert!(matches!(backend, SearchBackend::Scanning(_)));
    }

    #[test]
    fn ignored_build_churn_does_not_force_full_scan() {
        let root = Path::new(r"C:\source");
        for folder in ["target", "node_modules", ".venv", "dist"] {
            let event = Event::new(EventKind::Create(CreateKind::File))
                .add_path(root.join(folder).join("generated.bin"));
            let mut changed = BTreeSet::new();
            let mut full = false;
            classify_event(root, &event, &mut changed, &mut full);
            assert!(!full, "{folder}");
            assert!(changed.is_empty());
        }
        let git_event = Event::new(EventKind::Create(CreateKind::File))
            .add_path(root.join(".git").join("index"));
        let mut changed = BTreeSet::new();
        let mut full = false;
        classify_event(root, &git_event, &mut changed, &mut full);
        assert!(full);
    }

    #[test]
    fn watcher_detects_changes_without_trusting_unverified_snippets() {
        let project = tempfile::tempdir().expect("workspace");
        let file = project.path().join("app.rs");
        std::fs::write(&file, "fn before_marker() {}").expect("write initial");
        let mut watched = WatchedIndex::new(project.path()).expect("watcher");
        assert_eq!(
            watched
                .search("before_marker", 5)
                .expect("search")
                .hits
                .len(),
            1
        );

        std::fs::write(&file, "fn after_marker() {}").expect("edit");
        // Inject the same native watcher event deterministically: no sleeps or
        // timing assumptions about the operating system's event delivery.
        let (tx, rx) = mpsc::sync_channel(8);
        tx.send(Ok(Event::new(EventKind::Modify(ModifyKind::Data(
            DataChange::Any,
        )))
        .add_path(
            project.path().canonicalize().expect("root").join("app.rs"),
        )))
        .expect("enqueue");
        watched.changes = rx;
        assert_eq!(
            watched
                .search("after_marker", 5)
                .expect("search")
                .hits
                .len(),
            1
        );
        assert!(watched
            .search("before_marker", 5)
            .expect("search")
            .hits
            .is_empty());
    }
}
