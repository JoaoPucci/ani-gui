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
//! linked show, and writes as one. To the history guard, though, it is
//! a write that gives the row its Kitsu id, as a watch's is
//! ([`crate::history::guard`]): a play or watch whose request began
//! before the link writes nothing of its own on the row, and the
//! deferred writes of one in flight stand down — the user's naming of
//! the show stands. A stored mapping that names another
//! show for a linked row is dropped, since it is the guess the link
//! replaces; one that names the linked show stays. No played mark is
//! written: the mark says a play stored the mapping.
//!
//! The id must be one Kitsu serves: the entry is read first, from the
//! cache or from Kitsu, and a read Kitsu answers with a 404 or 410
//! refuses the link (and marks
//! the id gone, as any user read does). A row removed while that read
//! was out is not linked.
//!
//! An undo names, for each row, the id the link gave it and the row as
//! the link left it — its episode, watch moment and the id the link
//! gave it. A row that still reads so gets back the id it had before;
//! one a watch or another link wrote since reads otherwise, and is left
//! as it now is. A resolve that rewrote only the row's title is not
//! told apart, and is undone with the rest. The mapping
//! the link dropped is not restored: it was a guess, and the next
//! Continue load guesses again.

use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::error::Result;
use crate::history::guard::{Epoch, Held};
use crate::history::same_kitsu_id;

#[path = "history_link_plan.rs"]
mod plan;
pub use plan::{card_listing, left_as_linked, of_card, plan_held, plan_link, plan_unlink};

/// A request to link rows to a show.
#[derive(Debug, Clone, Deserialize)]
pub struct LinkArgs {
    /// The row of the card the link is made from.
    pub card: String,
    /// The history rows to link: the card's row and the guessed rows
    /// it stands for.
    pub rows: Vec<String>,
    /// The Kitsu id of the show the user named.
    pub kitsu_id: String,
}

/// A request for the rows a link would write.
#[derive(Debug, Clone, Deserialize)]
pub struct LinkGroupArgs {
    /// The row of the card the link would be made from.
    pub card: String,
    /// The rows the card stands for.
    pub rows: Vec<String>,
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

/// The rows a link of `args.rows` from the card `args.card` would
/// write now — the renderer's count of what its Link covers.
///
/// # Errors
/// History I/O errors.
pub fn link_group(state: &AppState, args: &LinkGroupArgs) -> Result<Vec<String>> {
    let entries = crate::history::read_all(&state.history_path)?;
    Ok(plan_link(&entries, &args.card, &args.rows)
        .into_iter()
        .map(|l| l.row)
        .collect())
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
    crate::commands::kitsu::kitsu_anime_detail_page(state, kitsu_id).await?;
    // Only the card's own listing: a row a guessed play wrote for
    // another listing is that show's history ([`card_listing`]).
    let rows = of_card(&args.card, &args.rows);
    crate::history::guard::hold(&state.history_path, |held| {
        link_held(state, held, &rows, kitsu_id, begun)
    })
}

fn link_held(
    state: &AppState,
    held: &mut Held<'_>,
    rows: &[String],
    kitsu_id: &str,
    begun: Epoch,
) -> Result<LinkOutcome> {
    let mut linked = plan_held(&held.rows()?, rows);
    linked.retain(|l| !held.show_removed_since(begun, &l.row));
    for l in &linked {
        held.set_kitsu_id(&l.row, Some(kitsu_id.to_string()))?;
        drop_other_mapping(state, &l.row, kitsu_id)?;
    }
    Ok(LinkOutcome {
        kitsu_id: kitsu_id.to_string(),
        linked,
    })
}

/// Drop the row's stored mapping when it names another show.
fn drop_other_mapping(state: &AppState, row: &str, kitsu_id: &str) -> Result<()> {
    let mapped = crate::commands::kitsu::allmanga_kitsu_get(state, row)?;
    if mapped.is_some_and(|m| !same_kitsu_id(&m, kitsu_id)) {
        crate::commands::kitsu::allmanga_kitsu_delete(state, row)?;
    }
    Ok(())
}

/// Undo a link: every named row that still reads as the link left it
/// gets back the id it had before.
///
/// # Errors
/// History I/O errors.
pub fn unlink(state: &AppState, args: &UnlinkArgs) -> Result<UnlinkOutcome> {
    crate::history::guard::hold(&state.history_path, |held| {
        let entries = held.rows()?;
        let restored: Vec<LinkedRow> = plan_unlink(&entries, &args.kitsu_id, &args.rows)
            .into_iter()
            .cloned()
            .collect();
        for l in &restored {
            held.set_kitsu_id(&l.row, l.previous.clone())?;
        }
        let kept = args
            .rows
            .iter()
            .filter(|r| !restored.iter().any(|l| l.row == r.row))
            .map(|r| r.row.clone())
            .collect();
        Ok(UnlinkOutcome {
            restored: restored.into_iter().map(|l| l.row).collect(),
            kept,
        })
    })
}

#[cfg(test)]
#[path = "history_link_test.rs"]
mod tests;
