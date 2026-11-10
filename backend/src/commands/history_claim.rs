//! Whether a history row is the one a Kitsu entry resumes from —
//! split from `history` for the per-file complexity bar.

use crate::app::AppState;
use crate::error::Result;
use crate::history::{kitsu_id_in, same_kitsu_id, HistoryEntry};

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
        return same_kitsu_id(recorded, kitsu_id);
    }
    if let Some((mapped, false)) = row.mapped {
        return same_kitsu_id(mapped, kitsu_id);
    }
    row.recorded.is_some() && row.title_matched.iter().any(|t| same_kitsu_id(t, kitsu_id))
}

/// What the cache knows of one row's Kitsu identity: read once per
/// row, it answers [`names`] for any id ([`RowIdentity::names`]).
pub(crate) struct RowIdentity {
    recorded: Option<(String, bool)>,
    mapped: Option<(String, bool)>,
    title_matched: Vec<String>,
}

impl RowIdentity {
    /// The identity of a row with these parts, keeping only what
    /// [`names`] reads: a recorded id that stands decides alone, so the
    /// mapping and title matches are dropped for it, and a row that
    /// recorded no id is never named by a title match.
    fn of(
        recorded: Option<(String, bool)>,
        mapped: impl FnOnce() -> Result<Option<(String, bool)>>,
        title_matched: impl FnOnce() -> Result<Vec<String>>,
    ) -> Result<Self> {
        if let Some((_, false)) = recorded {
            return Ok(Self {
                recorded,
                mapped: None,
                title_matched: Vec::new(),
            });
        }
        let mapped = mapped()?;
        let title_matched = if recorded.is_some() {
            title_matched()?
        } else {
            Vec::new()
        };
        Ok(Self {
            recorded,
            mapped,
            title_matched,
        })
    }

    /// [`names`] for `kitsu_id`, which Kitsu answered gone or not.
    pub(crate) fn names(&self, kitsu_id: &str, kitsu_gone: bool) -> bool {
        let row = RowIds {
            recorded: side(&self.recorded),
            mapped: side(&self.mapped),
            title_matched: &self.title_matched,
        };
        names(kitsu_id, kitsu_gone, &row)
    }
}

fn side(o: &Option<(String, bool)>) -> Option<(&str, bool)> {
    o.as_ref().map(|(id, gone)| (id.as_str(), *gone))
}

/// Whether Kitsu answered `id` gone. A gone mark is kept under digits;
/// a value that is not an id has none.
///
/// # Errors
/// SQLite read failures propagate.
pub(crate) fn id_gone(state: &AppState, id: &str) -> Result<bool> {
    kitsu_id_in(id).map_or(Ok(false), |id| super::kitsu_gone::is_gone(state, id))
}

/// What the cache knows of `entry`. A recorded id that stands decides
/// alone, so nothing else is read for it.
///
/// # Errors
/// SQLite read failures propagate.
pub(crate) fn row_identity(state: &AppState, entry: &HistoryEntry) -> Result<RowIdentity> {
    let recorded = match entry.kitsu_id.as_deref() {
        Some(id) => Some((id.to_owned(), id_gone(state, id)?)),
        None => None,
    };
    RowIdentity::of(
        recorded,
        || match super::kitsu::allmanga_kitsu_get(state, &entry.id)? {
            Some(id) => {
                let gone = id_gone(state, &id)?;
                Ok(Some((id, gone)))
            }
            None => Ok(None),
        },
        || super::history_forget_titles::current_title_match_ids(state, &entry.id, &entry.title),
    )
}

#[cfg(test)]
#[path = "history_claim_test.rs"]
mod tests;
