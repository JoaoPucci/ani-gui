//! The sweep of what downloads that are no longer running left in the
//! working folder, and a hold's removal of its own files.

use super::names::{owner_of, Entry};
use super::{os, Hold};
use std::path::PathBuf;

impl Hold {
    /// Remove the files of every other key whose download is provably
    /// not running: its hold gone, or locked here. A hold that cannot
    /// be opened or locked is somebody's, or unprovable, and stays.
    pub(super) fn sweep_dead(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        self.sweep_listed(entries.flatten().map(|e| e.file_name()));
    }

    /// [`Hold::sweep_dead`] over the names a listing of the folder
    /// returned — the seam the cases drive with a listing of their own.
    pub(super) fn sweep_listed(&self, listed: impl IntoIterator<Item = std::ffi::OsString>) {
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
    pub(super) fn remove_own_parts(&self) {
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
