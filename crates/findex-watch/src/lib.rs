//! Windows native filesystem-event index, used only by the long-lived MCP process.
//! Events are hints, never a source of truth. Overflow and uncertain events
//! trigger full reconciliation; retrieved code is still verified by its hash.

use findex_core::storage_dir;
use findex_index::{SearchReport, SyncStats};
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

    pub fn search(&mut self, query: &str, limit: usize) -> Result<SearchReport> {
        match self {
            Self::Watched(index) => index.search(query, limit),
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
