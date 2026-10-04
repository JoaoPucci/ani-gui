//! The skip times a history row's show left, found by Kitsu id —
//! split from `history_forget` for the per-file complexity bar.

use crate::app::AppState;
use crate::cache::meta_cache_delete_prefix;
use crate::error::Result;

/// Delete the skip times cached under each of `kitsu_ids`, and every
/// row of the key that carried the MAL id alone, which nothing reads
/// any more. A Kitsu id is digits, so the colon after it keeps one id
/// from matching another it starts.
///
/// # Errors
/// Cache write failures propagate.
pub(crate) fn forget_skip_times(state: &AppState, kitsu_ids: &[String]) -> Result<()> {
    for id in kitsu_ids.iter().filter(|id| !id.is_empty()) {
        meta_cache_delete_prefix(&state.cache_pool, &format!("aniskip:v2:{id}:"))?;
    }
    meta_cache_delete_prefix(&state.cache_pool, "aniskip:v1:")
}
