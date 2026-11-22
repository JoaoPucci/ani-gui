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
            let file = match os::hold_options().create_new(true).open(&path) {
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
        let mut holds = std::collections::BTreeSet::new();
        let mut parts: std::collections::BTreeMap<String, Vec<PathBuf>> =
            std::collections::BTreeMap::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            match owner_of(name) {
                Some((key, Entry::Hold)) => {
                    holds.insert(key.to_owned());
                }
                Some((key, Entry::Part)) => {
                    parts.entry(key.to_owned()).or_default().push(entry.path());
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
            if !holds.contains(key) {
                // A hold exists before its first part and goes after
                // its last: parts with none have no writer.
                remove_parts();
                continue;
            }
            let path = self.hold_path(key);
            match os::hold_options().open(&path) {
                Ok(file) => {
                    if fs4::FileExt::try_lock(&file).is_ok() {
                        remove_parts();
                        os::release(file, &path);
                    }
                }
                // Gone since the listing: its owner ended, or a sweep
                // beside this one removed it — either way after the
                // parts, which are no one's now.
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

#[cfg(not(windows))]
mod os {
    /// How a hold is opened: for writing, which the lock needs, never
    /// truncated. Creating is the caller's choice.
    pub(super) fn hold_options() -> std::fs::OpenOptions {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).truncate(false);
        options
    }

    /// Nothing to do: the leading dot already hides the folder.
    pub(super) fn hide_dir(_dir: &std::path::Path, _file: &std::fs::File) {}

    /// The folder went between creating it and the open.
    pub(super) fn lost_to_removal(e: &std::io::Error) -> bool {
        e.kind() == std::io::ErrorKind::NotFound
    }

    /// Whether `file` is still the file named `path`: a sweep can
    /// remove the name between the create and the lock, and a lock on
    /// an unnamed file holds nothing anyone can see.
    pub(super) fn still_named(file: &std::fs::File, path: &std::path::Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        matches!(
            (file.metadata(), std::fs::metadata(path)),
            (Ok(held), Ok(named)) if held.dev() == named.dev() && held.ino() == named.ino()
        )
    }

    /// Remove the name while the lock is still held, then let go.
    pub(super) fn release(file: std::fs::File, path: &std::path::Path) {
        if still_named(&file, path) {
            let _ = std::fs::remove_file(path);
        }
        drop(file);
    }
}

#[cfg(windows)]
mod os {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_DELETE_PENDING: i32 = 303;

    /// How a hold is opened: for writing, never truncated, shared for
    /// reading and writing but not for deletion — so no sweep can
    /// remove a hold while its owner has it open — and hidden if this
    /// open creates it. Creating is the caller's choice.
    pub(super) fn hold_options() -> std::fs::OpenOptions {
        let mut options = std::fs::OpenOptions::new();
        options
            .write(true)
            .truncate(false)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .attributes(FILE_ATTRIBUTE_HIDDEN);
        options
    }

    /// Give the folder the hidden attribute, unless it has it.
    ///
    /// The standard library sets attributes only from a
    /// [`std::fs::Permissions`] it read from a file, so they are
    /// copied from the hold just created, which is hidden and — opened
    /// for writing — not read-only. Best-effort: a visible folder is
    /// cosmetic.
    pub(super) fn hide_dir(dir: &std::path::Path, file: &std::fs::File) {
        let hidden = |m: &std::fs::Metadata| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0;
        if std::fs::metadata(dir).is_ok_and(|m| hidden(&m)) {
            return;
        }
        let Ok(meta) = file.metadata() else { return };
        if hidden(&meta) {
            let _ = std::fs::set_permissions(dir, meta.permissions());
        }
    }

    /// The open met a removal in flight: the folder gone, or a delete
    /// of it still pending, which Windows reports as a denial.
    pub(super) fn lost_to_removal(e: &std::io::Error) -> bool {
        e.kind() == std::io::ErrorKind::NotFound
            || matches!(
                e.raw_os_error(),
                Some(ERROR_SHARING_VIOLATION | ERROR_DELETE_PENDING | ERROR_ACCESS_DENIED)
            )
    }

    /// Always: a file opened without delete sharing cannot lose its
    /// name while the handle is open.
    pub(super) fn still_named(_file: &std::fs::File, _path: &std::path::Path) -> bool {
        true
    }

    /// Let go first, then delete: the delete needs the handle closed,
    /// since it was opened without delete sharing.
    pub(super) fn release(file: std::fs::File, path: &std::path::Path) {
        drop(file);
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
#[path = "download_parts_prop_test.rs"]
mod prop_tests;
#[cfg(test)]
#[path = "download_parts_test.rs"]
mod tests;
