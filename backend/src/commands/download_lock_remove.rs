//! Deleting a lock file: the retry a platform asks for, and a record of
//! a delete that did not happen.

use super::os;

/// How many times a lock file's delete is tried, and how long apart,
/// while [`os::delete_refused_for_now`] says the refusal may pass. A
/// waiter's handle refuses every try, and the cap keeps that case
/// short: the waiter removes the file itself. What the retries outlast
/// is an outside handle held for a moment.
const DELETE_TRIES: u32 = 8;
const DELETE_PAUSE: std::time::Duration = std::time::Duration::from_millis(10);

/// Delete the lock file at `path`, trying again for a moment while the
/// platform says the refusal may pass, and record a delete that did not
/// happen.
pub(crate) fn remove_lock_file(path: &std::path::Path) {
    let mut removed = std::fs::remove_file(path);
    for _ in 1..DELETE_TRIES {
        match &removed {
            Err(e) if os::delete_refused_for_now(e) => std::thread::sleep(DELETE_PAUSE),
            _ => break,
        }
        removed = std::fs::remove_file(path);
    }
    note_removal(path, removed);
}

/// Record a lock file's delete that did not happen. Not found means
/// somebody else already removed it, which is the protocol working;
/// anything else leaves a file for the sweep, and the log says which
/// file and why.
fn note_removal(path: &std::path::Path, removed: std::io::Result<()>) {
    match removed {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => tracing::debug!(
            path = %path.display(),
            error = %e,
            code = ?e.raw_os_error(),
            "download lock file left in place"
        ),
        _ => {}
    }
}
