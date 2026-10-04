//! Whether a play stored a show's reverse mapping.
//!
//! A watch stamps the show (`watched-at:v1:`) and stores its
//! `show_id → kitsu_id` mapping in the same moment. A resolve that
//! guesses from a slug's words stores the mapping too, but leaves the
//! stamp alone. So a mapping written beside the show's stamp is one a
//! play stored, and one written apart from it, or with no stamp at
//! all, is a guess. Nothing new is stored: both moments are already in
//! the cache.
//!
//! Rows that record the Kitsu id of the show played never need this.
//! It serves rows written before history recorded it.

use crate::app::AppState;
use crate::cache::meta_cache_fetched_at;
use crate::commands::kitsu::{allmanga_kitsu_key, watched_at_get};
use crate::error::Result;

/// How long after a watch's stamp its mapping may be written and still
/// be one play. Builds before the history recorded the Kitsu id —
/// the only ones whose rows this reads — stamped the watch, then read
/// the Kitsu entry the play page had already cached to guard the
/// write, then stored the mapping: seconds apart. A guess a Continue
/// load stores after a watch that stored no mapping is read as played
/// only if it lands inside this window.
const ONE_PLAY_MS: i64 = 10_000;

/// The cache writes in whole seconds, so a mapping written in the same
/// second as its stamp can read up to a second before it.
const SECOND_MS: i64 = 1_000;

/// Whether the show's stored mapping was written by a play.
///
/// # Errors
/// Cache I/O errors propagate.
pub(crate) fn mapping_played(state: &AppState, show_id: &str) -> Result<bool> {
    let Some(stamp_ms) = watched_at_get(state, show_id)? else {
        return Ok(false);
    };
    let Some(mapped_s) = meta_cache_fetched_at(&state.cache_pool, &allmanga_kitsu_key(show_id))?
    else {
        return Ok(false);
    };
    let gap_ms = mapped_s.saturating_mul(1000) - stamp_ms;
    Ok((-SECOND_MS..=ONE_PLAY_MS).contains(&gap_ms))
}

/// Store a resolve's guess as the show's mapping, with the history
/// held. Two things keep it out:
///
/// - A mapping a play stored is the show the user played; the guess
///   answers its request and leaves that mapping standing.
/// - The guess is for a history row. Continue Watching asks for it
///   while a row resolves, and the user can remove the row before the
///   answer is back; with no row for the show there is nothing to map,
///   and storing it would bring back what the removal took.
///
/// # Errors
/// History read, cache read and cache write failures propagate: a
/// mapping whose moment cannot be read is not known to be a guess.
pub(crate) fn store_guess(state: &AppState, show_id: &str, kitsu_id: &str) -> Result<()> {
    crate::history::guard::hold(&state.history_path, |held| {
        let listed = held.rows()?.iter().any(|e| e.id == show_id);
        if !listed || mapping_played(state, show_id)? {
            return Ok(());
        }
        crate::commands::kitsu::allmanga_kitsu_put(state, show_id, kitsu_id)
    })
}

#[cfg(test)]
#[path = "kitsu_played_test.rs"]
mod tests;
