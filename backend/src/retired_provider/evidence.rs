//! Which Kitsu entries a history row stands for, as the sweep weighs
//! them — split from [`super::history`] for the per-file complexity
//! bar.
//!
//! The two sides are read asymmetrically, each in the direction that
//! can only keep a row. A walked row stands for an entry only when it
//! recorded it and Kitsu has not answered it gone: then Continue
//! Watching shows it as that entry and the detail page's resume lookup
//! finds it by it, so it can take a retired row's place on both. A
//! guess — a mapping, a title match — makes neither surface certain.
//! A retired row stands for every entry anything names for it: its
//! recorded id, its mapping, every title match stored for its title.
//! More entries there only mean more a walked row must cover before
//! the retired row goes.

use crate::app::AppState;
use crate::commands::kitsu::{allmanga_kitsu_get, title_match_prefix};
use crate::error::Result;
use crate::history::{kitsu_id_in, HistoryEntry};
use crate::scraper::provider::ShowKey;

/// The entry a walked row recorded, unless Kitsu answered it gone.
pub(super) fn walked(state: &AppState, entry: &HistoryEntry) -> Result<Vec<String>> {
    let Some(id) = entry.kitsu_id.as_deref().and_then(kitsu_id_in) else {
        return Ok(Vec::new());
    };
    if crate::commands::kitsu_gone::is_gone(state, id)? {
        return Ok(Vec::new());
    }
    Ok(vec![id.to_string()])
}

/// Every entry anything names for a retired row.
pub(super) fn retired(state: &AppState, entry: &HistoryEntry) -> Result<Vec<String>> {
    let mut ids: Vec<String> = entry
        .kitsu_id
        .as_deref()
        .and_then(kitsu_id_in)
        .map(str::to_string)
        .into_iter()
        .collect();
    ids.extend(allmanga_kitsu_get(state, &entry.id)?);
    let provider = ShowKey::parse(&entry.id).provider;
    for searched in searched_titles(entry) {
        let prefix = title_match_prefix(provider, searched);
        for (key, body) in crate::cache::meta_cache_list_prefix(&state.cache_pool, &prefix)? {
            if is_cour(&key[prefix.len()..]) {
                ids.push(body);
            }
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// The titles Continue Watching searches a row by: as written, and
/// without a legacy "(N episodes)" tail.
pub(super) fn searched_titles(entry: &HistoryEntry) -> [&str; 2] {
    [
        entry.title.as_str(),
        crate::commands::history_title_tail::without_episode_tail(&entry.title),
    ]
}

/// Whether what follows a title-match prefix is a cour number.
pub(super) fn is_cour(rest: &str) -> bool {
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}
