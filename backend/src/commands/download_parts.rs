//! The claim a download keeps on the unfinished files it writes in its
//! destination, and the names it gives them.

use std::path::{Path, PathBuf};

const KEY_PREFIX: &str = ".ani-gui-";

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
