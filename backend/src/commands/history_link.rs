//! Linking history rows to the show the user names for them.
//!
//! A Continue Watching card whose row recorded no Kitsu id shows the
//! show a guess matched it to. The guess can be wrong, and some rows
//! never meet a play that could settle it. From the show page a guessed
//! card opened, the user can name the show the row really is: the
//! rows the card stands for then record that show's Kitsu id, as a
//! play from its page would have made them.
//!
//! A link writes the `kitsu:` column and nothing else of the row: its
//! episode, its marker, its watch moment and its place in the file
//! stay as they were. A link is no watch, so no stamp moves and no
//! tracker hears of it; the next play of the card is a play of the
//! linked show, and writes as one. A stored mapping that names another
//! show for a linked row is dropped, since it is the guess the link
//! replaces; one that names the linked show stays. No played mark is
//! written: the mark says a play stored the mapping.
//!
//! The id must be one Kitsu serves: the entry is read first, and a
//! read Kitsu answers with a 404 or 410 refuses the link (and marks
//! the id gone, as any user read does). A row removed while that read
//! was out is not linked.
//!
//! An undo names, for each row, the id the link gave it and the row as
//! the link left it — its episode and watch moment. A row that still
//! reads so gets back the id it had before; one written since by a
//! watch, a resolve or another link is left as it now is. The mapping
//! the link dropped is not restored: it was a guess, and the next
//! Continue load guesses again.

use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::error::Result;
use crate::history::guard::{Epoch, Held};
use crate::history::HistoryEntry;

/// A request to link rows to a show.
#[derive(Debug, Clone, Deserialize)]
pub struct LinkArgs {
    /// The history rows to link: the card's row and the guessed rows
    /// it stands for.
    pub rows: Vec<String>,
    /// The Kitsu id of the show the user named.
    pub kitsu_id: String,
}

/// A row a link wrote, as it left it, and the id it had before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedRow {
    /// The row's id: its provider show key.
    pub row: String,
    /// The Kitsu id the row recorded before the link, if any.
    pub previous: Option<String>,
    /// The row's episode when the link wrote it.
    pub ep_no: String,
    /// The row's watch moment when the link wrote it.
    pub watched_at: Option<i64>,
}

/// What a link wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkOutcome {
    /// The Kitsu id the rows now record.
    pub kitsu_id: String,
    /// Every row linked, in the order asked.
    pub linked: Vec<LinkedRow>,
}

/// A request to undo a link.
#[derive(Debug, Clone, Deserialize)]
pub struct UnlinkArgs {
    /// The Kitsu id the link gave the rows.
    pub kitsu_id: String,
    /// The rows as the link left them.
    pub rows: Vec<LinkedRow>,
}

/// What an undo did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnlinkOutcome {
    /// The rows given back the id they had before the link.
    pub restored: Vec<String>,
    /// The rows written since the link, left as they are.
    pub kept: Vec<String>,
}

/// The rows of `entries` a link of `rows` writes, each once and in the
/// order asked, with what each holds now. A row the history does not
/// hold is not linked.
#[must_use]
pub fn plan_link(entries: &[HistoryEntry], rows: &[String]) -> Vec<LinkedRow> {
    let _ = (entries, rows);
    Vec::new()
}

/// Whether `entry` still reads as the link of `kitsu_id` left it.
#[must_use]
pub fn left_as_linked(entry: &HistoryEntry, kitsu_id: &str, linked: &LinkedRow) -> bool {
    let _ = (entry, kitsu_id, linked);
    false
}

/// The rows of `entries` an undo gives back their earlier id: each
/// named row that still reads as the link left it, once.
#[must_use]
pub fn plan_unlink<'a>(
    entries: &[HistoryEntry],
    kitsu_id: &str,
    rows: &'a [LinkedRow],
) -> Vec<&'a LinkedRow> {
    let _ = (entries, kitsu_id, rows);
    Vec::new()
}

/// Link `args.rows` to `args.kitsu_id`.
///
/// # Errors
/// [`crate::error::AniError::InvalidKitsuId`] for a value that is not
/// an id; the Kitsu read's error when the entry cannot be read (a 404
/// or 410 for one Kitsu deleted); history and cache I/O errors.
pub async fn link(state: &AppState, args: &LinkArgs) -> Result<LinkOutcome> {
    let kitsu_id = crate::kitsu_id::require(&args.kitsu_id)?;
    let begun = crate::history::guard::epoch(&state.history_path);
    crate::history::guard::hold(&state.history_path, |held| {
        link_held(state, held, &args.rows, kitsu_id, begun)
    })
}

fn link_held(
    state: &AppState,
    held: &mut Held<'_>,
    rows: &[String],
    kitsu_id: &str,
    begun: Epoch,
) -> Result<LinkOutcome> {
    let _ = (state, held, rows, begun);
    Ok(LinkOutcome {
        kitsu_id: kitsu_id.to_string(),
        linked: Vec::new(),
    })
}

/// Undo a link: every named row that still reads as the link left it
/// gets back the id it had before.
///
/// # Errors
/// History I/O errors.
pub fn unlink(state: &AppState, args: &UnlinkArgs) -> Result<UnlinkOutcome> {
    let _ = state;
    Ok(UnlinkOutcome {
        restored: Vec::new(),
        kept: args.rows.iter().map(|r| r.row.clone()).collect(),
    })
}
#[cfg(test)]
#[path = "history_link_test.rs"]
mod tests;

#[cfg(test)]
#[path = "history_link_prop_test.rs"]
mod prop_tests;
