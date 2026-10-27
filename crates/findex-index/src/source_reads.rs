//! Bounded native file I/O; SQLite writes and result ordering stay on the caller.
use findex_core::MAX_SOURCE_BYTES;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

type ReadResult = io::Result<Vec<u8>>;
type Job = (usize, PathBuf);

pub(super) fn read_one(path: &Path) -> ReadResult {
    // Recheck immediately before opening: queued paths can change after walking.
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file()
        || findex_core::is_cloud_only(&metadata)
        || metadata.len() > MAX_SOURCE_BYTES
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "source no longer eligible",
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize + 1);
    std::fs::File::open(path)?
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

struct Pool {
    jobs: Vec<SyncSender<Job>>,
    results: Receiver<(usize, ReadResult)>,
    workers: Vec<JoinHandle<()>>,
}

impl Pool {
    fn new(width: usize) -> io::Result<Self> {
        let (results_tx, results) = mpsc::sync_channel(width);
        let mut pool = Self {
            jobs: Vec::new(),
            results,
            workers: Vec::new(),
        };
        for number in 0..width {
            let (jobs, receiver) = mpsc::sync_channel::<Job>(1);
            let results = results_tx.clone();
            let worker = thread::Builder::new()
                .name(format!("findex-read-{number}"))
                .spawn(move || {
                    while let Ok((slot, path)) = receiver.recv() {
                        if results.send((slot, read_one(&path))).is_err() {
                            break;
                        }
                    }
                })?;
            pool.jobs.push(jobs);
            pool.workers.push(worker);
        }
        Ok(pool)
    }

    fn read(&self, paths: &[PathBuf]) -> io::Result<Vec<ReadResult>> {
        for (slot, path) in paths.iter().enumerate() {
            self.jobs[slot]
                .send((slot, path.clone()))
                .map_err(|_| io::Error::other("source reader disconnected"))?;
        }
        let mut ordered: Vec<Option<ReadResult>> =
            std::iter::repeat_with(|| None).take(paths.len()).collect();
        for _ in paths {
            let (slot, result) = self
                .results
                .recv()
                .map_err(|_| io::Error::other("source reader disconnected"))?;
            ordered[slot] = Some(result);
        }
        ordered
            .into_iter()
            .map(|result| result.ok_or_else(|| io::Error::other("source read missing")))
            .collect()
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.jobs.clear();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

pub(super) struct SourceReads {
    width: usize,
    pool: Option<Pool>,
    attempted: bool,
}

impl SourceReads {
    pub(super) fn new(width: usize) -> Self {
        Self {
            width: width.clamp(1, 8),
            pool: None,
            attempted: false,
        }
    }

    pub(super) fn capacity(&self) -> usize {
        self.width
    }

    pub(super) fn read(&mut self, paths: &[PathBuf]) -> io::Result<Vec<ReadResult>> {
        if paths.len() > self.width {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "source batch exceeds reader capacity",
            ));
        }
        if paths.len() > 1 && !self.attempted {
            self.attempted = true;
            // Resource pressure must not make a readable project unindexable.
            self.pool = Pool::new(self.width).ok();
        }
        match &self.pool {
            Some(pool) => pool.read(paths),
            None => Ok(paths.iter().map(|path| read_one(path)).collect()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn batches_keep_order_errors_and_memory_bounds_across_reuse() {
        let root = tempfile::tempdir().expect("fixture");
        let first = root.path().join("first.rs");
        let large = root.path().join("large.rs");
        let missing = root.path().join("missing.rs");
        std::fs::write(&first, "first").expect("source");
        std::fs::write(&large, vec![b'x'; MAX_SOURCE_BYTES as usize]).expect("large");
        let mut reader = SourceReads::new(2);
        let batch = reader.read(&[large, first.clone()]).expect("batch");
        assert_eq!(
            batch[0].as_ref().expect("bounded large").len(),
            MAX_SOURCE_BYTES as usize
        );
        assert_eq!(batch[1].as_ref().expect("first"), b"first");
        let batch = reader
            .read(&[missing, first.clone()])
            .expect("reused batch");
        assert_eq!(
            batch[0].as_ref().expect_err("missing").kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(batch[1].as_ref().expect("first"), b"first");
        std::fs::write(&first, vec![b'x'; MAX_SOURCE_BYTES as usize + 1]).expect("oversized");
        assert_eq!(
            read_one(&first).expect_err("oversized").kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(reader
            .read(&[first.clone(), first.clone(), first.clone()])
            .is_err());
        std::fs::write(&first, "first").expect("restore");
        let mut serial = SourceReads::new(2);
        serial.attempted = true;
        let fallback = serial
            .read(&[first.clone(), first])
            .expect("serial fallback");
        assert!(serial.pool.is_none());
        assert_eq!(fallback[0].as_ref().expect("fallback source"), b"first");
    }
}
