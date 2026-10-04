//! One lock over a history file and what its rows leave behind, and
//! the record of its removals.
//!
//! Every write of the history file reads it, changes it and writes it
//! back, so writers take turns: [`hold`] runs a piece of work with the
//! file held, and the plain writers ([`super::upsert_and_write`],
//! [`super::set_kitsu_id`]) go through it. A removal of history holds
//! the file across everything it removes — the rows, and what they left
//! in the cache and beside the file — so nothing else is half-way
//! through a write while it runs.
//!
//! Holding the file is not enough for work that waits on the network
//! between its writes: a watch records its row, waits on Kitsu, then
//! stores the show's mapping, and a removal can run in between. So a
//! history has a moment that every removal moves on. Work takes the
//! moment it began ([`epoch`], [`Held::epoch`]) and, holding the file
//! again for a later write, asks whether the show was removed since
//! ([`Held::show_removed_since`]; [`Held::kitsu_removed_since`] for
//! skip times, which are keyed by Kitsu id). A removal wins over work
//! begun before it; work begun after it is new.
//!
//! Both belong to this process. Another instance of the app writing
//! the same file does not take turns with this one, and its pending
//! work is not known here.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::{read_all, upsert, write_atomic, HistoryEntry};
use crate::error::Result;

/// A moment in a history's removals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Epoch(u64);

/// What has been removed from one history, and when.
#[derive(Default)]
struct Removals {
    /// Moves on with every removal.
    moment: u64,
    /// The moment the history was last cleared.
    cleared: u64,
    /// The moment each show was last removed, since the last clear.
    shows: HashMap<String, u64>,
    /// The moment the skip times cached under each Kitsu id were last
    /// removed with a show, since the last clear.
    kitsu: HashMap<String, u64>,
}

/// Every history this process has held, by its file. One lock for all
/// of them: the work done under it is a file's read and write and a
/// few cache rows.
static HISTORIES: Mutex<BTreeMap<PathBuf, Removals>> = Mutex::new(BTreeMap::new());

/// A history file, held: no other writer of it, and no removal, runs
/// until the work it was handed to returns.
pub struct Held<'a> {
    path: &'a Path,
    removals: &'a mut Removals,
}

/// Run `work` with the history at `path` held. The work must not ask
/// for the history again — [`hold`], [`epoch`], or a writer that goes
/// through them — or it waits on itself.
pub fn hold<T>(path: &Path, work: impl FnOnce(&mut Held<'_>) -> T) -> T {
    // A writer that panicked left the file at its last whole write,
    // which is all the lock protects.
    let mut histories = HISTORIES.lock().unwrap_or_else(PoisonError::into_inner);
    let removals = histories.entry(path.to_path_buf()).or_default();
    work(&mut Held { path, removals })
}

/// The moment an operation on the history at `path` begins.
#[must_use]
pub fn epoch(path: &Path) -> Epoch {
    hold(path, |held| held.epoch())
}

impl Held<'_> {
    /// The history's moment now.
    #[must_use]
    pub fn epoch(&self) -> Epoch {
        Epoch(self.removals.moment)
    }

    /// The file's rows.
    ///
    /// # Errors
    /// The file exists but cannot be read.
    pub fn rows(&self) -> Result<Vec<HistoryEntry>> {
        read_all(self.path)
    }

    /// Replace the file's rows.
    ///
    /// # Errors
    /// The file cannot be written.
    pub fn write(&self, entries: &[HistoryEntry]) -> Result<()> {
        write_atomic(self.path, entries)
    }

    /// Add `new`, or replace the row with its id.
    ///
    /// # Errors
    /// The file cannot be read or written.
    pub fn upsert(&self, new: HistoryEntry) -> Result<()> {
        let mut entries = self.rows()?;
        upsert(&mut entries, new);
        self.write(&entries)
    }

    /// Set the Kitsu id the row for `id` records, `None` clearing it.
    /// A missing row is left missing.
    ///
    /// # Errors
    /// The file cannot be read or written.
    pub fn set_kitsu_id(&self, id: &str, kitsu_id: Option<String>) -> Result<()> {
        let mut entries = self.rows()?;
        for entry in entries.iter_mut().filter(|e| e.id == id) {
            entry.kitsu_id.clone_from(&kitsu_id);
        }
        self.write(&entries)
    }

    /// Record that the show `id` was removed from the history, and
    /// with it the skip times cached under each of `kitsu_ids`.
    pub fn removed_show(&mut self, id: &str, kitsu_ids: &[String]) {
        self.removals.moment += 1;
        let now = self.removals.moment;
        self.removals.shows.insert(id.to_owned(), now);
        for kitsu_id in kitsu_ids {
            self.removals.kitsu.insert(kitsu_id.clone(), now);
        }
    }

    /// Record that the history was cleared.
    pub fn removed_all(&mut self) {
        self.removals.moment += 1;
        self.removals.cleared = self.removals.moment;
        // The clear stands for everything removed before it.
        self.removals.shows.clear();
        self.removals.kitsu.clear();
    }

    /// Whether the show `id` was removed, or the history cleared,
    /// since `begun`.
    #[must_use]
    pub fn show_removed_since(&self, begun: Epoch, id: &str) -> bool {
        let removed = self.removals.shows.get(id).copied().unwrap_or(0);
        removed.max(self.removals.cleared) > begun.0
    }

    /// Whether the skip times cached under `kitsu_id` were removed
    /// with a show, or the history cleared, since `begun`.
    #[must_use]
    pub fn kitsu_removed_since(&self, begun: Epoch, kitsu_id: &str) -> bool {
        let removed = self.removals.kitsu.get(kitsu_id).copied().unwrap_or(0);
        removed.max(self.removals.cleared) > begun.0
    }
}
