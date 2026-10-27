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
//! recorded id, its mapping, every title match stored for its title
//! under any version or segment the detail page reads it by.
//! More entries there only mean more a walked row must cover before
//! the retired row goes.

use crate::app::AppState;
use crate::commands::kitsu::allmanga_kitsu_get;
use crate::error::Result;
use crate::history::{kitsu_id_in, HistoryEntry};

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
    // Every title match stored for the row's title, the ones Continue
    // Watching reads and the ones the detail page reads for a row whose
    // recorded entry is gone alike: every version and cour, expired
    // ones and retired segments included.
    ids.extend(crate::commands::history_forget_titles::title_match_ids(
        state,
        &entry.id,
        &entry.title,
    )?);
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Whether what follows a title-match prefix is a cour number.
pub(super) fn is_cour(rest: &str) -> bool {
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}
