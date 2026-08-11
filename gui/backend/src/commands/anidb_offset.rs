//! Provider-numbering offset store — the bridge between the two
//! numbering spaces that share `ani-hsts`.
//!
//! anidb.app carries a franchise's cumulative episode count into
//! continuation cours (TYBW's fourth part lists 41 and 42), and
//! `ani-cli`'s `process_hist_entry` greps the stored `ep_no` in that
//! provider list — so the shared history file speaks PROVIDER
//! numbering. Every GUI surface counts per-entry like Kitsu. The
//! resolver computes the shift once per show; this module persists it
//! keyed by slug so the history writers can add it and the read
//! boundary can subtract it — for GUI- and CLI-written rows alike.
//! A show the GUI never resolved has no stamp and reads as offset 0,
//! which is exactly today's behavior.
//!
//! The store is a TSV file BESIDE the history file, not a cache row:
//! debug and packaged builds keep separate cache databases while
//! `ani-hsts` stays shared, and the diagnostics clear (or a deleted
//! XDG cache dir) wipes the database outright — the translation must
//! live exactly as long, and exactly as shared, as the rows it makes
//! readable.

use crate::app::AppState;

#[path = "anidb_offset_store.rs"]
mod anidb_offset_store;
use anidb_offset_store::{merge_row, parse, store_path};

/// Persist the slug's numbering offset. Best-effort — a failed write
/// degrades to the unstamped (offset 0) read, never breaks a play.
/// Last write wins, so a re-resolve can correct a stale stamp.
pub fn put(state: &AppState, slug: &str, offset: u32) {
    merge_row(state, slug, offset, None);
}

/// Persist the slug's offset together with the last watch's
/// (slot, display tag) pair — written when a native resolve lands
/// on a row whose display differs from its slot, so the shared
/// history can carry the slot the CLI greps while the GUI translates
/// it back. Last write wins, like the offset.
pub fn put_display(state: &AppState, slug: &str, offset: u32, slot: u32, tag: &str) {
    merge_row(state, slug, offset, Some((slot, tag.to_string())));
}

/// The slug's stamped (slot, display tag) pair, when one exists.
pub fn display_stamp(state: &AppState, slug: &str) -> Option<(u32, String)> {
    let body = std::fs::read_to_string(store_path(&state.history_path)).unwrap_or_default();
    parse(&body).into_iter().find(|r| r.slug == slug)?.display
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

/// The ep_no a listing-less writer (mark-watched, cache-hit) should
/// store for `episode`: the offset translation, except when the
/// provider-space value names the stamped display tag — by numeric
/// identity, since the frontend normalizes — in which case the
/// CLI-greppable slot is written instead.
pub fn write_ep_no(state: &AppState, slug: &str, episode: &str, offset: u32) -> String {
    let provider = provider_ep_no(episode, offset);
    if let Some((slot, tag)) = display_stamp(state, slug) {
        if super::play_native_episode::tag_matches(&tag, &provider) {
            return slot.to_string();
        }
    }
    provider
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

/// Kitsu-relative episode → the provider's number, for ani-hsts
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
#[path = "anidb_offset_test.rs"]
mod tests;
