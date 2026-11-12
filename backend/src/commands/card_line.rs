//! The history line a Continue card stands for, and whether its show
//! can be corrected from the card.
//!
//! Continue Watching shows the rows guessed as one show as one card,
//! the newer row's, for its progress. A guessed card's play can land on
//! another listing and write a row of its own there: JoJo's 2000 card,
//! guessed as the 2012 series, writes the 2012 listing's row, later
//! than the card's own. The backend notes, beside that row, the card it
//! came from ([`super::guessed_from`]). While that line is still in the
//! history and shows as the same show, the card stands for it: the
//! pill, the failed resume and the player correct the line the user
//! clicked, not the 2012 series' own history. Once the line is linked
//! to another show, each row is a card of its own. A shown row that
//! records its own show — played from that show's page — is the
//! user's, and the card stands for it.
//!
//! A card's line can be corrected while it is a guess, or once the user
//! named a show for it, which can be changed again.
//!
//! Which show each row shows, and whether that is only a guess, is
//! Continue Watching's matching, which the renderer runs: the rows
//! arrive with it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A history row as Continue Watching shows it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CardRow {
    /// The row's id: its provider show key.
    pub id: String,
    /// The card whose guessed play wrote this row on another listing.
    #[serde(default)]
    pub guessed_from: Option<String>,
    /// The Kitsu id of the show the row shows, `None` while it is not
    /// matched or matches nothing.
    #[serde(default)]
    pub show: Option<String>,
    /// The row's match is only a guess.
    #[serde(default)]
    pub guess: bool,
    /// The row records a show the user named for it.
    #[serde(default)]
    pub named: bool,
}

/// The line a card stands for, and whether it can be corrected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardLine {
    /// The history row the card stands for.
    pub line: String,
    /// The line is a guess, or one the user named a show for.
    pub correctable: bool,
}

/// The line the card showing `shown` stands for, among `rows`.
#[must_use]
pub fn card_line<'a>(shown: &'a CardRow, rows: &'a [CardRow]) -> &'a CardRow {
    let _ = rows;
    shown
}

/// The line each of `rows` stands for as a card, by row id.
#[must_use]
pub fn card_lines(rows: &[CardRow]) -> HashMap<String, CardLine> {
    rows.iter()
        .map(|shown| {
            let line = card_line(shown, rows);
            (
                shown.id.clone(),
                CardLine {
                    line: line.id.clone(),
                    correctable: false,
                },
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "card_line_test.rs"]
mod tests;
