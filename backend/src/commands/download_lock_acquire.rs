//! Taking the cross-instance lock: open, wait, and check that what is
//! held is still the file at the lock's name.

use super::on_blocking_pool;
use super::os;
use super::release::{sweep_once, InstanceLock};
use crate::error::{AniError, Result};

/// How often a download waiting on another instance re-tries the OS
/// lock. Long enough that a queued download is not spinning, short
/// enough that the handover is not what the user notices.
const INSTANCE_LOCK_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// How many times an attempt may lose the lock file to a concurrent
/// removal — an open that found the directory or the file mid-delete,
/// or a lock taken on a file that lost its name meanwhile — before the
/// lock is given up as unobtainable. Each loss is one remover winning a
/// race, so a real contention ends in a handful; the cap is for a
/// volume that answers every open with a refusal that only looks like
/// a removal (Windows reports a delete in progress and a plain denial
/// with the same code), which would otherwise poll to the deadline.
/// Giving up is the lock's ordinary best-effort failure.
const INSTANCE_LOCK_LOSSES: u32 = 40;

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
/// Only the filesystem calls run on the blocking pool — the sweep, the
/// open and the check that the locked file is still the one at the
/// path — where none of them can park indefinitely.
///
/// Once locked, the handle is checked against the path, and an
/// attempt whose file was removed while it waited opens again: see
/// [`InstanceLock`] for why removal makes that necessary.
///
/// The first acquisition in a lock directory sweeps it (see
/// [`sweep_once`]).
///
/// # Errors
/// [`AniError::Timeout`] when the deadline passes with another
/// instance still holding it — the transfer's own ceiling, since a
/// download blocked on another instance is spending the wait the
/// transfer was given. [`AniError::Io`] when the lock file cannot be
/// created or opened at all, or keeps being removed from under the
/// attempt; see [`super::target_lock_path`] for why that is worth
/// refusing over.
pub(crate) async fn acquire_instance_lock(
    target: &std::path::Path,
    deadline: tokio::time::Instant,
) -> Result<InstanceLock> {
    let path = super::target_lock_path(target).ok_or(AniError::Io)?;
    sweep_once(&path).await;
    let mut losses = 0;
    loop {
        if let Some(file) = open_lock_file(&path, &mut losses, deadline).await? {
            wait_for_lock(&file, deadline).await?;
            let (file, kept) = on_blocking_pool(path.clone(), move |p| {
                let kept = os::still_named(&file, p);
                (file, kept)
            })
            .await?;
            if kept {
                return Ok(InstanceLock::new(file, path));
            }
            // Removed while this waited: what it holds has no name.
            note_loss(&mut losses, deadline).await?;
        }
    }
}

/// Open (creating) the lock file, or `None` when a concurrent removal
/// got in the way and the caller should try again.
async fn open_lock_file(
    path: &std::path::Path,
    losses: &mut u32,
    deadline: tokio::time::Instant,
) -> Result<Option<std::fs::File>> {
    match on_blocking_pool(path.to_path_buf(), os::open_creating).await? {
        Ok(file) => Ok(Some(file)),
        Err(e) if os::lost_to_removal(&e) => {
            note_loss(losses, deadline).await?;
            Ok(None)
        }
        Err(_) => Err(AniError::Io),
    }
}

/// Count one loss to a concurrent removal and wait a poll before the
/// next attempt — or give up, past the cap or the deadline.
async fn note_loss(losses: &mut u32, deadline: tokio::time::Instant) -> Result<()> {
    *losses += 1;
    if *losses > INSTANCE_LOCK_LOSSES {
        return Err(AniError::Io);
    }
    poll_until(deadline).await
}

/// Poll the OS lock on `file` until it is granted or `deadline` passes.
async fn wait_for_lock(file: &std::fs::File, deadline: tokio::time::Instant) -> Result<()> {
    loop {
        match fs4::FileExt::try_lock(file) {
            Ok(()) => return Ok(()),
            Err(fs4::TryLockError::WouldBlock) => poll_until(deadline).await?,
            Err(fs4::TryLockError::Error(_)) => return Err(AniError::Io),
        }
    }
}

/// Sleep one poll interval, clipped to `deadline`; [`AniError::Timeout`]
/// once it has passed.
async fn poll_until(deadline: tokio::time::Instant) -> Result<()> {
    let now = tokio::time::Instant::now();
    if now >= deadline {
        return Err(AniError::Timeout);
    }
    tokio::time::sleep_until(deadline.min(now + INSTANCE_LOCK_POLL)).await;
    Ok(())
}
