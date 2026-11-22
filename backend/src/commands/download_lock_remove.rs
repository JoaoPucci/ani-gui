//! Deleting a lock file: the retry a platform asks for, and a record of
//! a delete that did not happen.

use super::os;

/// How many times a lock file's delete is tried, and how long apart,
/// while [`os::delete_refused_for_now`] says the refusal may pass. A
/// waiter's handle refuses every try, and that case costs nothing but
/// a blocking-pool thread for the window: the waiter removes the file
/// itself when its own download ends. What the retries outlast is an
/// outside handle held for a moment, such as a scanner's or an
/// indexer's on the file just created.
const DELETE_TRIES: u32 = 8;
const DELETE_PAUSE: std::time::Duration = std::time::Duration::from_millis(10);

/// Delete the lock file at `path`, and record a delete that did not
/// happen.
///
/// A refusal that may pass is retried off the caller. A release runs
/// where its download's task was dropped, on a runtime worker the
/// playback proxy shares, and sleeping there between tries would stall
/// it. The retries go to the blocking pool, and the lock directory is
/// removed after them, since it cannot go while the file is there.
/// Without a runtime, they run in place. During runtime teardown the
/// blocking pool takes no new work and the retries are dropped; that
/// costs a leftover the next run's sweep removes, and it does not arise
/// in practice — Unix never retries, and a Windows quit ends the
/// backend outright rather than tearing it down. A retry deletes only
/// what no handle of this app has open, since every one of them
/// refuses it, so a late one cannot take a file a later holder of this
/// version created; one of an earlier version, which shares delete, is
/// exposed here as everywhere else (see
/// [`super::release::InstanceLock`]).
pub(crate) fn remove_lock_file(path: &std::path::Path) {
    remove_lock_file_with(path, os::delete_refused_for_now);
}

/// [`remove_lock_file`], with the platform's answer to "may this
/// refusal pass" given — so the retries can be exercised where the
/// platform never asks for one.
pub(crate) fn remove_lock_file_with(path: &std::path::Path, passing: Refusal) {
    match std::fs::remove_file(path) {
        Err(e) if passing(&e) => retry_off_caller(path.to_path_buf(), passing),
        done => note_removal(path, done),
    }
}

/// Whether a refused delete may succeed if tried again shortly.
pub(crate) type Refusal = fn(&std::io::Error) -> bool;

fn retry_off_caller(path: std::path::PathBuf, passing: Refusal) {
    let retry = move || {
        retry_delete(&path, passing);
        if let Some(dir) = path.parent() {
            super::release::remove_dir_if_empty(dir);
        }
    };
    match tokio::runtime::Handle::try_current() {
        Ok(runtime) => drop(runtime.spawn_blocking(retry)),
        Err(_) => retry(),
    }
}

/// The tries after the first refusal, each after a pause.
fn retry_delete(path: &std::path::Path, passing: Refusal) {
    let mut removed = Ok(());
    for _ in 1..DELETE_TRIES {
        std::thread::sleep(DELETE_PAUSE);
        removed = std::fs::remove_file(path);
        if !removed.as_ref().is_err_and(passing) {
            break;
        }
    }
    note_removal(path, removed);
}

/// Record a lock file's delete that did not happen. Not found means
/// somebody else already removed it, which is the protocol working.
/// Anything else leaves the file in place: to this app's waiter, which
/// removes it; to an outside handle's owner, which does not, and then
/// the next run's sweep does; or, where nothing has it open and the
/// delete itself is refused (permissions, a read-only volume), for good
/// — the sweep's delete fails the same way. The log says which file
/// and why.
fn note_removal(path: &std::path::Path, removed: std::io::Result<()>) {
    match removed {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => tracing::debug!(
            path = %path.display(),
            error = %e,
            code = ?e.raw_os_error(),
            "download lock or hold file left in place"
        ),
        _ => {}
    }
}
