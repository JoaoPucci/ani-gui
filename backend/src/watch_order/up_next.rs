//! The player's "Up next" card at a finale (`docs/architecture.md`,
//! "The player's Up next card"): which entries follow the one just
//! finished, in the order the setting chooses. The card names them
//! whether or not the provider carries them, and nothing here asks the
//! provider.

use serde::Serialize;

use super::chain_view::chain_franchise;
use super::entry::is_unaired;
use super::place::placement;
use super::progress::progress_from_history;
use super::run::run_of;
use super::{Franchise, HistoryRow, Progress, WatchEntry, WatchOrder, WatchRun};

/// What follows an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum UpNext {
    /// One entry that has aired.
    One {
        /// The entry.
        entry: WatchEntry,
    },
    /// One entry that has not aired yet.
    Unaired {
        /// The entry.
        entry: WatchEntry,
    },
    /// Several ways on.
    Choices {
        /// The entries, start order.
        entries: Vec<WatchEntry>,
    },
    /// Nothing follows.
    None,
}

impl UpNext {
    /// The entries the card names, in its order.
    #[must_use]
    pub fn entries(&self) -> Vec<&WatchEntry> {
        match self {
            Self::One { entry } | Self::Unaired { entry } => vec![entry],
            Self::Choices { entries } => entries.iter().collect(),
            Self::None => Vec::new(),
        }
    }
}

/// The end-card rule: one aired, one unaired, several, or none.
#[must_use]
pub fn up_next_for(run: &WatchRun, id: &str, order: WatchOrder, today: &str) -> UpNext {
    let mut after = placement(run, id, order).after;
    match after.len() {
        0 => UpNext::None,
        1 => {
            let entry = after.remove(0);
            if is_unaired(&entry, today) {
                UpNext::Unaired { entry }
            } else {
                UpNext::One { entry }
            }
        }
        _ => UpNext::Choices { entries: after },
    }
}

/// What an entry says about itself that decides its finale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinaleSelf {
    /// Kitsu's status: `finished` once the entry has finished airing.
    pub status: Option<String>,
    /// Its episode count.
    pub episodes: Option<u32>,
}

impl From<&WatchEntry> for FinaleSelf {
    fn from(e: &WatchEntry) -> Self {
        Self {
            status: e.status.clone(),
            episodes: e.episodes,
        }
    }
}

/// Whether `episode` is the last of an entry that has finished airing.
#[must_use]
pub fn is_finale(own: &FinaleSelf, episode: f64) -> bool {
    own.status.as_deref() == Some("finished")
        && own.episodes.is_some_and(|n| episode >= f64::from(n))
}

/// What the card shows: the next entries, and their progress for the
/// meta line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FinaleCard {
    /// What follows (never `none`).
    pub up_next: UpNext,
    /// Progress of the entries it names.
    pub progress: Progress,
}

/// The entry `id` is on, in the run its strip draws: its own chain
/// outside the main story, else the main story. None where the
/// franchise was read for another id or only in part.
pub(super) fn drawn_run(franchise: &Franchise, id: &str) -> Option<WatchRun> {
    if franchise.anchor != id || !franchise.complete {
        return None;
    }
    run_of(&chain_franchise(franchise).unwrap_or_else(|| franchise.clone()))
}

/// The card `id`'s `episode` shows, or none: not a finale, the franchise
/// read for another id or only in part (where the entry that really
/// comes next may be missing), `id` outside the run, or nothing after
/// it. `own` is what the player read about the entry; without it the
/// run's entry decides the finale. An entry outside the main story is
/// read on its own chain, as its strip draws it.
#[must_use]
pub fn finale_card(
    franchise: &Franchise,
    rows: &[HistoryRow],
    id: &str,
    order: WatchOrder,
    today: &str,
    episode: f64,
    own: Option<&FinaleSelf>,
) -> Option<FinaleCard> {
    let run = drawn_run(franchise, id)?;
    let entry = run.entries.iter().find(|e| e.id == id)?;
    let decides = own.cloned().unwrap_or_else(|| entry.into());
    if !is_finale(&decides, episode) {
        return None;
    }
    let up_next = up_next_for(&run, id, order, today);
    if up_next == UpNext::None {
        return None;
    }
    let shown: Vec<WatchEntry> = up_next.entries().into_iter().cloned().collect();
    Some(FinaleCard {
        progress: progress_from_history(rows, &shown),
        up_next,
    })
}
