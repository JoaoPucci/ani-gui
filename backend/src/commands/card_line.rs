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
//! to another show, each row is a card of its own.
//!
//! The same holds once the user named the line's show: its next play —
//! from Home, or "Switch to it now" after a pick — is that show's own
//! stream, and lands on its listing, so the shown row records its show
//! and is newer than the line. While the line it came from records the
//! same show as one the user named, the card stands for that line and
//! keeps the way to change it again. A bare line of a retired provider
//! plays on the walked listing with its words, which is its own
//! listing, so its play notes no card: a named line of the shown row's
//! own listing, shown as the same show, is the line the card stands
//! for too. Any other shown row that records its own show — played
//! from that show's page, or by a card whose line is not named for
//! that show — stands for itself; so does a shown row the user named
//! itself.
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
    /// The Continue card whose play wrote this row on another listing
    /// ([`super::guessed_from`]).
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

/// The line the card showing `shown` stands for, among `rows`: the
/// row its card's play came from while that row is held and shows as
/// the same show — for a guessed `shown`, or for a line the user named
/// when `shown` was not named itself — or, for a `shown` neither
/// guessed nor named, a named line of its own listing shown as the
/// same show; else `shown` itself.
#[must_use]
pub fn card_line<'a>(shown: &'a CardRow, rows: &'a [CardRow]) -> &'a CardRow {
    if shown.show.is_none() {
        return shown;
    }
    if let Some(from) = shown.guessed_from.as_deref().filter(|f| *f != shown.id) {
        let noted = rows.iter().find(|r| {
            r.id == from && r.show == shown.show && (shown.guess || (r.named && !shown.named))
        });
        if let Some(line) = noted {
            return line;
        }
    }
    if shown.guess || shown.named {
        return shown;
    }
    // A named line's play of its own listing notes no card: a bare
    // line's lands on the walked row with its words.
    rows.iter()
        .find(|r| {
            r.id != shown.id
                && r.named
                && r.show == shown.show
                && super::history_link::card_listing(&r.id, &shown.id)
        })
        .unwrap_or(shown)
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
                    correctable: line.guess || line.named,
                },
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "card_line_test.rs"]
mod tests;
