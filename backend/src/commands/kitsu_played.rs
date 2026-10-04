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

/// How far apart a watch's stamp and its mapping may be written and
/// still be one play. The mapping follows the stamp after a Kitsu
/// detail read that guards the write, so the gap is that read's time.
const ONE_PLAY_MS: i64 = 60_000;

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
    Ok((mapped_s.saturating_mul(1000) - stamp_ms).abs() <= ONE_PLAY_MS)
}
