//! The lock a download takes on its destination file, held for the
//! whole spawn, within the app and across instances.

/// One lock per destination file, held for the whole spawn.
///
/// Two downloads of the same episode resolve to the same path — the
/// dock allows the second click — and before this they ran at once,
/// each writing the other's output. The repackage retry sharpened it
/// into deletion: that path removes the target and passes `-y`, so a
/// late arrival can replace a file the first run already finished.
///
/// Keyed by the target's verbatim lock path rather than by show, so
/// a range download's episodes, which differ, still run as the loop
/// schedules them — see `target_lock` for why the key carries the
/// path as given instead of a folded spelling of it. Entries are
/// kept rather than reaped: one `Arc<Mutex>` per file a session has
/// downloaded is a few dozen bytes, and reaping introduces the race
/// this exists to remove — a waiter holding an `Arc` the map has
/// already dropped locks a mutex nobody else can see.
pub(super) static TARGET_LOCKS: std::sync::Mutex<
    Option<std::collections::HashMap<std::path::PathBuf, std::sync::Arc<tokio::sync::Mutex<()>>>>,
> = std::sync::Mutex::new(None);

/// Where the lock that crosses app instances lives for `target`.
///
/// The map above is per-process, so a second copy of the app shares
/// none of it. What both instances do share is the filesystem, and a
/// lock file named the same way by both is enough for the kernel to
/// order them — including when one is killed mid-download, since it
/// releases the lock on the dying process's behalf.
///
/// Placed beside the target, in a `.ani-gui-locks/` directory next to
/// the file, and named after the file itself.
///
/// The location cannot depend on anything a process is configured
/// with. Two earlier attempts did: the app cache directory moves with
/// the dev profile, and the profile-independent one still resolved
/// under `$XDG_CACHE_HOME`, which any launcher can set. The
/// destination is what both contenders provably agree on — they were
/// handed the same one, or they would not be racing — and it is
/// writable by construction, since a video is about to land in it,
/// which is what keeps refusing on a lock failure reasonable.
///
/// The *name* carries the target's own, verbatim, and that is the
/// part worth explaining, because it replaced a digest of a
/// case-folded key. Whether two names are one file is the
/// filesystem's question, and carrying the name into the lock
/// directory asks it there: two spellings the volume calls one file
/// collide on one lock file, under the same case rules, canonical
/// normalization and trailing-dot handling that made them one
/// target. Hashing a folded key substituted a case table for that,
/// and the table was wrong three times — first on case, then on
/// final sigma, then on normalization — each found separately
/// because a table can only be wrong one entry at a time.
///
/// Delegating also gets the case-sensitive filesystems right, which
/// folding could not: there `Show.mp4` and `show.mp4` really are two
/// files, and now they take two locks instead of queueing behind one.
///
/// The target's own name, in a sibling directory.
///
/// This is no longer what makes concurrent downloads safe — publishing
/// is, by asking the filesystem for the name rather than reproducing
/// its identity anywhere. The lock is an optimization: it stops two
/// clicks on one episode both spending the bandwidth, and it is
/// allowed to be wrong. Where it is fooled, the second transfer runs
/// and then finds the name taken, which costs a download and loses
/// nothing.
///
/// That reframing is why the naming can stay simple. It carries the
/// target's name verbatim so the filesystem's own answers about case,
/// normalization and the rest apply for free; where they do not —
/// NTFS 8.3 aliases are assigned per directory, so a target and its
/// lock can alias differently — the consequence is now a wasted
/// transfer rather than a deleted file.
///
/// The cost the user sees is a directory in their download folder
/// while a download runs, and nothing after: the lock file goes when
/// its download ends and the directory when the last one does (see
/// [`release::InstanceLock`]). It is dot-prefixed, which hides it on Linux and
/// macOS, and carries the hidden attribute on Windows, where a dot
/// hides nothing.
pub(crate) fn target_lock_path(target: &std::path::Path) -> Option<std::path::PathBuf> {
    let parent = target.parent()?;
    Some(parent.join(".ani-gui-locks").join(target.file_name()?))
}

// The lock file's life on disk — taking it, letting it go, and what
// differs per platform — in files of their own, so each stays small
// enough to read whole.
#[path = "download_lock_acquire.rs"]
mod acquire;
#[path = "download_lock_os.rs"]
mod os;
#[path = "download_lock_release.rs"]
mod release;
pub(crate) use acquire::acquire_instance_lock;

/// Run a filesystem call on `path` on the blocking pool.
async fn on_blocking_pool<T, F>(path: std::path::PathBuf, f: F) -> crate::error::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&std::path::Path) -> T + Send + 'static,
{
    tokio::task::spawn_blocking(move || f(&path))
        .await
        .map_err(|_| crate::error::AniError::Io)
}

pub(crate) fn target_lock(target: &std::path::Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    // Keyed by the lock path, so this and the file lock share one
    // notion of identity rather than two that can disagree. It is the
    // weaker of the pair: two names a case-insensitive filesystem
    // calls one file get two entries here and meet at the file lock
    // instead, which is exact. Guessing at that equivalence again to
    // catch them a layer earlier is what this just stopped doing.
    let key = target_lock_path(target).unwrap_or_else(|| target.to_path_buf());
    let mut guard = TARGET_LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .get_or_insert_with(std::collections::HashMap::new)
        .entry(key)
        .or_default()
        .clone()
}
