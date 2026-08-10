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

use crate::app::AppState;
use crate::cache::{meta_cache_get, meta_cache_put};

const OFFSET_PREFIX: &str = "anidb-offset:v1:";

/// Offsets are as stable as the provider's numbering itself; re-puts
/// on every resolve keep the row fresh.
const OFFSET_TTL_SECS: u64 = 60 * 60 * 24 * 30;

fn key(slug: &str) -> String {
    format!("{OFFSET_PREFIX}{slug}")
}

/// Persist the slug's numbering offset. Best-effort — a failed write
/// degrades to the unstamped (offset 0) read, never breaks a play.
pub fn put(state: &AppState, slug: &str, offset: u32) {
    if let Err(e) = meta_cache_put(
        &state.cache_pool,
        &key(slug),
        &offset.to_string(),
        OFFSET_TTL_SECS,
    ) {
        tracing::warn!(slug, offset, error = ?e, "anidb offset write failed");
    }
}

/// The slug's stamped numbering offset; 0 on a missing or unreadable
/// row — the no-shift case.
pub fn get(state: &AppState, slug: &str) -> u32 {
    meta_cache_get(&state.cache_pool, &key(slug))
        .ok()
        .flatten()
        .and_then(|body| body.trim().parse().ok())
        .unwrap_or(0)
}

/// Kitsu-relative episode → the provider's number, for ani-hsts
/// writes. Non-numeric input passes through unchanged (defensive —
/// the play paths only ever see digits).
#[must_use]
pub fn provider_ep_no(episode: &str, offset: u32) -> String {
    match episode.trim().parse::<u32>() {
        Ok(n) => n.saturating_add(offset).to_string(),
        Err(_) => episode.to_string(),
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
        _ => ep_no.to_string(),
    }
}

#[cfg(test)]
#[path = "anidb_offset_test.rs"]
mod tests;
