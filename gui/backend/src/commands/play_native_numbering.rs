//! Per-entry vs provider episode numbering — split from
//! `play_native_resolve` so each file stays inside the complexity
//! ratchet's per-file bar.

/// The shift between the provider's episode numbers and the
/// per-entry numbers every caller requests. anidb.app keeps a
/// franchise's cumulative count on continuation cours — TYBW's
/// fourth part lists episodes 41 and 42 — while Kitsu (and so every
/// number the UI can send) restarts each entry at 1. An entry whose
/// first listed number is above 1 is such a continuation; entries
/// starting at 0 or 1 already number per-entry and shift nothing.
pub fn numbering_offset(episodes: &[crate::scraper::anidb::EpisodeRef]) -> u32 {
    match episodes.iter().map(|e| e.number).min() {
        Some(first) if first > 1 => first - 1,
        _ => 0,
    }
}

/// The entry's highest listed episode in per-entry (Kitsu) numbering
/// — what availability caps and the play response's `episode_cap`
/// must report, or a continuation cour's raw provider numbers unlock
/// episodes that don't exist.
pub fn kitsu_episode_cap(episodes: &[crate::scraper::anidb::EpisodeRef]) -> Option<u32> {
    let offset = numbering_offset(episodes);
    episodes.iter().map(|e| e.number).max().map(|m| m - offset)
}
