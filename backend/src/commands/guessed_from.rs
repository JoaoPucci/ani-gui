//! Which Continue card a history row's latest play came from, when a
//! guessed card's play landed on another listing.
//!
//! A Continue card whose row records no Kitsu id plays the show a guess
//! matched it to. That play can land on another listing than the card's
//! own, and write a row of its own there: JoJo's 2000 card, guessed as
//! the 2012 series, writes the 2012 listing's row. The two rows are
//! guessed as the same show, so Continue Watching shows them as one card,
//! the newer row's. The user clicked the 2000 line, though, and that is
//! the line a correction from the card is about. So the play notes,
//! beside the row it wrote, the card it came from; the history route
//! names it (`guessed_from`), and the renderer stands the card for that
//! line while the two show as the same show.
//!
//! The note is a cache row (`guessed-from:v1:<row>` → the card's row),
//! not a column of the history file: the file's format is shared with
//! builds before this one, which read an unknown trailing column into
//! the title, while a cache row they never ask for costs them nothing.
//! It needs no schema change, and losing it — a cleared cache, a year
//! with no such play — costs only the grouping: the card stands for its
//! newer row again, as it did before the note existed.
//!
//! Every play that records a row settles the note for it: a guessed
//! card's play of another listing sets it, any other play takes it away,
//! since the row's latest play is then its own. Removing the row takes
//! it with the row's other cache rows.

use std::collections::HashMap;
use std::time::Duration;

use crate::app::AppState;
use crate::cache::families::GUESSED_FROM;
use crate::cache::{meta_cache_delete, meta_cache_list_prefix, meta_cache_put};
use crate::error::Result;

/// How long a note lasts without a play renewing it. Long: Continue
/// Watching holds a row for as long as the user keeps it, and a note
/// lost early only makes the card stand for its newer row again.
const TTL: Duration = Duration::from_secs(365 * 24 * 60 * 60);

/// What a recorded play says of the row it wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Note<'a> {
    /// The row was written by the play of this card, of another listing.
    From(&'a str),
    /// The row's latest play is its own.
    Clear,
}

/// What a play that a guessed card `guess_row` asked for (or no card,
/// `None`), and that wrote the row `played`, says of that row.
#[must_use]
pub fn note<'a>(guess_row: Option<&'a str>, played: &str) -> Note<'a> {
    match guess_row {
        Some(card)
            if !card.is_empty()
                && card != played
                && !crate::scraper::provider::same_listing(card, played) =>
        {
            Note::From(card)
        }
        _ => Note::Clear,
    }
}

/// The cache key of the note for `row`.
pub(crate) fn key(row: &str) -> String {
    format!("{GUESSED_FROM}{row}")
}

/// Settle the note for the row `played` a play recorded, asked by the
/// guessed card `guess_row` or by none. A cache that cannot store it is
/// logged: the watch is recorded, and the card stands for the newer row
/// as it would without a note.
pub(crate) fn record(state: &AppState, guess_row: Option<&str>, played: &str) {
    if played.is_empty() {
        return;
    }
    let stored = match note(guess_row, played) {
        Note::From(card) => meta_cache_put(&state.cache_pool, &key(played), card, TTL.as_secs()),
        Note::Clear => meta_cache_delete(&state.cache_pool, &key(played)),
    };
    if let Err(e) = stored {
        tracing::warn!(row = %played, error = ?e, "could not note which card the row came from");
    }
}

/// Every row's note: the row → the card it came from.
///
/// # Errors
/// SQLite errors propagate.
pub(crate) fn all(state: &AppState) -> Result<HashMap<String, String>> {
    Ok(meta_cache_list_prefix(&state.cache_pool, GUESSED_FROM)?
        .into_iter()
        .filter_map(|(k, card)| Some((k.strip_prefix(GUESSED_FROM)?.to_owned(), card)))
        .collect())
}
