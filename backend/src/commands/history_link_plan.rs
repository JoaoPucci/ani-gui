//! Which rows a link writes and which an undo can give back — the
//! pure half of [`super`], split from it for the per-file complexity
//! bar.

use super::LinkedRow;
use crate::history::{same_kitsu_id, HistoryEntry};
use crate::scraper::provider::same_listing;

/// Whether a link made from the card whose row is `card` may write
/// the row `row`: whether the two rows name one listing.
///
/// A card stands for every row Continue Watching guesses as the same
/// show, and a guessed play that landed on another listing writes a
/// row there that the same guess matches — JoJo's 2000 OVA card plays
/// the 2012 series and writes the 2012 listing's row. That row is the
/// other show's history, so a link from the card never reaches it.
/// The rows of one listing are the card's own row and a bare row of a
/// retired provider whose slug carries the same words
/// ([`same_listing`], asked either way round, since either row can be
/// the bare one).
#[must_use]
pub fn card_listing(card: &str, row: &str) -> bool {
    card == row || same_listing(card, row) || same_listing(row, card)
}

/// The rows of `entries` a link of `rows` from the card whose row is
/// `card` writes, each once and in the order asked, with what each
/// holds now: the held rows of the card's listing ([`card_listing`]).
#[must_use]
pub fn plan_link(entries: &[HistoryEntry], card: &str, rows: &[String]) -> Vec<LinkedRow> {
    plan_held(entries, &of_card(card, rows))
}

/// The rows of `rows` in the listing of the card whose row is `card`.
#[must_use]
pub fn of_card(card: &str, rows: &[String]) -> Vec<String> {
    rows.iter()
        .filter(|r| card_listing(card, r))
        .cloned()
        .collect()
}

/// The rows of `entries` a link of `rows` writes, each once and in the
/// order asked, with what each holds now. A row the history does not
/// hold is not linked.
#[must_use]
pub fn plan_held(entries: &[HistoryEntry], rows: &[String]) -> Vec<LinkedRow> {
    let mut planned: Vec<LinkedRow> = Vec::new();
    for row in rows {
        if planned.iter().any(|p| &p.row == row) {
            continue;
        }
        if let Some(entry) = entries.iter().find(|e| &e.id == row) {
            planned.push(LinkedRow {
                row: row.clone(),
                previous: entry.kitsu_id.clone(),
                ep_no: entry.ep_no.clone(),
                watched_at: entry.watched_at,
            });
        }
    }
    planned
}

/// Whether `entry` still reads as the link of `kitsu_id` left it.
#[must_use]
pub fn left_as_linked(entry: &HistoryEntry, kitsu_id: &str, linked: &LinkedRow) -> bool {
    entry.id == linked.row
        && entry.ep_no == linked.ep_no
        && entry.watched_at == linked.watched_at
        && entry
            .kitsu_id
            .as_deref()
            .is_some_and(|id| same_kitsu_id(id, kitsu_id))
}

/// The rows of `entries` an undo gives back their earlier id: each
/// named row that still reads as the link left it, once.
#[must_use]
pub fn plan_unlink<'a>(
    entries: &[HistoryEntry],
    kitsu_id: &str,
    rows: &'a [LinkedRow],
) -> Vec<&'a LinkedRow> {
    let mut planned: Vec<&LinkedRow> = Vec::new();
    for linked in rows {
        if planned.iter().any(|p| p.row == linked.row) {
            continue;
        }
        if entries.iter().any(|e| left_as_linked(e, kitsu_id, linked)) {
            planned.push(linked);
        }
    }
    planned
}

#[cfg(test)]
#[path = "history_link_prop_test.rs"]
mod prop_tests;
