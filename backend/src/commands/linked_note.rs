//! Which Kitsu id a history row records because the user named it.
//!
//! A link ([`crate::commands::history_link`]) writes the show the user
//! named in a row's `kitsu:` column. A later watch that names that same
//! id still runs the cour guard against the stream's provider title, and
//! a refusal clears an id the row held that is the refused one
//! (`settle_refused_id`). For an id an earlier, guardless build left on
//! the row that is the point. For one the user linked it is not: the
//! link is the user overruling what the provider's title suggests, and
//! clearing it turns the card back into a guess. The linked card's
//! Continue click names the link, and so do a guessed session's later
//! episodes once it was linked from the player.
//!
//! So a link notes, beside each row it linked, the id it gave it, and a
//! refused watch leaves that id on the row. The note is a cache row
//! (`history-linked:v1:<row>` → the id), as `guessed_from`'s is, for the same
//! reason: builds before this one read the history file and never ask
//! for it. Undoing the link gives each row it restored the note an
//! earlier link left on it, or none, and removing a row or clearing the
//! history takes the note with the row's other cache rows. A row another id is written to since (a play from another
//! show's page) no longer records the noted id, so the note says
//! nothing of it. A lost note only lets the guard clear the id again,
//! as it did before the note existed.

use std::time::Duration;

use crate::app::AppState;
use crate::cache::families::LINKED;
use crate::cache::{meta_cache_delete, meta_cache_get, meta_cache_put};
use crate::error::Result;

/// How long a note lasts. A link is meant for as long as the row
/// stays, and the cache has no unbounded entry.
const TTL: Duration = Duration::from_secs(10 * 365 * 24 * 60 * 60);

/// Whether the id a row records, `previous`, is the one the user linked
/// it to: then a refused watch leaves it on the row.
#[must_use]
pub fn stands(previous: Option<&str>, linked: Option<&str>) -> bool {
    matches!((previous, linked), (Some(p), Some(l)) if crate::history::same_kitsu_id(p, l))
}

/// The cache key of the note for `row`.
pub(crate) fn key(row: &str) -> String {
    format!("{LINKED}{row}")
}

/// Note that the user linked `row` to `kitsu_id`.
///
/// # Errors
/// SQLite errors propagate.
pub(crate) fn record(state: &AppState, row: &str, kitsu_id: &str) -> Result<()> {
    meta_cache_put(&state.cache_pool, &key(row), kitsu_id, TTL.as_secs())
}

/// Drop the note for `row`.
///
/// # Errors
/// SQLite errors propagate.
pub(crate) fn forget(state: &AppState, row: &str) -> Result<()> {
    meta_cache_delete(&state.cache_pool, &key(row))
}

/// The id the user linked `row` to, if a note says so. A cache that
/// cannot be read answers none: the guard then judges the id as it
/// would without the note.
pub(crate) fn linked(state: &AppState, row: &str) -> Option<String> {
    meta_cache_get(&state.cache_pool, &key(row)).ok().flatten()
}
