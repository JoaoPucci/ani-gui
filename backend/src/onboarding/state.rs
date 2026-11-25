//! The install's onboarding record: `onboarding.json` in the app's
//! state directory, beside the watch history
//! (`config::paths::state_dir`, which places it on Linux and Windows
//! alike).
//!
//! It holds which tips were seen, the release a first-ever install
//! started on, whether that install's baseline is still to be taken,
//! and the release of the first launch each tip's flag was on in.
//! Nothing in it leaves the machine, and nothing records whether a
//! tip was read: only that it was closed.
//!
//! **First-ever installs.** 0.19.0 is the first release with this
//! record, so every install that ran an earlier one has none either,
//! and "no record" cannot mean "new install". The launch decides that
//! from the profile, before anything writes it ([`profile_existed`]):
//! a watch history, a metadata cache or a settings file means the app
//! ran here before. [`boot`] then writes the record at once, so the
//! answer outlives a launch that ends before any tip is asked for.
//!
//! **Reading it.** No file reads as an update's empty record, and so
//! does a file that cannot be read or parsed: a corrupt record must not
//! turn an existing user into a new one, and the next write replaces
//! it. Writes go to a temporary file renamed over the record, so a
//! write cut short leaves the old record whole.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::release::Release;

/// The record's file name in the state directory.
pub const FILE_NAME: &str = "onboarding.json";

/// What the record holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    /// The release a first-ever install's first launch ran; `None` for
    /// a profile an earlier release had already used.
    pub installed_in: Option<Release>,
    /// A first-ever install whose baseline (every tip offered then,
    /// marked seen) is still to be taken. Taken with the launch's
    /// flags, on the first read after they settle.
    pub baseline_pending: bool,
    /// The tips seen, by id.
    pub seen: BTreeSet<String>,
    /// Per flag a tip names, the release of the first launch on which
    /// this install had it on.
    pub flag_on_since: BTreeMap<String, Release>,
}

impl Record {
    /// The record a launch writes when there is none: a first-ever
    /// install's, started on `current`, or an update's.
    #[must_use]
    pub fn new_install(first_ever: bool, current: Release) -> Self {
        Self {
            installed_in: first_ever.then_some(current),
            baseline_pending: first_ever,
            ..Self::default()
        }
    }

    /// The release of the first launch `flag` was on in, if any was.
    #[must_use]
    pub fn flag_on_since(&self, flag: &str) -> Option<Release> {
        self.flag_on_since.get(flag).copied()
    }
}

/// What reading the record found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    /// No file.
    Absent,
    /// A file that could not be read or parsed.
    Unreadable,
    /// The record.
    Record(Record),
}

impl Loaded {
    /// The record to work from: an update's empty record when there is
    /// none to read.
    #[must_use]
    pub fn or_empty(self) -> Record {
        match self {
            Self::Record(r) => r,
            Self::Absent | Self::Unreadable => Record::default(),
        }
    }
}

/// The record's path in `state_dir`.
#[must_use]
pub fn path_in(state_dir: &Path) -> PathBuf {
    state_dir.join(FILE_NAME)
}

/// Read the record at `path`.
#[must_use]
pub fn load(path: &Path) -> Loaded {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_or(Loaded::Unreadable, Loaded::Record),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Loaded::Absent,
        Err(_) => Loaded::Unreadable,
    }
}

/// Write `record` to `path`: to a temporary file beside it, renamed
/// over it.
///
/// # Errors
/// The I/O error that stopped the write.
pub fn save(path: &Path, record: &Record) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Whether the app ran in this profile before: any of `files` (the
/// watch history, the metadata cache, the settings file) is there.
/// Asked before the launch creates any of them.
#[must_use]
pub fn profile_existed(files: &[&Path]) -> bool {
    files.iter().any(|f| f.exists())
}

/// At launch, before anything writes the profile: when there is no
/// record (none at all, not an unreadable one), write the one this
/// install starts with. A failure is logged and the launch goes on;
/// the record is then an update's, as for any missing file.
pub fn boot(state_dir: &Path, profile_existed: bool, current: Release) {
    let path = path_in(state_dir);
    if load(&path) != Loaded::Absent {
        return;
    }
    let record = Record::new_install(!profile_existed, current);
    if let Err(e) = save(&path, &record) {
        tracing::warn!(
            target: "onboarding",
            error = %e,
            "could not write the onboarding record"
        );
    }
}

#[cfg(test)]
#[path = "state_test.rs"]
mod tests;
