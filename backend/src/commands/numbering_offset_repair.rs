//! Putting a show's numbering stamps right from a listing at hand, in
//! the order the listings were heard. Split from [`super`] for the
//! per-file complexity bar.
//!
//! Two walks of a show can overlap — a play and a probe, two probes —
//! and the one that heard its listing first can finish, and repair,
//! last. Each repair carries its listing's hearing
//! ([`crate::commands::listing_tags::Hearing`]), and the stamps end as
//! the listing heard last repairs them:
//! - a repair heard wholly before the last one this run took writes
//!   nothing;
//! - one heard wholly after it repairs the stamps as they stood before
//!   this run's first repair, so what an earlier listing decided — a
//!   former tag it found free, a slot it moved the stamp to — does not
//!   outlive a later listing that decides otherwise; a later listing
//!   that repairs nothing leaves the last repair as it is;
//! - two whose hearings overlap cannot be ordered, so the order they
//!   write in changes nothing: a former tag only one of them names is
//!   not named, as the other's listing may give it to a row of its own,
//!   and two that put the display stamp in different places leave it as
//!   it was before this run's repairs.
//!
//! What a repair started from is kept for this run alone. A hearing
//! counts this process's walks, and a stamp an earlier run repaired is
//! where this run's first repair starts; so is a stamp something other
//! than a repair changed since the last one.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use super::numbering_offset_store::{parse, read_store, rewrite_if, Row};
use super::AppState;
use crate::commands::listing_tags::{Hearing, Order};

#[path = "numbering_offset_repair_rule.rs"]
mod rule;
pub(super) use rule::Listing;
use rule::{repaired, settle, Stamps};

/// The repair a show's stamps last took this run: when its listing was
/// heard (spanning every overlapping one), the stamps from before the
/// run's first repair, and the stamps it left. Stamps that are no longer
/// the ones it left were written by something else since — a removal,
/// another instance — and a repair starts over from them.
struct Taken {
    heard: Hearing,
    base: Stamps,
    left: Stamps,
}

/// The repairs taken this run, by history file and show key. Read and
/// written inside the store's locks, as a repair decides.
static TAKEN: Mutex<BTreeMap<(PathBuf, String), Taken>> = Mutex::new(BTreeMap::new());

/// Repair the stamps of the show `slug` keys from `listing`, heard as
/// `heard` says; `None` for a repair that knows no hearing, which is
/// ordered against nothing and leaves nothing for another to be.
/// Decided under the store's locks, against the row as it stands there.
pub(super) fn repair(state: &AppState, slug: &str, listing: &Listing<'_>, heard: Option<Hearing>) {
    // A show with no display stamp has nothing to repair, and its store
    // is not written.
    let rows = parse(&read_store(&state.history_path));
    let Some(found) = rows
        .iter()
        .find(|r| r.slug == slug)
        .and_then(|r| r.display.clone())
    else {
        return;
    };
    let decide = |rows: &mut Vec<Row>| {
        // A stamp that is not the one the unlocked read found is left as
        // it is. A build from before the marker, running beside this one
        // on the same files, stamps a play before it writes the play's
        // row, outside these locks; a stamp that moved meanwhile may
        // have that row on its way, and the next listing at hand repairs
        // a stamp that has none.
        let Some(row) = rows
            .iter_mut()
            .find(|r| r.slug == slug && r.display.as_ref() == Some(&found))
        else {
            return false;
        };
        // The history row as it stands under the store's locks, read
        // without the history guard so a repair inside
        // `stamp_numbering`'s hold cannot deadlock.
        let row_ep = listing.rows.and_then(|_| legacy_row_ep(state, slug));
        let mut taken = TAKEN.lock().unwrap_or_else(PoisonError::into_inner);
        let key = (state.history_path.clone(), slug.to_string());
        let Some(next) = decided(&mut taken, key, row, row_ep.as_deref(), listing, heard) else {
            return false;
        };
        row.display = next.display;
        row.former = next.former;
        true
    };
    if let Err(e) = rewrite_if(state, decide) {
        tracing::warn!(slug, error = ?e, "numbering stamp repair failed");
    }
}

/// The stamps a repair writes to `row`, the show's store row, or `None`
/// to write nothing, noting the repair in `taken` under `key`.
fn decided(
    taken: &mut BTreeMap<(PathBuf, String), Taken>,
    key: (PathBuf, String),
    row: &Row,
    row_ep: Option<&str>,
    listing: &Listing<'_>,
    heard: Option<Hearing>,
) -> Option<Stamps> {
    let current = Stamps::of(row);
    current.display.as_ref()?;
    let prior = heard
        .and_then(|_| taken.get(&key))
        .filter(|t| t.left == current);
    let order = heard.map_or(Order::Later, |h| h.against(prior.map(|t| t.heard)));
    if order == Order::Earlier {
        return None;
    }
    let base = prior.map_or_else(|| current.clone(), |t| t.base.clone());
    let ours = repaired(&base, row.offset, row_ep, listing);
    let next = settle(order, &current, &base, ours);
    if let Some(h) = heard {
        let heard = match prior {
            Some(t) if order == Order::Overlapping => h.spanning(t.heard),
            _ => h,
        };
        let left = next.clone().unwrap_or_else(|| current.clone());
        taken.insert(key, Taken { heard, base, left });
    }
    next
}

/// What the show's history row from before rows recorded the app's own
/// number holds; `None` for a show with no such row, or a history file
/// that cannot be read, which says nothing about the row.
fn legacy_row_ep(state: &AppState, slug: &str) -> Option<String> {
    crate::history::read_all(&state.history_path)
        .ok()?
        .into_iter()
        .find(|e| e.id == slug && !e.own_number)
        .map(|e| e.ep_no)
}

#[cfg(test)]
#[path = "numbering_offset_repair_test.rs"]
mod tests;
