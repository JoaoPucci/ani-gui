//! Opening the lock file — making its directory against a release that
//! is removing it — and what a refused try logs. Apart from taking the
//! lock, so each stays small enough to read whole.

use super::os;

/// Open the lock file at `path`, creating it and its directory, and
/// hide the directory where a leading dot does not.
pub(super) fn open_creating(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    open_creating_with(path, |dir| std::fs::create_dir_all(dir))
}

/// [`open_creating`], with the directory's creation given — the seam the
/// cases stage a removal racing it through.
pub(super) fn open_creating_with(
    path: &std::path::Path,
    create_dir: impl FnOnce(&std::path::Path) -> std::io::Result<()>,
) -> std::io::Result<std::fs::File> {
    let dir = path.parent().unwrap_or(path);
    match create_dir(dir) {
        Ok(()) => {}
        // Found there and removed before the check that it is a
        // directory: a release emptied it. The open answers for what is
        // there now — a directory made again, or "not found", a loss.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && !obstructed(dir) => {}
        Err(e) => return Err(e),
    }
    let file = os::lock_file_options().create(true).open(path)?;
    os::hide_dir(dir, &file);
    Ok(file)
}

/// Whether something other than a directory has the lock directory's
/// name — a refusal that no retry will change.
fn obstructed(dir: &std::path::Path) -> bool {
    std::fs::symlink_metadata(dir).is_ok_and(|m| !m.is_dir())
}

/// Log why a try refused the lock.
///
/// The caller sees a bare I/O error and runs the download without the
/// lock, so the log is the only place the cause can be read: the step
/// that refused, and the error's kind and the system's own code.
pub(super) fn log_refusal(step: &'static str, path: &std::path::Path, e: &std::io::Error) {
    tracing::warn!(
        path = %path.display(),
        step,
        kind = ?e.kind(),
        // 0 when the error carries none: no system uses it for one.
        code = e.raw_os_error().unwrap_or(0),
        error = %e,
        "download lock refused"
    );
}
