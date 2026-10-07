//! Whether a play stored a show's reverse mapping.
//!
//! A play stores the `show_id → kitsu_id` mapping and, beside it, a
//! mark naming the Kitsu id it stored
//! ([`crate::commands::kitsu::allmanga_kitsu_put_played`]). Any other
//! write of the mapping — a resolve's guess — takes the mark, and so
//! do dropping the mapping and removing the show. A mapping is a
//! play's while the mark names it, and a guess otherwise, however near
//! a watch it was written: a Continue load can store a guess within
//! the second a play stamps the show.
//!
//! Mappings stored before plays left the mark got theirs once, when
//! this build first opened the cache, by the rule the builds that
//! wrote them read them with: a mapping written from a second before
//! to ten seconds after the show's watch stamp was the play's
//! ([`crate::cache::played_marks`]). The rule's
//! flaw — a guess stored in the second a play stamps the show reads as
//! played — is carried into those marks, and only those; every mapping
//! written since is judged by its mark alone.
//!
//! Rows that record the Kitsu id of the show played never need this.
//! It serves rows written before history recorded it.

use crate::app::AppState;
use crate::cache::meta_cache_get;
use crate::commands::kitsu::{allmanga_kitsu_get, allmanga_kitsu_played_key};
use crate::error::Result;

/// The Kitsu id of the show's stored mapping when a play wrote it,
/// else `None`. A caller that read the mapping earlier compares this
/// with the id it read: the mapping can change between the two reads,
/// and the mark vouches only for the one standing now.
///
/// # Errors
/// Cache I/O errors propagate.
pub(crate) fn played_mapping(state: &AppState, show_id: &str) -> Result<Option<String>> {
    let Some(played) = meta_cache_get(&state.cache_pool, &allmanga_kitsu_played_key(show_id))?
    else {
        return Ok(None);
    };
    Ok(allmanga_kitsu_get(state, show_id)?
        .filter(|mapped| crate::history::same_kitsu_id(mapped, &played)))
}

/// Whether the show's stored mapping was written by a play.
///
/// # Errors
/// Cache I/O errors propagate.
pub(crate) fn mapping_played(state: &AppState, show_id: &str) -> Result<bool> {
    Ok(played_mapping(state, show_id)?.is_some())
}

/// Store a resolve's guess as the show's mapping, with the history
/// held. Two things keep it out:
///
/// - A mapping a play stored is the show the user played; the guess
///   answers its request and leaves that mapping standing.
/// - The guess is for a history row. Continue Watching asks for it
///   while a row resolves, and the user can remove the row before the
///   answer is back; with no row for the show there is nothing to map,
///   and storing it would bring back what the removal took.
///
/// # Errors
/// History read, cache read and cache write failures propagate: a
/// mapping whose mark cannot be read is not known to be a guess.
pub(crate) fn store_guess(state: &AppState, show_id: &str, kitsu_id: &str) -> Result<()> {
    crate::history::guard::hold(&state.history_path, |held| {
        let listed = held.rows()?.iter().any(|e| e.id == show_id);
        if !listed || mapping_played(state, show_id)? {
            return Ok(());
        }
        crate::commands::kitsu::allmanga_kitsu_put(state, show_id, kitsu_id)
    })
}

#[cfg(test)]
#[path = "kitsu_played_test.rs"]
mod tests;
