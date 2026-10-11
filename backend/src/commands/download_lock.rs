//! The lock a download takes on its destination file, held for the
//! whole spawn, within the app and across instances.

use crate::error::{AniError, Result};

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
/// The cost the user sees is a directory in their download folder. It
/// is dot-prefixed, so hidden on Linux and macOS and visible on
/// Windows.
pub(crate) fn target_lock_path(target: &std::path::Path) -> Option<std::path::PathBuf> {
    let parent = target.parent()?;
    Some(parent.join(".ani-gui-locks").join(target.file_name()?))
}

/// How often a download waiting on another instance re-tries the OS
/// lock. Long enough that a queued download is not spinning, short
/// enough that the handover is not what the user notices.
pub(super) const INSTANCE_LOCK_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// Take the lock that crosses app instances for `target`, waiting for
/// a holder until `deadline`.
///
/// Polled rather than blocked. `flock` parks the thread that calls it,
/// and a parked thread is not a future: cancelling the download drops
/// the `JoinHandle` while the closure stays inside the call, so the
/// dock reports a cancel that left a blocking-pool thread behind. Do
/// that a few times and the pool starves every other blocking caller,
/// invisibly. Waiting between polls on the async side means a
/// cancelled download lands on an await point and is simply dropped.
///
/// Only the open runs on the blocking pool, where it cannot park
/// indefinitely — it either finds the file or fails.
///
/// # Errors
/// [`AniError::Timeout`] when the deadline passes with another
/// instance still holding it — the transfer's own ceiling, since a
/// download blocked on another instance is spending the wait the
/// transfer was given. [`AniError::Io`] when the lock file cannot be
/// created or opened at all; see [`target_lock_path`] for why that is
/// worth refusing over.
pub(crate) async fn acquire_instance_lock(
    target: &std::path::Path,
    deadline: tokio::time::Instant,
) -> Result<std::fs::File> {
    let path = target_lock_path(target).ok_or(AniError::Io)?;
    let file = tokio::task::spawn_blocking(move || -> Result<std::fs::File> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| AniError::Io)?;
        }
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|_| AniError::Io)
    })
    .await
    .map_err(|_| AniError::Io)??;
    loop {
        match fs4::FileExt::try_lock(&file) {
            Ok(()) => return Ok(file),
            Err(fs4::TryLockError::WouldBlock) => {}
            Err(fs4::TryLockError::Error(_)) => return Err(AniError::Io),
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(AniError::Timeout);
        }
        tokio::time::sleep_until(deadline.min(now + INSTANCE_LOCK_POLL)).await;
    }
}

pub(super) fn target_lock(target: &std::path::Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
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
