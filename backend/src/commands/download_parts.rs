//! The claim a download keeps on the unfinished files it writes in its
//! destination, and the names it gives them.

use std::path::{Path, PathBuf};

const KEY_PREFIX: &str = ".ani-gui-";

/// The working folder's name inside a destination.
pub(crate) const PARTS_DIR: &str = ".ani-gui-parts";

/// Where `dest`'s unfinished files are to live.
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

/// The key that owns `name`, and what the name is to it. Not yet
/// answered: nothing owns anything.
#[must_use]
pub(crate) fn owner_of(_name: &str) -> Option<(&str, Entry)> {
    None
}

/// A download's claim on its unfinished files: where they go, and a
/// fresh name for each. For now every name sits beside the target.
#[derive(Debug)]
pub(crate) struct Hold {
    dir: PathBuf,
}

impl Hold {
    /// Take a hold in `dest`.
    ///
    /// # Errors
    /// None yet; the signature is the one the working folder needs.
    pub(crate) fn take(dest: &Path, _sweep: bool) -> std::io::Result<Self> {
        Ok(Self {
            dir: dest.to_path_buf(),
        })
    }

    /// Where the hold's files go.
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// A fresh name of this hold's, ending in `suffix`.
    pub(crate) fn part(&self, suffix: &str) -> PathBuf {
        self.dir.join(format!(
            "{KEY_PREFIX}{}-{}{suffix}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ))
    }
}

#[cfg(test)]
#[path = "download_parts_prop_test.rs"]
mod prop_tests;
#[cfg(test)]
#[path = "download_parts_test.rs"]
mod tests;
