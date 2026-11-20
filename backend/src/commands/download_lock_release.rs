//! Letting the cross-instance lock go: removing the lock file, the
//! emptied directory, and leftovers nothing removed in time.

use super::os;
use super::waiting::Waiting;

/// The cross-instance lock, held: the open lock file, removed from
/// disk when this is dropped.
///
/// Dropping is how every download ends — returning with the episode,
/// returning early with an error, being cancelled (the future is
/// dropped), and a quit's runtime teardown dropping the task — so all
/// of them tidy up. A process that dies without unwinding (a crash, a
/// kill, and every quit on Windows, which ends the backend with
/// `taskkill /F`) leaves its file, which [`sweep_once`] collects
/// later.
///
/// Removing a lock file is the classic way to break a file lock: a
/// waiter that opened the file before it was removed is then granted
/// a lock on a file nobody else can reach, and the next arrival
/// creates a fresh one and locks that too. Each platform closes it
/// with its own standard answer:
///
/// - Unix removes the name while still holding the lock, and every
///   acquisition checks, once locked, that its handle is still the
///   file at the path (same device and inode); a waiter that lost
///   that race drops its handle and opens again. This is the
///   delete-under-lock discipline long-running daemons use for pid
///   and lock files.
/// - Windows opens the file without sharing delete access, so the file
///   can be deleted only once no handle is open on it at all; a
///   waiter's open handle is what keeps it in place. The holder
///   releases first and deletes after, and a delete that finds anyone
///   else's handle simply fails and leaves the file to them. Python's
///   `filelock` relies on the same share-mode rule for its Windows
///   lock. It is also why identity is not compared there: the
///   standard library does not expose a file's index on stable Rust,
///   and the crate forbids the unsafe call that reads it — but a file
///   that cannot be deleted while open needs no check that it was.
///
/// An instance of an earlier version takes part in none of this: it
/// opens the file allowing deletion and never checks what it locked,
/// so a removal here can leave it holding a nameless file while a new
/// instance locks a fresh one. The lock is best-effort, and what that
/// costs is a duplicate transfer, which publication refuses to install.
///
/// The work is done inline rather than on the blocking pool. A drop
/// during runtime teardown is exactly when blocking tasks no longer
/// start, and a removal and a directory removal are no heavier than
/// the open that took the lock.
#[derive(Debug)]
pub(crate) struct InstanceLock {
    file: Option<std::fs::File>,
    path: std::path::PathBuf,
}

impl InstanceLock {
    /// The lock held on `file`, the lock file named `path`.
    pub(super) fn new(file: std::fs::File, path: std::path::PathBuf) -> Self {
        Self {
            file: Some(file),
            path,
        }
    }
}

impl InstanceLock {
    /// Whether the locked file is still the one at the lock's name.
    pub(super) fn is_at_path(&self) -> bool {
        self.file
            .as_ref()
            .is_some_and(|file| os::still_named(file, &self.path))
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            os::release(file, &self.path);
            if let Some(dir) = self.path.parent() {
                remove_dir_if_empty(dir);
            }
        }
    }
}

/// Remove the lock directory once nothing is left in it. Racing
/// another instance is harmless either way: a directory with a file
/// in it refuses, one already gone is not found, and an instance whose
/// open loses to the removal opens again.
pub(super) fn remove_dir_if_empty(dir: &std::path::Path) {
    let _ = std::fs::remove_dir(dir);
}

/// Lock directories this run has already swept.
static SWEPT: std::sync::Mutex<Option<std::collections::HashSet<std::path::PathBuf>>> =
    std::sync::Mutex::new(None);

/// Sweep the directory holding `lock_path`, the first time this run
/// takes a lock there.
///
/// What it removes is what nothing removed in time: files earlier
/// versions left behind, one per episode they ever attempted, and the
/// file of any process that died holding its lock. Once per directory
/// per run, since everything this version creates goes with its
/// download — and before the acquisition's own open, so a leftover at
/// the episode's own name is replaced by a fresh file rather than
/// reused.
pub(super) async fn sweep_once(lock_path: &std::path::Path) {
    let Some(dir) = lock_path.parent().map(std::path::Path::to_path_buf) else {
        return;
    };
    let first = {
        let mut swept = SWEPT.lock().unwrap_or_else(|e| e.into_inner());
        swept
            .get_or_insert_with(std::collections::HashSet::new)
            .insert(dir.clone())
    };
    if first {
        let own = lock_path.to_path_buf();
        let _ = super::on_blocking_pool(dir, move |d| sweep(d, &own)).await;
    }
}

/// Remove every lock file in `dir` that nobody holds.
///
/// Each is opened without creating it, locked without waiting, and
/// released exactly as a holder releases its own — so a file another
/// instance holds refuses the lock and is kept, and one a waiter has
/// open is handled by the same discipline that protects a release.
/// Anything that cannot be opened or locked is left alone: a leftover
/// costs an empty file, a wrong removal a lock.
/// Lock files of this process's own downloads are skipped, however the
/// lock would answer — except `own`, the one about to be taken, whose
/// leftover is better replaced. Where locks belong to the process
/// rather than to the open file (NFS emulates `flock` with POSIX
/// record locks), a second handle in this process would be granted a
/// running download's lock, remove its file and, on closing, release
/// its lock. Checked per entry, which narrows that to a download that
/// registers and locks in the instant between the check and the
/// sweep's own try — not closed, since closing the sweep's handle
/// would release a process-wide lock anyway. The cost there is the
/// lock's ordinary failure, a duplicate transfer.
fn sweep(dir: &std::path::Path, own: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if path != own && super::in_use_here(&path) {
            continue;
        }
        let Ok(file) = os::lock_file_options().open(&path) else {
            continue;
        };
        // Either way out gives the file up as a holder or a waiter
        // would: released and removed if the lock was free, closed (and
        // on Windows the delete retried) if not — so the sweep's own
        // brief handle never strands a file its holder failed to remove
        // because of it.
        let waiting = Waiting::new(file, &path);
        if waiting
            .file()
            .is_some_and(|f| fs4::FileExt::try_lock(f).is_ok())
        {
            drop(waiting.into_lock());
        }
    }
}
