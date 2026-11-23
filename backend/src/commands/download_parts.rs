//! The working folder a download's unfinished files live in, and the
//! hold each download keeps on its own files there.
//!
//! Every name a transfer writes before it is an episode — the app's
//! scratch, and what yt-dlp derives from it: the `.part`, a
//! `.part-FragN` per fragment in flight, the `.ytdl` resume state —
//! and a subtitle's scratch, is a file in `<destination>/.ani-gui-parts/`.
//! The leading dot hides them on Linux and macOS; on Windows a dot
//! hides nothing, and the folder carries the hidden attribute instead.
//! What a download writes in the folder the user sees is the finished
//! file and nothing else.
//!
//! The folder is a child of the destination rather than a temp
//! directory, because publishing is a rename or a link and both need
//! the same volume: a temp directory on another drive turns the end
//! of every download into a copy of the whole episode.
//!
//! Several downloads share the folder — a range's sidecars beside its
//! transfer, two dock entries, two app instances — so nothing removes
//! a file it cannot prove is its own or dead:
//!
//! - Each download names its files under a key of its own,
//!   `.ani-gui-<pid>-<random>`, and keeps `<key>.hold` open and locked
//!   for as long as it may write. The hold exists before any of its
//!   parts and goes after the last of them it removes; a tool tree
//!   that outlived its teardown can still write one after, and the
//!   next sweep finds it with no hold.
//! - A download that ends removes its key's files, then its hold, then
//!   the folder — which only an empty folder lets go, so another
//!   download's files keep it.
//! - A process that never unwinds — a crash, a kill, every quit on
//!   Windows — leaves its files, and the operating system releases its
//!   lock. The next transfer into the same destination sweeps them:
//!   a key whose hold it can lock, or whose hold is gone, has no
//!   writer. A volume that cannot lock at all keeps everything, since
//!   nothing can prove a hold unheld.

use std::path::{Path, PathBuf};

// A hold is opened, hidden, checked and released exactly as a lock
// file is — on Windows without delete sharing, its removal retried
// past a brief outside handle — so it uses the lock's own layer.
use super::lock::os;

#[path = "download_parts_names.rs"]
mod names;
pub(crate) use names::parts_dir;
use names::KEY_PREFIX;
#[cfg(test)]
pub(crate) use names::{owner_of, Entry, PARTS_DIR};
#[path = "download_parts_sweep.rs"]
mod sweep;

/// A download's claim on its files in the working folder: the locked
/// hold, and the key every one of its names carries. Dropped, it
/// removes them, the hold last, and the folder if nothing else is in
/// it.
#[derive(Debug)]
pub(crate) struct Hold {
    dir: PathBuf,
    key: String,
    file: Option<std::fs::File>,
    /// How many names [`Hold::part`] has given out.
    parts: std::sync::atomic::AtomicU32,
}

/// Where in taking a hold [`Hold::take_with`] lets a case act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// The folder exists; the hold is about to be created in it.
    BeforeCreate,
    /// The hold is created and open; it is about to be locked.
    BeforeLock,
}

/// Take a hold in the working folder beside `target` — the folder of
/// the file a download is for — on the blocking pool: with `sweep`, it
/// can remove episode-sized files a killed run left, on whatever volume
/// the user chose.
///
/// # Errors
/// What [`Hold::take`] reports; a task that did not finish, as an io
/// error of its own.
pub(crate) async fn take_beside(target: &Path, sweep: bool) -> std::io::Result<Hold> {
    let dest = target.parent().map(Path::to_path_buf).unwrap_or_default();
    tokio::task::spawn_blocking(move || Hold::take(&dest, sweep))
        .await
        .map_err(std::io::Error::other)?
}

/// How many times taking a hold starts over when the folder or the
/// hold goes under it: another download removing the folder as it
/// emptied, or a sweep that found the hold before it was locked.
const ATTEMPTS: usize = 8;

impl Hold {
    /// Take a hold in `dest`'s working folder, creating the folder —
    /// hidden on Windows — if it is not there; `dest` itself must be.
    /// With `sweep`, the files of every download that is provably not
    /// running go first.
    ///
    /// # Errors
    /// What creating the folder or the hold reports — the last open's
    /// own error when the folder kept disappearing under it, or an
    /// error of this one's when the attempts ended another way.
    pub(crate) fn take(dest: &Path, sweep: bool) -> std::io::Result<Self> {
        Self::take_with(dest, sweep, &mut |_, _| {})
    }

    /// [`Hold::take`], with `between` called at each point another
    /// download or a sweep can act on the folder or the hold — the seam
    /// the cases stage those races through.
    pub(crate) fn take_with(
        dest: &Path,
        sweep: bool,
        between: &mut dyn FnMut(Step, &Path),
    ) -> std::io::Result<Self> {
        let dir = parts_dir(dest);
        // The open's own error, if the last attempt ended on one: what
        // the caller reports, rather than a cause this guessed at.
        let mut lost = None;
        for _ in 0..ATTEMPTS {
            lost = None;
            // The folder, never the destination: a destination that is
            // gone is the caller's failure to report, not one to paper
            // over by recreating it.
            match std::fs::create_dir(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
            let key = format!(
                "{KEY_PREFIX}{}-{:016x}",
                std::process::id(),
                uuid::Uuid::new_v4().as_u64_pair().0
            );
            let path = dir.join(format!("{key}.hold"));
            between(Step::BeforeCreate, &dir);
            let file = match os::lock_file_options().create_new(true).open(&path) {
                Ok(file) => file,
                // The folder went between creating it and the open:
                // another download removed it as it emptied.
                Err(e) if os::lost_to_removal(&e) => {
                    lost = Some(e);
                    continue;
                }
                Err(e) => return Err(e),
            };
            between(Step::BeforeLock, &path);
            match fs4::FileExt::try_lock(&file) {
                Ok(()) => {}
                // A sweep reached the hold before this lock did and
                // took it for a dead download's; it removes the name.
                Err(fs4::TryLockError::WouldBlock) => continue,
                // A volume that cannot lock: the hold is kept unlocked,
                // and no sweep can prove it unheld either.
                Err(fs4::TryLockError::Error(_)) => {}
            }
            if !os::still_named(&file, &path) {
                continue;
            }
            os::hide_dir(&dir, &file);
            let hold = Self {
                dir,
                key,
                file: Some(file),
                parts: std::sync::atomic::AtomicU32::new(0),
            };
            if sweep {
                hold.sweep_dead();
            }
            return Ok(hold);
        }
        Err(lost.unwrap_or_else(|| {
            std::io::Error::other("the download working folder kept disappearing")
        }))
    }

    /// The working folder.
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// A fresh name of this hold's, ending in `suffix`: the key, and
    /// a number no earlier name of the hold's has had.
    pub(crate) fn part(&self, suffix: &str) -> PathBuf {
        let n = self
            .parts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.dir.join(format!("{}-{n}{suffix}", self.key))
    }

    fn hold_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.hold"))
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        self.remove_own_parts();
        if let Some(file) = self.file.take() {
            let path = self.hold_path(&self.key);
            os::release(file, &path);
        }
        // Refused while anything else is in it — another download's
        // files, or a hold a scanner kept open — which is the point.
        let _ = std::fs::remove_dir(&self.dir);
    }
}

#[cfg(test)]
#[path = "download_parts_test.rs"]
mod tests;

#[cfg(test)]
#[path = "download_parts_prop_test.rs"]
mod prop_tests;
