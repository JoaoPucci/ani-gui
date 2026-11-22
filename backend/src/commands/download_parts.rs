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

/// The working folder's name inside a destination.
pub(crate) const PARTS_DIR: &str = ".ani-gui-parts";

/// Where `dest`'s unfinished files live.
pub(crate) fn parts_dir(dest: &Path) -> PathBuf {
    dest.join(PARTS_DIR)
}

/// What a name in the working folder is to the download that owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Entry {
    /// `<key>.hold`, the file the owner keeps locked.
    Hold,
    /// `<key>-…`, one of the owner's files.
    Part,
}

const KEY_PREFIX: &str = ".ani-gui-";
/// The random part of a key: 64 bits, in lowercase hex. Short, because
/// every name a tool derives inside the working folder carries it on
/// top of the destination's path, and Windows paths have a ceiling.
const ID_LEN: usize = 16;

/// The key that owns `name`, and what the name is to it; none for a
/// name this scheme did not give out, which the sweep leaves alone.
///
/// A key is `.ani-gui-<pid>-<16 lowercase hex>`, exactly as [`Hold`]
/// spells it, so the parse is syntactic: the hold is the key and
/// `.hold`, a part the key and a hyphen and anything after it.
#[must_use]
pub(crate) fn owner_of(name: &str) -> Option<(&str, Entry)> {
    let rest = name.strip_prefix(KEY_PREFIX)?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = rest[digits..].strip_prefix('-')?;
    let id = rest.get(..ID_LEN)?;
    if !id
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let key_len = KEY_PREFIX.len() + digits + 1 + ID_LEN;
    let (key, tail) = name.split_at(key_len);
    if tail == ".hold" {
        Some((key, Entry::Hold))
    } else if tail.len() > 1 && tail.starts_with('-') {
        Some((key, Entry::Part))
    } else {
        None
    }
}

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
    /// What creating the folder or the hold reports, or an error of
    /// its own when the folder kept disappearing.
    pub(crate) fn take(dest: &Path, sweep: bool) -> std::io::Result<Self> {
        let dir = parts_dir(dest);
        for _ in 0..ATTEMPTS {
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
            let file = match os::lock_file_options().create_new(true).open(&path) {
                Ok(file) => file,
                // The folder went between creating it and the open:
                // another download removed it as it emptied.
                Err(e) if os::lost_to_removal(&e) => continue,
                Err(e) => return Err(e),
            };
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
        Err(std::io::Error::other(
            "the download working folder kept disappearing",
        ))
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

    /// Remove the files of every other key whose download is provably
    /// not running: its hold gone, or locked here. A hold that cannot
    /// be opened or locked is somebody's, or unprovable, and stays.
    fn sweep_dead(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        self.sweep_listed(entries.flatten().map(|e| e.file_name()));
    }

    /// [`Hold::sweep_dead`] over the names a listing of the folder
    /// returned — the seam the cases drive with a listing of their own.
    fn sweep_listed(&self, listed: impl IntoIterator<Item = std::ffi::OsString>) {
        let mut holds = std::collections::BTreeSet::new();
        let mut parts: std::collections::BTreeMap<String, Vec<PathBuf>> =
            std::collections::BTreeMap::new();
        for name in listed {
            let Some(name) = name.to_str() else { continue };
            match owner_of(name) {
                Some((key, Entry::Hold)) => {
                    holds.insert(key.to_owned());
                }
                Some((key, Entry::Part)) => {
                    parts
                        .entry(key.to_owned())
                        .or_default()
                        .push(self.dir.join(name));
                }
                None => {}
            }
        }
        let keys: std::collections::BTreeSet<&String> = holds.iter().chain(parts.keys()).collect();
        for key in keys {
            if *key == self.key {
                continue;
            }
            let remove_parts = || {
                for path in parts.get(key).into_iter().flatten() {
                    let _ = std::fs::remove_file(path);
                }
            };
            // Asked of the hold's path even when the listing showed
            // none: a listing is not a snapshot, and a hold created
            // while it ran may be missing from it beside a part that is
            // not.
            let path = self.hold_path(key);
            match os::lock_file_options().open(&path) {
                Ok(file) => {
                    if fs4::FileExt::try_lock(&file).is_ok() {
                        remove_parts();
                        os::release(file, &path);
                    }
                }
                // No hold: a hold exists before its first part, so its
                // owner ended, or a sweep beside this one removed it —
                // either way the parts are no one's now.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => remove_parts(),
                Err(_) => {}
            }
        }
    }

    /// Remove every file of this hold's key except the hold itself.
    fn remove_own_parts(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name
                .to_str()
                .and_then(owner_of)
                .is_some_and(|(key, kind)| key == self.key && kind == Entry::Part)
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
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
