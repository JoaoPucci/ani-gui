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

/// Persist the slug's numbering offset in the dedicated table —
/// NOT meta_cache: history rows live indefinitely, so the offset
/// must survive TTL expiry and the diagnostics cache clear.
/// Best-effort — a failed write degrades to the unstamped (offset 0)
/// read, never breaks a play. Last write wins, so a re-resolve can
/// correct a stale stamp.
pub fn put(state: &AppState, slug: &str, offset: u32) {
    let write = || -> Result<(), rusqlite::Error> {
        let conn = state
            .cache_pool
            .get()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "INSERT INTO anidb_offsets (slug, ep_offset) VALUES (?1, ?2)
             ON CONFLICT(slug) DO UPDATE SET ep_offset = excluded.ep_offset",
            rusqlite::params![slug, offset],
        )?;
        Ok(())
    };
    if let Err(e) = write() {
        tracing::warn!(slug, offset, error = ?e, "anidb offset write failed");
    }
}

/// The slug's stamped numbering offset; 0 on a missing or unreadable
/// row — the no-shift case.
pub fn get(state: &AppState, slug: &str) -> u32 {
    let Ok(conn) = state.cache_pool.get() else {
        return 0;
    };
    conn.query_row(
        "SELECT ep_offset FROM anidb_offsets WHERE slug = ?1",
        rusqlite::params![slug],
        |row| row.get::<_, u32>(0),
    )
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
