//! Storing a title-match row: only for a title a history row carries
//! — split from `history_forget_titles` for the per-file complexity
//! bar.

use crate::app::AppState;
use crate::commands::history_title_tail::without_episode_tail;
use crate::commands::kitsu::title_match_put;
use crate::error::Result;
use crate::history::HistoryEntry;
use crate::scraper::provider::{ProviderId, ShowKey};

/// Store a title-match row, with the history held — for a title a
/// history row carries. Continue Watching stores one when a row's
/// search settles, and the user can remove the row before it does;
/// with no row searching that title there is nothing to match, and
/// storing it would bring back what the removal took. Nor is one
/// stored for something that is not a Kitsu id.
///
/// # Errors
/// History read and cache write failures propagate.
pub(crate) fn store_title_match(
    state: &AppState,
    provider: ProviderId,
    title: &str,
    cour: u32,
    kitsu_id: &str,
) -> Result<()> {
    let Some(kitsu_id) = crate::history::kitsu_id_in(kitsu_id) else {
        return Ok(());
    };
    crate::history::guard::hold(&state.history_path, |held| {
        if !held.rows()?.iter().any(|e| searches(e, provider, title)) {
            return Ok(());
        }
        title_match_put(state, provider, title, cour, kitsu_id)
    })
}

/// Whether Continue Watching searches `title` for the row `entry`: the
/// row's title as listed or less a legacy episode tail, on the row's
/// provider, compared the way the cache key folds a title.
fn searches(entry: &HistoryEntry, provider: ProviderId, title: &str) -> bool {
    let wanted = title.trim().to_lowercase();
    ShowKey::parse(&entry.id).provider == provider
        && [entry.title.as_str(), without_episode_tail(&entry.title)]
            .iter()
            .any(|searched| searched.trim().to_lowercase() == wanted)
}
