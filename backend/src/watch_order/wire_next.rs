//! The watch order's wire for what follows a finale: the player's Up
//! next card and Home's rail of hand-offs. Split out of `wire.rs`, whose
//! `Reading` it shares, to keep that file under the CRAP high-risk bar.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::super::chain_end::{player_up_next, ChainEndCard, Finale, PlayerUpNext};
use super::super::handoff::{handoff_ask, played_ids, rail_handoffs, AskRow, Handoff, PlayedRow};
use super::super::up_next::UpNext;
use super::super::{Franchise, HistoryRow};
use super::{Reading, ViewEntry};

/// What follows a finale, its entries as the card shows them.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NextWire {
    /// One entry that has aired.
    One {
        /// The entry.
        entry: ViewEntry,
    },
    /// One entry that has not aired yet.
    Unaired {
        /// The entry.
        entry: ViewEntry,
    },
    /// Several ways on.
    Choices {
        /// The entries.
        entries: Vec<ViewEntry>,
    },
}

/// The one card the player shows at a finale.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "card", rename_all = "lowercase")]
pub enum UpNextWire {
    /// What comes next.
    Entry(NextWire),
    /// The franchise page, at a chain's dead end.
    Franchise(ChainEndCard),
}

/// What the player's Up next card shows at `episode`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpNextView {
    /// The card, where the episode is a finale that has one.
    pub card: Option<UpNextWire>,
    /// Whether the history rows were read: false where the history
    /// could not be, and the answer carries no progress for that reason
    /// (a page drawing one already keeps it).
    pub history: bool,
}

/// The player's view of `franchise` for `id` at `episode`. `own` is the
/// player's own reading of the entry, where it has one.
#[must_use]
pub fn up_next_view(franchise: &Franchise, rows: &[HistoryRow], at: Finale<'_>) -> UpNextView {
    let card = player_up_next(franchise, rows, at).map(|card| match card {
        PlayerUpNext::Franchise(c) => UpNextWire::Franchise(c),
        PlayerUpNext::Entry(c) => {
            let read = Reading {
                progress: &c.progress,
                today: at.today,
            };
            UpNextWire::Entry(match c.up_next {
                UpNext::One { entry } => NextWire::One {
                    entry: read.entry(&entry),
                },
                UpNext::Unaired { entry } => NextWire::Unaired {
                    entry: read.entry(&entry),
                },
                UpNext::Choices { entries } => NextWire::Choices {
                    entries: read.entries(&entries),
                },
                // A finale card never says nothing follows.
                UpNext::None => NextWire::Choices {
                    entries: Vec::new(),
                },
            })
        }
    });
    UpNextView {
        card,
        history: true,
    }
}

/// What Home sends to read its rail's hand-offs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RailRequest {
    /// The rail's rows, in order.
    pub rows: Vec<AskRow>,
    /// Every history row, for the played set.
    pub history: Vec<PlayedRow>,
    /// Each history row's Kitsu match: its id, or none for a row matched
    /// to nothing. A row still being matched is absent.
    pub matches: BTreeMap<String, Option<String>>,
    /// The hand-offs read so far, by the Kitsu id that asked: each
    /// series' candidate (none where it has none, or it failed).
    pub series: BTreeMap<String, Option<Handoff>>,
}

/// Home's rail: which ids to read a hand-off for, and the hand-offs it
/// shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RailView {
    /// Each row's ask: the Kitsu id whose hand-off it needs, or none.
    pub asks: BTreeMap<String, Option<String>>,
    /// The hand-off each row shows, by row id.
    pub handoffs: BTreeMap<String, Handoff>,
}

/// Home's rail from what it has read so far. A hand-off shows once the
/// played set is known (every row matched), its next entry is not
/// played, and no row earlier in the rail moves on to the same entry.
#[must_use]
pub fn rail_view(req: &RailRequest) -> RailView {
    let asks: Vec<(String, Option<String>)> = req
        .rows
        .iter()
        .map(|r| (r.row_id.clone(), handoff_ask(r)))
        .collect();
    let played: Option<BTreeSet<String>> = played_ids(&req.history, &req.matches);
    let handoffs = match &played {
        None => BTreeMap::new(),
        Some(played) => rail_handoffs(&asks, |k| {
            req.series
                .get(k)
                .cloned()
                .flatten()
                .filter(|h| !played.contains(&h.next.id))
        }),
    };
    RailView {
        asks: asks.into_iter().collect(),
        handoffs,
    }
}
