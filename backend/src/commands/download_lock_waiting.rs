//! A lock file opened but not locked, and how it is given up.

use super::os;

/// An open lock file not (yet) locked: a try that found another holder,
/// carried to the next try. Dropped — a cancel while waiting, or a try
/// that failed — it gives the file up the way the platform allows.
///
/// On Unix that is only closing it: removing a name without the lock is
/// exactly what the discipline forbids, and the holder removes it under
/// its lock anyway. On Windows it is closing and then trying the
/// delete, because this handle may be what made the holder's own
/// delete fail — the holder leaves the file to whoever still has it
/// open, and a waiter cancelled before its next try would otherwise
/// leave it to nobody. The delete is as safe there as a release's: it
/// succeeds only once no handle at all is open on the file.
#[derive(Debug)]
pub(super) struct Waiting {
    file: Option<std::fs::File>,
    path: std::path::PathBuf,
}

impl Waiting {
    pub(super) fn new(file: std::fs::File, path: &std::path::Path) -> Self {
        Self {
            file: Some(file),
            path: path.to_path_buf(),
        }
    }

    /// The handle, to try the lock on. Until the caller wraps it again,
    /// nothing gives it up on its behalf.
    pub(super) fn file(&self) -> Option<&std::fs::File> {
        self.file.as_ref()
    }

    /// Lock taken: the handle becomes the held lock.
    pub(super) fn into_lock(mut self) -> Option<super::release::InstanceLock> {
        let file = self.file.take()?;
        Some(super::release::InstanceLock::new(
            file,
            std::mem::take(&mut self.path),
        ))
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            os::abandon(file, &self.path);
            if let Some(dir) = self.path.parent() {
                super::release::remove_dir_if_empty(dir);
            }
        }
    }
}
