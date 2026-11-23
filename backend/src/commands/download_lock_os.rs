//! What differs between platforms in opening, checking and removing a
//! lock file. See [`super::release::InstanceLock`] for why each side does what
//! it does.
//!
//! The download working folder's holds go through the same layer (see
//! `download_parts.rs`): a hold is a lock file in a folder of its own,
//! opened, hidden, checked and released the same way. Where a comment
//! here speaks of the lock directory, the working folder is treated
//! alike; the note about files an earlier version created applies to
//! lock files only, since no earlier version made holds.

pub(crate) use imp::*;

#[cfg(not(windows))]
mod imp {
    /// How every lock file is opened: for writing (which the lock
    /// needs), never truncated. Creating is the caller's choice.
    pub(crate) fn lock_file_options() -> std::fs::OpenOptions {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).truncate(false);
        options
    }

    /// Nothing to do: the leading dot already hides the directory.
    pub(crate) fn hide_dir(_dir: &std::path::Path, _file: &std::fs::File) {}

    /// The directory went between creating it and opening the file in
    /// it: another instance removed it as it emptied.
    ///
    /// Linux answers that with "not found". macOS can answer it with
    /// "invalid argument" instead: its CI caught a contender's open
    /// failing so while the others took and released the lock, which
    /// is what removes the directory. An open that keeps failing that
    /// way for some other reason meets the cap on losses in a row (the
    /// working folder's on attempts), and is refused there.
    pub(crate) fn lost_to_removal(e: &std::io::Error) -> bool {
        lost_to_removal_on(e, cfg!(target_os = "macos"))
    }

    /// EINVAL, the same code on macOS and Linux.
    const INVALID_ARGUMENT: i32 = 22;

    /// [`lost_to_removal`], for macOS or not as `macos` says — so both
    /// answers can be checked wherever the tests run.
    pub(crate) fn lost_to_removal_on(e: &std::io::Error, macos: bool) -> bool {
        e.kind() == std::io::ErrorKind::NotFound
            || (macos && e.raw_os_error() == Some(INVALID_ARGUMENT))
    }

    /// Whether the locked `file` is still the file named `path`: the
    /// same device and inode.
    pub(crate) fn still_named(file: &std::fs::File, path: &std::path::Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        let same =
            |a: &std::fs::Metadata, b: &std::fs::Metadata| a.dev() == b.dev() && a.ino() == b.ino();
        matches!(
            (file.metadata(), std::fs::metadata(path)),
            (Ok(held), Ok(named)) if same(&held, &named)
        )
    }

    /// Never: nothing but the lock's own protocol removes or holds the
    /// name here, so a delete that failed fails again.
    pub(crate) fn delete_refused_for_now(_e: &std::io::Error) -> bool {
        false
    }

    /// Give up a handle that holds no lock: close it. The name is not
    /// this handle's to remove.
    pub(crate) fn abandon(file: std::fs::File, _path: &std::path::Path) {
        drop(file);
    }

    /// Remove the name while the lock is still held, then release it.
    /// Checked first, so a file somebody else put at the name — after
    /// removing this one from outside the protocol — is not taken from
    /// them.
    pub(crate) fn release(file: std::fs::File, path: &std::path::Path) {
        if still_named(&file, path) {
            super::super::remove::remove_lock_file(path);
        }
        drop(file);
    }
}

#[cfg(windows)]
mod imp {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_DELETE_PENDING: i32 = 303;

    /// How every lock file is opened: for writing (which the lock
    /// needs), never truncated, shared for reading and writing but not
    /// for deletion, and hidden if this open creates it. Creating is
    /// the caller's choice.
    pub(crate) fn lock_file_options() -> std::fs::OpenOptions {
        let mut options = std::fs::OpenOptions::new();
        options
            .write(true)
            .truncate(false)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .attributes(FILE_ATTRIBUTE_HIDDEN);
        options
    }

    /// Give the lock directory the hidden attribute, unless it has it.
    ///
    /// The standard library sets attributes only from a
    /// [`std::fs::Permissions`] it read from a file, so they are copied
    /// from the lock file just opened, which was created hidden — and
    /// is not read-only, having just been opened for writing. A file
    /// that is not hidden — one an earlier version created and
    /// somebody still holds, since the sweep removed the rest — is no
    /// source, and the next lock file hides the directory instead.
    /// The copy replaces the directory's attributes rather than adding
    /// one: the standard library has no way to set a single bit.
    /// Best-effort throughout: a visible directory is cosmetic.
    pub(crate) fn hide_dir(dir: &std::path::Path, file: &std::fs::File) {
        let hidden = |m: &std::fs::Metadata| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0;
        if std::fs::metadata(dir).is_ok_and(|m| hidden(&m)) {
            return;
        }
        let Ok(meta) = file.metadata() else { return };
        if hidden(&meta) {
            let _ = std::fs::set_permissions(dir, meta.permissions());
        }
    }

    /// The open met a removal in flight: the directory gone (not
    /// found), a delete holding the file (sharing violation), or a
    /// delete still pending, which Windows reports as a denial.
    pub(crate) fn lost_to_removal(e: &std::io::Error) -> bool {
        e.kind() == std::io::ErrorKind::NotFound
            || matches!(
                e.raw_os_error(),
                Some(ERROR_SHARING_VIOLATION | ERROR_DELETE_PENDING | ERROR_ACCESS_DENIED)
            )
    }

    /// Always: a file opened without delete sharing cannot lose its
    /// name while the handle is open.
    pub(crate) fn still_named(_file: &std::fs::File, _path: &std::path::Path) -> bool {
        true
    }

    /// A delete refused by a handle that may close in a moment: one
    /// opened without sharing delete (a waiter, or a scanner or indexer
    /// looking at the new file), or a delete still pending.
    pub(crate) fn delete_refused_for_now(e: &std::io::Error) -> bool {
        matches!(
            e.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION | ERROR_ACCESS_DENIED)
        )
    }

    /// Give up a handle that holds no lock — only by taking the lock.
    ///
    /// Granted, nobody holds the file, and it is released like any
    /// held lock. Refused, somebody does, and the handle is only
    /// closed: this app's holder removes the file on release, and one
    /// of an earlier version, which never does, leaves it to the next
    /// run's sweep. Closing and deleting without
    /// asking would be safe against this app's own handles, which do
    /// not share delete, but not against one that does — the standard
    /// library's default, which an earlier version of the app opens
    /// with — and that delete would remove a file its holder still
    /// holds.
    pub(crate) fn abandon(file: std::fs::File, path: &std::path::Path) {
        if fs4::FileExt::try_lock(&file).is_ok() {
            release(file, path);
        }
    }

    /// Release first, then delete — which fails, harmlessly, while any
    /// other handle is open on the file (see
    /// [`super::super::remove::remove_lock_file`] for how long it keeps
    /// trying).
    pub(crate) fn release(file: std::fs::File, path: &std::path::Path) {
        drop(file);
        super::super::remove::remove_lock_file(path);
    }
}
