//! Storing a title-match row: only for a title a history row carries
//! — and the retired segments a bare row's rows may sit under. Split
//! from `history_forget_titles` for the per-file complexity bar.

use crate::app::AppState;
use crate::commands::history_title_tail::without_episode_tail;
use crate::commands::kitsu::{title_match_put, RETIRED_TITLE_MATCH_SEGMENTS};
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
    provider: Option<ProviderId>,
    title: &str,
    cour: u32,
    kitsu_id: &str,
) -> Result<()> {
    store_title_match_for_year(state, provider, title, None, cour, kitsu_id)
}

/// [`store_title_match`] for a row that names `year`: kept under the
/// title keyed by it ([`crate::commands::title_match_year::keyed_title`]),
/// for a title a history row carries.
///
/// # Errors
/// History read and cache write failures propagate.
pub(crate) fn store_title_match_for_year(
    state: &AppState,
    provider: Option<ProviderId>,
    title: &str,
    year: Option<u32>,
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
        let keyed = crate::commands::title_match_year::keyed_title(title, year);
        title_match_put(state, provider, &keyed, cour, kitsu_id)
    })
}

/// Whether Continue Watching searches `title` for the row `entry`: the
/// row's title as listed or less a legacy episode tail, on the row's
/// provider, compared the way the cache key folds a title.
fn searches(entry: &HistoryEntry, provider: Option<ProviderId>, title: &str) -> bool {
    let wanted = title.trim().to_lowercase();
    ShowKey::parse(&entry.id).provider == provider
        && [entry.title.as_str(), without_episode_tail(&entry.title)]
            .iter()
            .any(|searched| searched.trim().to_lowercase() == wanted)
}

/// The last version that wrote under a retired segment: the segment
/// left with its provider before the version after it.
const LAST_RETIRED_SEGMENT_VERSION: u32 = 3;

/// The key prefixes a bare row's title matches for `normalized` may
/// also sit under: the segments retired with the provider whose rows
/// were bare ([`RETIRED_TITLE_MATCH_SEGMENTS`]), every cour to follow.
/// A row whose key names a provider has none.
pub(crate) fn retired_title_match_prefixes(
    provider: Option<ProviderId>,
    normalized: &str,
) -> Vec<String> {
    if provider.is_some() {
        return Vec::new();
    }
    (crate::commands::kitsu::FIRST_SEGMENTED_TITLE_MATCH_VERSION..=LAST_RETIRED_SEGMENT_VERSION)
        .flat_map(|version| {
            RETIRED_TITLE_MATCH_SEGMENTS
                .iter()
                .map(move |segment| format!("title-match:v{version}:{segment}:{normalized}:c"))
        })
        .collect()
}
