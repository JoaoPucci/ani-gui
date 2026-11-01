//! Putting a show's numbering stamps right from a listing at hand, in
//! the order the listings were heard. Split from [`super`] for the
//! per-file complexity bar.
//!
//! Two walks of a show can overlap — a play and a probe, two probes —
//! and the one that heard its listing first can finish, and repair,
//! last. Each repair carries its listing's hearing
//! ([`crate::commands::listing_tags::Hearing`]), and the stamps end as
//! the listings heard last repair them, whatever order the repairs
//! write in:
//! - every repair is made from the stamps as they stood before this
//!   run's first repair, so what an earlier listing decided — a former
//!   tag it found free, a slot it moved the stamp to — does not outlive
//!   a listing heard after it that decides otherwise;
//! - a repair whose listing was heard wholly before another's has no
//!   say once that one is taken, and writes nothing after it;
//! - a listing that repairs nothing takes no part, and leaves the stamps
//!   to the listings that do, heard before or after;
//! - a former tag any listing at hand this run carries is not kept,
//!   repairing or not, heard before or after: a position kept under it
//!   may be that row's;
//! - the repairs left, heard alongside one another, cannot be ordered:
//!   a former tag only some of them name is not named, as another's
//!   listing may give it to a row of its own, and where they put the
//!   display stamp in different places it stays as it was before this
//!   run's repairs.
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
use crate::commands::listing_tags::Hearing;

#[path = "numbering_offset_repair_rule.rs"]
mod rule;
use rule::{repaired, Stamps};
pub(super) use rule::{Carrier, Listing};

#[path = "numbering_offset_repair_record.rs"]
mod record;
#[cfg(test)]
use record::decided;
use record::{settled, Taken};
#[cfg(test)]
use rule::standing;

/// The repairs taken this run, by history file and show key. Read and
/// written inside the store's locks, as a repair decides.
static TAKEN: Mutex<BTreeMap<(PathBuf, String), Taken>> = Mutex::new(BTreeMap::new());

/// Repair the stamps of the show `slug` keys from `listing`, heard as
/// `heard` says; `None` for a repair that knows no hearing, which is
/// ordered against nothing and leaves nothing for another to be.
/// Decided under the store's locks, against the row as it stands there.
pub(super) fn repair(state: &AppState, slug: &str, listing: &Listing<'_>, heard: Option<Hearing>) {
    repair_carried(state, slug, listing, heard, &Carrier::Tags(Vec::new()));
}

/// [`repair`], from a listing that carries the rows `carrier` names: a
/// former tag the stamps keep, which it or any other listing at hand
/// this run carries, is dropped, since a position kept under it may be
/// that row's.
pub(super) fn repair_carried(
    state: &AppState,
    slug: &str,
    listing: &Listing<'_>,
    heard: Option<Hearing>,
    carrier: &Carrier,
) {
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
        let mut taken = TAKEN.lock().unwrap_or_else(PoisonError::into_inner);
        let key = (state.history_path.clone(), slug.to_string());
        // A stamp that is not the one the unlocked read found, nor the
        // one this run's last repair left, is left as it is. A build from
        // before the marker, running beside this one on the same files,
        // stamps a play before it writes the play's row, outside these
        // locks; a stamp that moved meanwhile may have that row on its
        // way, and the next listing at hand repairs a stamp that has
        // none. A stamp another repair here moved is the order's to
        // decide.
        let ours_last = |r: &Row| taken.get(&key).is_some_and(|t| t.left == Stamps::of(r));
        let Some(row) = rows
            .iter_mut()
            .find(|r| r.slug == slug && (r.display.as_ref() == Some(&found) || ours_last(r)))
        else {
            return false;
        };
        // The history row as it stands under the store's locks, read
        // without the history guard so a repair inside
        // `stamp_numbering`'s hold cannot deadlock.
        let row_ep = listing.rows.and_then(|_| legacy_row_ep(state, slug));
        let offset = row.offset;
        let ours = |base: &Stamps| repaired(base, offset, row_ep.as_deref(), listing);
        let current = Stamps::of(row);
        let next = settled(&mut taken, key, &current, heard, carrier, ours);
        if next == current {
            return false;
        }
        row.display = next.display;
        row.former = next.former;
        true
    };
    // A write that fails leaves the record of repairs taken naming
    // stamps the store does not hold: the next repair finds the stamps
    // changed, and starts over from them.
    if let Err(e) = rewrite_if(state, decide) {
        tracing::warn!(slug, error = ?e, "numbering stamp repair failed");
    }
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
