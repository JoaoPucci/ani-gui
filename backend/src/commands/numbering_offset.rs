//! Provider-numbering offset store — the bridge between the two
//! numbering spaces the history file has to span.
//!
//! A provider can carry a franchise's cumulative episode count into
//! continuation cours — anidb.app listed TYBW's fourth part as 41 and
//! 42 — and a resume looks the stored `ep_no` up in that provider's
//! list, so the history file speaks PROVIDER numbering. Every GUI
//! surface counts per-entry like Kitsu. The resolver computes the shift
//! once per show; this module persists it keyed by show key so the
//! history writers can add it and the read boundary can subtract it.
//! The rows anidb.app's resolves stamped stay: they are what keeps the
//! history rows from its days readable.
//! A show the GUI never resolved has no stamp and reads as offset 0,
//! which is exactly today's behavior.
//!
//! The store is a TSV file BESIDE the history file, not a cache row:
//! the diagnostics clear (or a deleted XDG cache dir) wipes the
//! database outright, and the translation has to outlive that — it
//! must live exactly as long as the rows it makes readable.

use crate::app::AppState;

#[path = "numbering_offset_store.rs"]
mod numbering_offset_store;
use numbering_offset_store::{merge_row, remove_rows};

#[cfg(test)]
#[path = "numbering_offset_read_tap.rs"]
pub mod read_tap;

#[path = "numbering_offset_snapshot.rs"]
mod snapshot;
pub use snapshot::Numbering;

#[path = "numbering_offset_former.rs"]
mod former;
pub use former::{
    former_stamp, put_watched_display, retag_display, retag_from_listing, retag_from_resolve,
    stamp_beside,
};

/// Persist the slug's numbering offset. Best-effort — a failed write
/// degrades to the unstamped (offset 0) read, never breaks a play.
/// Last write wins, so a re-resolve can correct a stale stamp.
pub fn put(state: &AppState, slug: &str, offset: u32) {
    merge_row(state, slug, offset, None);
}

/// Persist the slug's offset together with the last watch's
/// (slot, display tag) pair — written when a native resolve lands
/// on a row whose display differs from its slot, so the history can
/// carry the slot a resume needs while the read boundary translates
/// it back for display. Last write wins, like the offset.
pub fn put_display(state: &AppState, slug: &str, offset: u32, slot: u32, tag: &str) {
    merge_row(state, slug, offset, Some((slot, tag.to_string())));
}

/// Drop the rows of the slugs `removed` names, with the history rows
/// they made readable. Every other row stays, a show's offset stamped
/// by a resolve before it has a history row among them.
///
/// # Errors
/// The store's read or write failure. A missing store reads as empty;
/// one that exists but cannot be read is left as it is.
pub fn forget(state: &AppState, removed: &std::collections::HashSet<&str>) -> std::io::Result<()> {
    remove_rows(state, |r| removed.contains(r.slug.as_str()))
}

/// The slug's stamped (slot, display tag) pair, when one exists.
pub fn display_stamp(state: &AppState, slug: &str) -> Option<(u32, String)> {
    Numbering::read(&state.history_path).display(slug)
}

/// The slug's stamped numbering offset; 0 on a missing row or an
/// unreadable file — the no-shift case.
pub fn get(state: &AppState, slug: &str) -> u32 {
    Numbering::read(&state.history_path).offset(slug)
}

/// The per-entry (Kitsu-space) reading of a stored row: the stamped
/// slot presents as its display tag translated per-entry; everything
/// else keeps the plain offset translation.
pub fn read_ep_no(state: &AppState, slug: &str, ep_no: &str) -> String {
    Numbering::read(&state.history_path).read_ep_no(slug, ep_no)
}

/// Kitsu-relative episode → the provider's number, for history
/// writes. Non-numeric input passes through unchanged (defensive —
/// the play paths only ever see digits).
#[must_use]
pub fn provider_ep_no(episode: &str, offset: u32) -> String {
    match episode.trim().parse::<u32>() {
        Ok(n) => n.saturating_add(offset).to_string(),
        // Fractional tags ride the same translation the resolve
        // uses; non-numeric strings pass through.
        Err(_) => super::play_native_numbering::provider_fraction(episode.trim(), offset),
    }
}

/// The provider's number → Kitsu-relative, for the GUI's history
/// read boundary. A number at or below the offset means the stamp
/// doesn't describe this row (a stale or foreign entry) — it passes
/// through unchanged rather than collapsing to zero.
#[must_use]
pub fn kitsu_ep_no(ep_no: &str, offset: u32) -> String {
    match ep_no.trim().parse::<u32>() {
        Ok(n) if n > offset => (n - offset).to_string(),
        Ok(_) => ep_no.to_string(),
        // per_entry_fraction passes a fraction below the offset
        // through; one at the offset is the recap before episode 1.
        Err(_) => super::play_native_numbering::per_entry_fraction(ep_no.trim(), offset),
    }
}

#[cfg(test)]
#[path = "numbering_offset_test.rs"]
mod tests;
