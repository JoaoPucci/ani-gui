//! Continue Watching's hand-off (`docs/architecture.md`, "Continue
//! Watching's hand-off"): once an entry's finale has been played
//! through, its card moves on to the entry that comes next, the way the
//! player's Up next card names it, and opens that entry's show page.
//!
//! It moves on only where exactly one entry comes next, has aired, and
//! has no history row of its own; in every other case the row keeps its
//! ordinary card. Nothing here asks the provider.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::progress::history_episode;
use super::up_next::{drawn_run, is_finale, up_next_for, FinaleSelf, UpNext};
use super::{Franchise, WatchEntry, WatchOrder};

/// A Continue card moved on: the entry played through, and the one after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handoff {
    /// The entry the row played through.
    pub finished: WatchEntry,
    /// The entry the card moves on to.
    pub next: WatchEntry,
}

/// What a Continue row's Kitsu match says about the entry it plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffMatch {
    /// Its Kitsu id.
    pub id: String,
    /// Kitsu's status: `finished` once the entry has finished airing.
    pub status: Option<String>,
    /// Its episode count.
    pub episode_count: Option<u32>,
}

/// One Continue row as the hand-off's ask reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskRow {
    /// The history row's id.
    pub row_id: String,
    /// Its Kitsu match, once Home has one.
    #[serde(default)]
    pub matched: Option<HandoffMatch>,
    /// The episode the row names.
    pub ep_no: String,
    /// The episode count the row's availability probe found, until it answers none.
    #[serde(default)]
    pub playable_count: Option<u32>,
    /// The player kept a resume position for the row's last episode: it
    /// was left part-way.
    #[serde(default)]
    pub left_part_way: bool,
}

/// The Kitsu id whose series a row's hand-off needs, or none: a matched
/// row whose last episode is the finale of an entry that has finished
/// airing, and was not left part-way. Only such a row asks for its
/// series, so no other card costs a walk.
#[must_use]
pub fn handoff_ask(row: &AskRow) -> Option<String> {
    let matched = row.matched.as_ref()?;
    let episode = history_episode(&row.ep_no)?;
    // The player's own last step: a source listing past Kitsu's count
    // plays on to that episode first.
    let last = matched
        .episode_count
        .map(|n| n.max(row.playable_count.unwrap_or(0)));
    let own = FinaleSelf {
        status: matched.status.clone(),
        episodes: last,
    };
    if !is_finale(&own, episode) || row.left_part_way {
        return None;
    }
    Some(matched.id.clone())
}

/// The hand-off from `id`, the entry a row played through, or none: the
/// series read for another id or only in part, `id` outside it, a fork,
/// an entry not aired yet, nothing after it, or an entry after it that
/// `played` holds.
#[must_use]
pub fn handoff_from(
    franchise: &Franchise,
    id: &str,
    order: WatchOrder,
    today: &str,
    played: &BTreeSet<String>,
) -> Option<Handoff> {
    // Outside the main story, the show moves on along its own chain.
    let run = drawn_run(franchise, id)?;
    let entry = run.entries.iter().find(|e| e.id == id)?;
    match up_next_for(&run, id, order, today) {
        UpNext::One { entry: next } if !played.contains(&next.id) => Some(Handoff {
            finished: entry.clone(),
            next,
        }),
        _ => None,
    }
}

/// A history row as the played set reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayedRow {
    /// The row's id.
    pub id: String,
    /// The Kitsu id it recorded, for a row written since history records one.
    #[serde(default)]
    pub kitsu_id: Option<String>,
}

/// The Kitsu ids with a history row: each row's recorded id, and the id
/// its match found. None while any row is still being matched (absent
/// from `matches`): until then a show it played could look unplayed. A
/// row matched to nothing is `Some(None)`.
#[must_use]
pub fn played_ids(
    rows: &[PlayedRow],
    matches: &BTreeMap<String, Option<String>>,
) -> Option<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for row in rows {
        if let Some(k) = &row.kitsu_id {
            out.insert(k.clone());
        }
        if let Some(found) = matches.get(&row.id)? {
            out.insert(found.clone());
        }
    }
    Some(out)
}

/// Each rail row's hand-off, keyed by row id. `asks` is the rail in
/// order, each row with its [`handoff_ask`]. Two rows that would move on
/// to the same entry show it once: the first in the rail moves on, the
/// other keeps its ordinary card.
pub fn rail_handoffs(
    asks: &[(String, Option<String>)],
    mut from: impl FnMut(&str) -> Option<Handoff>,
) -> BTreeMap<String, Handoff> {
    let mut out = BTreeMap::new();
    let mut shown = BTreeSet::new();
    for (row_id, kitsu_id) in asks {
        let Some(handoff) = kitsu_id.as_deref().and_then(&mut from) else {
            continue;
        };
        if shown.insert(handoff.next.id.clone()) {
            out.insert(row_id.clone(), handoff);
        }
    }
    out
}

/// `handoff` once the resume lookup has answered that no history row
/// resumes its next entry (`named` `Some(false)`), else none: the lookup
/// failed (`None`), or a row resumes it. The played set reads Home's own
/// matches, which can miss a row the lookup names for the entry.
#[must_use]
pub fn unclaimed_handoff(handoff: Option<Handoff>, named: Option<bool>) -> Option<Handoff> {
    if named == Some(false) {
        handoff
    } else {
        None
    }
}
