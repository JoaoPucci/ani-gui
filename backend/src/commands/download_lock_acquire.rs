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
/// Only the filesystem calls run on the blocking pool — the sweep, and
/// each try at the lock: the open, the lock's non-blocking try and the
/// check that the locked file is still the one at the path. None of
/// them waits for another holder, which is what would park a thread.
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
    let mut busy = None;
    loop {
        let open = busy.take();
        match on_blocking_pool(path.clone(), move |p| attempt(p, open)).await? {
            Attempt::Held(lock) => return Ok(lock),
            Attempt::Busy(file) => {
                busy = Some(file);
                poll_until(deadline).await?;
            }
            Attempt::Lost => note_loss(&mut losses, deadline).await?,
            Attempt::Failed => return Err(AniError::Io),
        }
    }
}

/// One try at the lock, run whole on the blocking pool.
enum Attempt {
    /// Locked, and still the file at the lock's name.
    Held(InstanceLock),
    /// Another holder has it; the handle to try again with.
    Busy(std::fs::File),
    /// A concurrent removal got in the way: try again from the open.
    Lost,
    /// The lock file cannot be opened or locked at all.
    Failed,
}

/// Open the lock file (unless `open` is a handle from a busy try),
/// try its lock, and check what is locked is still at the path.
///
/// Whole on the blocking pool, so a cancel cannot strand what it took.
/// A cancelled acquisition drops the join handle, not the call, and
/// the call's answer is then dropped with nobody to read it. A lock
/// taken here is already an [`InstanceLock`] by then, so dropping it
/// releases and removes the file as any holder's drop does; a lock
/// handed back as a bare handle to be wrapped on the async side would
/// be closed without either.
fn attempt(path: &std::path::Path, open: Option<std::fs::File>) -> Attempt {
    let file = match open.map_or_else(|| os::open_creating(path), Ok) {
        Ok(file) => file,
        Err(e) if os::lost_to_removal(&e) => return Attempt::Lost,
        Err(_) => return Attempt::Failed,
    };
    match fs4::FileExt::try_lock(&file) {
        Ok(()) => {}
        Err(fs4::TryLockError::WouldBlock) => return Attempt::Busy(file),
        Err(fs4::TryLockError::Error(_)) => return Attempt::Failed,
    }
    let lock = InstanceLock::new(file, path.to_path_buf());
    // Removed while this waited: what it holds has no name, and
    // dropping it removes nothing, since the release checks the same.
    if lock.is_at_path() {
        Attempt::Held(lock)
    } else {
        Attempt::Lost
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
