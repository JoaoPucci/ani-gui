//! One lock over a history file and what its rows leave behind, and
//! the record of its removals.
//!
//! Every write of the history file reads it, changes it and writes it
//! back, so writers take turns: [`hold`] runs a piece of work with the
//! file held, and every writer of the file goes through it — the plain
//! [`super::upsert_and_write`], and the plays and watches that write a
//! row with something beside it. So does everything that stores what a
//! row leaves behind: the stamp, the mapping, the title match, the
//! numbering, the resolution row, the skip times. A removal of history
//! holds the file across everything it removes, so none of those is
//! half-way through a store while it runs. What only takes away — an
//! eviction, a cache clear — does not ask for the file.
//!
//! Holding the file is not enough for work that waits on the network
//! between its writes: a watch records its row, waits on Kitsu, then
//! stores the show's mapping, and a removal can run in between. So a
//! history has a moment that every removal moves on. Work takes the
//! moment it began ([`epoch`], [`Held::epoch`]) and, holding the file
//! again for a later write, asks whether the show was removed since
//! ([`Held::show_removed_since`]). A removal wins over work begun
//! before it; work begun after it is new.
//!
//! A later watch is the other thing a watch's deferred writes must not
//! undo: a second watch of the show can write its row, and settle its
//! id and mapping, while the first still waits on Kitsu. So a write
//! that gives a row its watch moment or its Kitsu id moves the moment
//! on as well, and a watch's deferred writes ask whether the row was
//! changed since — removed, or written by another watch
//! ([`Held::show_changed_since`]). The first write of a play or a watch
//! asks the same of the moment its request began
//! ([`Held::overtaken_since`]): a request that stalled while a later
//! watch was recorded leaves that watch's row and stamp as they are.
//!
//! A show is removed under its row's key, and not everything that
//! would write of it has that key: skip times are cached under the
//! Kitsu id of the page they were fetched from, and a play resolves
//! under the key of whichever provider answers, which is another's
//! when the walk fails over. So a removal also records the Kitsu ids
//! the show was known by — the one its row recorded, the ones its
//! mapping and title match named, and the pages this process saw it
//! played from ([`Held::played_from`]), the row itself recording its
//! page only once the watch's verdict is in — less any a remaining row
//! still claims. A play names the page it was asked from ([`Asked`])
//! and loses to a removal of a show known by it
//! ([`Held::removed_since`]); a skip-time lookup, and a detail read
//! that would mark its id gone, ask by the id
//! ([`Held::kitsu_removed_since`]).
//!
//! Both belong to this process, which is enough because only one runs
//! per profile: the desktop shell takes a single-instance lock before
//! it spawns a backend (`electron/lib/single-instance.cjs`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::{read_all, upsert, write_atomic, HistoryEntry};
use crate::error::Result;

/// A moment in a history's removals and watches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Epoch(u64);

/// A play as it was asked for: the moment its request began, and the
/// Kitsu page it was made from when the caller named one.
#[derive(Debug, Clone, Copy)]
pub struct Asked<'a> {
    /// The moment the request began.
    pub begun: Epoch,
    /// The Kitsu id of the page the play was asked from.
    pub page: Option<&'a str>,
}

impl<'a> Asked<'a> {
    /// A request on the history at `path` that begins now, made from
    /// the Kitsu page `page`. Only a Kitsu id is a page
    /// ([`super::kitsu_id_in`]); anything else, the empty id included,
    /// is no page.
    #[must_use]
    pub fn now(path: &Path, page: Option<&'a str>) -> Self {
        Self {
            begun: epoch(path),
            page: page.and_then(super::kitsu_id_in),
        }
    }
}

/// What has been removed from one history, and when.
#[derive(Default)]
struct Removals {
    /// Moves on with every removal, and every write of a row's watch
    /// moment or Kitsu id.
    moment: u64,
    /// The moment the history was last cleared.
    cleared: u64,
    /// The moment each show was last removed, since the last clear.
    shows: HashMap<String, u64>,
    /// The moment each show's row was last given a watch moment or a
    /// Kitsu id, since the last clear.
    watched: HashMap<String, u64>,
    /// The moment a show known by each Kitsu id was last removed, with
    /// no remaining row still claiming the id, since the last clear.
    kitsu: HashMap<String, u64>,
    /// The Kitsu pages each show in the history was played from, as
    /// far as this process saw. A row records its page only once the
    /// watch's verdict is in, and a removal before that has to know it.
    pages: HashMap<String, BTreeSet<String>>,
    /// The moment Kitsu last served each id, as far as this process
    /// saw: a failure of a read begun before it says nothing newer.
    served: HashMap<String, u64>,
    /// The moment Kitsu last answered each id gone, as far as this
    /// process saw: a success served before it says nothing newer.
    gone: HashMap<String, u64>,
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
/// for the history again — [`hold`], [`epoch`], [`Asked::now`], or a
/// writer that goes through them — or it waits on itself.
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
    pub fn upsert(&mut self, new: HistoryEntry) -> Result<()> {
        let mut entries = self.rows()?;
        let id = new.id.clone();
        let watch = new.watched_at.is_some() || new.kitsu_id.is_some();
        upsert(&mut entries, new);
        self.write(&entries)?;
        if watch {
            self.watched(&id);
        }
        Ok(())
    }

    /// Note that the row for `id` was given a watch moment or a Kitsu id.
    fn watched(&mut self, id: &str) {
        self.removals.moment += 1;
        let now = self.removals.moment;
        self.removals.watched.insert(id.to_owned(), now);
    }

    /// Set the Kitsu id the row for `id` records, `None` clearing it.
    /// A missing row is left missing.
    ///
    /// # Errors
    /// The file cannot be read or written.
    pub fn set_kitsu_id(&mut self, id: &str, kitsu_id: Option<String>) -> Result<()> {
        let mut entries = self.rows()?;
        for entry in entries.iter_mut().filter(|e| e.id == id) {
            entry.kitsu_id.clone_from(&kitsu_id);
        }
        self.write(&entries)?;
        self.watched(id);
        Ok(())
    }

    /// Note that the show `id` was played from the Kitsu page `page`.
    pub fn played_from(&mut self, id: &str, page: Option<&str>) {
        if let Some(page) = page {
            self.removals
                .pages
                .entry(id.to_owned())
                .or_default()
                .insert(page.to_owned());
        }
    }

    /// The Kitsu pages the show `id` was played from, as far as this
    /// process saw.
    #[must_use]
    pub fn pages_of(&self, id: &str) -> Vec<String> {
        self.removals
            .pages
            .get(id)
            .map(|pages| pages.iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[path = "guard_kitsu.rs"]
mod kitsu;

#[path = "guard_removals.rs"]
mod removals;
