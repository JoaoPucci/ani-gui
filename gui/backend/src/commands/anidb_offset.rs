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

use std::path::{Path, PathBuf};

use crate::app::AppState;

/// The offsets file: a sibling of `ani-hsts`, one `slug\toffset` row
/// per stamped show.
fn store_path(history_path: &Path) -> PathBuf {
    history_path.with_file_name("ani-gui-offsets")
}

fn parse(body: &str) -> Vec<(String, u32)> {
    body.lines()
        .filter_map(|line| {
            let (slug, offset) = line.split_once('\t')?;
            if slug.is_empty() {
                return None;
            }
            Some((slug.to_string(), offset.trim().parse().ok()?))
        })
        .collect()
}

/// Serializes every put's read-merge-write sequence: concurrent
/// prefetch resolves would otherwise read the same old file,
/// independently merge their row, and overwrite one another (or
/// consume each other's temp file) — a lost stamp exposes provider
/// numbering on the home rail.
static PUT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Persist the slug's numbering offset. Best-effort — a failed write
/// degrades to the unstamped (offset 0) read, never breaks a play.
/// Last write wins, so a re-resolve can correct a stale stamp.
pub fn put(state: &AppState, slug: &str, offset: u32) {
    let path = store_path(&state.history_path);
    let _guard = PUT_LOCK.lock().expect("offset put lock");
    let write = || -> std::io::Result<()> {
        // A fresh profile reaches this write before anything has
        // created $XDG_STATE_HOME/ani-cli — the history writer only
        // runs afterwards.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = std::fs::read_to_string(&path).unwrap_or_default();
        let mut rows = parse(&body);
        match rows.iter_mut().find(|(s, _)| s == slug) {
            Some(row) => row.1 = offset,
            None => rows.push((slug.to_string(), offset)),
        }
        let mut out = String::new();
        for (s, o) in &rows {
            out.push_str(s);
            out.push('\t');
            out.push_str(&o.to_string());
            out.push('\n');
        }
        // Atomic like the history writer: a concurrent reader sees
        // the full pre- or post-state, never a half-written file.
        let tmp = path.with_extension("new");
        std::fs::write(&tmp, out)?;
        std::fs::rename(&tmp, &path)
    };
    if let Err(e) = write() {
        tracing::warn!(slug, offset, error = ?e, "anidb offset write failed");
    }
}

/// The slug's stamped numbering offset; 0 on a missing row or an
/// unreadable file — the no-shift case.
pub fn get(state: &AppState, slug: &str) -> u32 {
    let body = std::fs::read_to_string(store_path(&state.history_path)).unwrap_or_default();
    parse(&body)
        .into_iter()
        .find(|(s, _)| s == slug)
        .map_or(0, |(_, o)| o)
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
