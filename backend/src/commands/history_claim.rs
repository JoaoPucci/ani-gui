//! Whether a history row is the one a Kitsu entry resumes from —
//! split from `history` for the per-file complexity bar.

use crate::app::AppState;
use crate::error::Result;
use crate::history::HistoryEntry;

/// What is known of a row's Kitsu identity.
pub(crate) struct RowIds<'a> {
    /// The id the row recorded, and whether Kitsu answered it gone.
    pub recorded: Option<(&'a str, bool)>,
    /// The row's stored `(show id → kitsu_id)` mapping, and whether
    /// Kitsu answered that id gone.
    pub mapped: Option<(&'a str, bool)>,
    /// The ids the title-match rows stored for the row's title name.
    pub title_matched: &'a [String],
}

/// Whether the row names `kitsu_id` (Kitsu has not answered it gone:
/// `kitsu_gone`). The id the row recorded decides alone while it
/// stands. A row that records none — or records one Kitsu deleted —
/// is named by its mapping while that stands. A row whose recorded
/// entry was deleted, with no standing mapping, is named by the title
/// match the home page stored for it, the entry it is now shown as; a
/// row that never recorded an id is not, since that would be the
/// guess the mapping is there to stand in for.
pub(crate) fn names(kitsu_id: &str, kitsu_gone: bool, row: &RowIds<'_>) -> bool {
    if kitsu_gone {
        return false;
    }
    if let Some((recorded, false)) = row.recorded {
        return recorded == kitsu_id;
    }
    if let Some((mapped, false)) = row.mapped {
        return mapped == kitsu_id;
    }
    row.recorded.is_some() && row.title_matched.iter().any(|t| t == kitsu_id)
}

/// [`names`], over what the cache knows of `entry`.
///
/// # Errors
/// SQLite read failures propagate.
pub(crate) fn row_names(state: &AppState, entry: &HistoryEntry, kitsu_id: &str) -> Result<bool> {
    let gone = |id: &str| super::kitsu_gone::is_gone(state, id);
    let recorded = match entry.kitsu_id.as_deref() {
        Some(id) => Some((id, gone(id)?)),
        None => None,
    };
    if let Some((id, false)) = recorded {
        return Ok(id == kitsu_id);
    }
    let mapped_id = super::kitsu::allmanga_kitsu_get(state, &entry.id)?;
    let mapped = match mapped_id.as_deref() {
        Some(id) => Some((id, gone(id)?)),
        None => None,
    };
    let title_matched = if recorded.is_some() {
        super::history_forget_titles::title_match_ids(state, &entry.id, &entry.title)?
    } else {
        Vec::new()
    };
    let row = RowIds {
        recorded,
        mapped,
        title_matched: &title_matched,
    };
    Ok(names(kitsu_id, gone(kitsu_id)?, &row))
}
