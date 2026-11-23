//! Warming a Continue Watching strip's AniList fills at once (stub).

use crate::app::AppState;

/// Stub: warms nothing.
pub(crate) async fn warm_strip(
    _state: &AppState,
    _kitsu_ids: &[String],
    _anilist_base: Option<&str>,
) {
}

#[cfg(test)]
#[path = "strip_warm_test.rs"]
mod tests;
