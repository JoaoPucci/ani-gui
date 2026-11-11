//! A directory of its own for a test's `AppState`, removed once the
//! test is done with the state.
//!
//! The state carries paths for the history file, the image cache, the
//! settings file and the account store. A test helper that fills them
//! with fixed paths gives every test it builds the same files: cargo
//! runs those tests side by side, so one reads the rows another wrote,
//! and whatever they wrote outlives the run. [`dir`] makes a fresh
//! temporary directory, its methods name the state's files inside it,
//! and [`Dir::hold`] ties the directory to the state, so the files go
//! when the state does.
//!
//! [`Scratch`] dereferences to what it holds, so a helper that returned
//! `AppState` returns `Scratch<AppState>` and its callers read and
//! mutate the state as before. Taking the state out of it is not
//! offered: that would remove the directory under a state still using
//! it.

use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

/// A temporary directory a test's state keeps its files in.
pub(crate) struct Dir(tempfile::TempDir);

/// A fresh, empty directory for one test state.
pub(crate) fn dir() -> Dir {
    Dir(tempfile::tempdir().expect("a scratch directory for the test state"))
}

impl Dir {
    /// The directory itself.
    pub(crate) fn path(&self) -> &Path {
        self.0.path()
    }

    /// Where the state's history file goes.
    pub(crate) fn history(&self) -> PathBuf {
        self.path().join("history")
    }

    /// Where the state's image cache goes.
    pub(crate) fn images(&self) -> PathBuf {
        self.path().join("images")
    }

    /// Where the state's settings file goes.
    pub(crate) fn config(&self) -> PathBuf {
        self.path().join("config.toml")
    }

    /// Where the state's account store goes.
    pub(crate) fn state(&self) -> PathBuf {
        self.path().join("state")
    }

    /// Keep this directory for as long as `value` lives.
    pub(crate) fn hold<T>(self, value: T) -> Scratch<T> {
        Scratch { value, _dir: self }
    }
}

/// A value together with the directory its paths point into. Dropping
/// it drops the value first, then removes the directory.
pub(crate) struct Scratch<T> {
    value: T,
    _dir: Dir,
}

impl<T> Deref for Scratch<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for Scratch<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

mod tests {
    use super::*;

    #[test]
    fn the_directory_goes_with_the_value_it_holds() {
        let d = dir();
        let root = d.path().to_path_buf();
        std::fs::write(d.history(), "row\n").expect("writable");
        let held = d.hold(7_u8);
        assert_eq!(*held, 7);
        assert!(root.exists(), "the directory went before the value");
        drop(held);
        assert!(!root.exists(), "{} outlived its value", root.display());
    }

    #[test]
    fn two_directories_never_coincide() {
        let (a, b) = (dir(), dir());
        assert_ne!(a.path(), b.path());
        for p in [a.history(), a.images(), a.config(), a.state()] {
            assert!(p.starts_with(a.path()), "{} escapes", p.display());
        }
    }
}
