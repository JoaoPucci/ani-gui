//! Provider-numbering offset store — the bridge that keeps history rows
//! from before rows recorded the app's own number readable.
//!
//! A provider can carry a franchise's cumulative episode count into
//! continuation cours — anidb.app listed TYBW's fourth part as 41 and
//! 42 — and rows written then hold the provider's number for the
//! episode: the slot a watch resolved to, or, for a write with no
//! listing at hand, the display number plus the offset. Every GUI
//! surface counts per-entry like Kitsu, and every row written now
//! records that number itself, marked as such
//! ([`crate::history::HistoryEntry::own_number`]). For the rows from
//! before, the resolver's shift is persisted per show key, with the
//! (slot, tag) display stamp the last displaced resolve left — warms
//! included, so a row past a recap whose stamp a warm moved before
//! this build reads one episode high until its show is next watched —
//! and the read boundary subtracts and translates through them. A
//! resolve stamps the offset only for a show with no row here, and no
//! longer stamps the displaced row it lands on, so neither moves to
//! another row under such a row.
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
use numbering_offset_store::{merge_row, parse, remove_rows, store_path};

/// Persist the slug's numbering offset. Best-effort — a failed write
/// degrades to the unstamped (offset 0) read, never breaks a play.
/// Last write wins; resolves call it only for a show with no row yet.
pub fn put(state: &AppState, slug: &str, offset: u32) {
    merge_row(state, slug, offset, None);
}

/// Test setup only: no resolve stamps the row it lands on now. Persist the
/// slug's offset together with a (slot, display tag) pair, the stamp
/// a displaced watch left before rows recorded the app's own number.
/// Last write wins, like the offset.
#[cfg(test)]
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
    let body = std::fs::read_to_string(store_path(&state.history_path)).unwrap_or_default();
    parse(&body).into_iter().find(|r| r.slug == slug)?.display
}

/// Whether the slug has a row in the store at all.
pub fn has(state: &AppState, slug: &str) -> bool {
    let body = std::fs::read_to_string(store_path(&state.history_path)).unwrap_or_default();
    parse(&body).iter().any(|r| r.slug == slug)
}

/// The slug's stamped numbering offset; 0 on a missing row or an
/// unreadable file — the no-shift case.
pub fn get(state: &AppState, slug: &str) -> u32 {
    let body = std::fs::read_to_string(store_path(&state.history_path)).unwrap_or_default();
    parse(&body)
        .into_iter()
        .find(|r| r.slug == slug)
        .map_or(0, |r| r.offset)
}

/// The per-entry (Kitsu-space) reading of a stored row: the stamped
/// slot presents as its display tag translated per-entry; everything
/// else keeps the plain offset translation.
pub fn read_ep_no(state: &AppState, slug: &str, ep_no: &str) -> String {
    let offset = get(state, slug);
    if let Some((slot, tag)) = display_stamp(state, slug) {
        if ep_no.trim() == slot.to_string() {
            return kitsu_ep_no(&tag, offset);
        }
    }
    kitsu_ep_no(ep_no, offset)
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
        // per_entry_fraction keeps its own at-or-below-offset
        // pass-through, mirroring the integer rule.
        Err(_) => super::play_native_numbering::per_entry_fraction(ep_no.trim(), offset),
    }
}

#[cfg(test)]
#[path = "numbering_offset_test.rs"]
mod tests;
