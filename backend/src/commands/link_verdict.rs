//! Whether a surface asks about a Continue card's line, and which rows
//! a correction of it covers.
//!
//! A surface opened from a guessed Continue card — the player, the show
//! page it goes up to, Home's pill and failed resume — asks which show
//! the card's line really is while Continue Watching's match for the
//! line is still a guess of the show on screen: a row with no recorded
//! id, or one whose recorded entry Kitsu has deleted. A session the
//! guessed card opened keeps that verdict for its life (`carried`), so
//! a line its own play has since recorded — a guess that landed on the
//! card's own listing — is still asked about, unless the user named a
//! show for it. A line the user named, anywhere, or picked on the
//! player in this run, is offered again instead, as the way to change
//! it. On the player, a line named for another show than the one
//! playing is that pick, kept.
//!
//! A correction covers every guessed row the card stands for on the
//! card's own listing: the rows Continue Watching guesses as the same
//! show, less any another listing wrote ([`super::history_link`]). A
//! row it trusts — one whose recorded entry Kitsu still serves, or one
//! a play's mapping names — stays out. For a line the user named, the
//! rows are those recorded as that show: the naming moved them all.
//!
//! Continue Watching's match for each row, and whether it is a guess,
//! is the renderer's matching: the rows arrive with it. Which of them
//! the surface has seen this run (`carried`, `picked`) is the
//! renderer's session, and arrives as facts.

use serde::{Deserialize, Serialize};

/// What a surface knows of the card's line, for its question.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct VerdictArgs {
    /// The show the surface shows.
    pub page: String,
    /// The Kitsu id of the show Continue Watching matches the line to.
    #[serde(default)]
    pub show: Option<String>,
    /// That match is one a play may record: not a guess.
    #[serde(default)]
    pub trusted: bool,
    /// The line records a show the user named for it.
    #[serde(default)]
    pub named: bool,
    /// The surface may ask about a guess.
    #[serde(default)]
    pub asks_guess: bool,
    /// The surface's session was opened from the line as a guess, and
    /// that guessed play still stands.
    #[serde(default)]
    pub carried: bool,
    /// The line was picked on the player in this run.
    #[serde(default)]
    pub picked: bool,
}

/// What a surface offers for the card's line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferKind {
    /// Asks whether the guess is the show.
    Guess,
    /// Offers to change again the show the user named.
    Again,
}

/// The surface's answer for the card's line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Verdict {
    /// The offer it makes, if any.
    pub offer: Option<OfferKind>,
    /// The line was named for another show than the one shown: on the
    /// player, the pick kept.
    pub kept: bool,
}

/// The surface's answer for the card's line.
#[must_use]
pub fn verdict(args: &VerdictArgs) -> Verdict {
    let shows_page = args.show.as_deref() == Some(args.page.as_str());
    // A line picked on the player is named, though its note may not
    // have been read: it is only offered again.
    let carried = args.carried && !args.picked;
    let guess = args.asks_guess && shows_page && (!args.trusted || (carried && !args.named));
    let offer = if guess {
        Some(OfferKind::Guess)
    } else if shows_page && (args.picked || args.named) {
        Some(OfferKind::Again)
    } else {
        None
    };
    Verdict {
        offer,
        kept: args.named && args.show.is_some() && !shows_page,
    }
}

/// A history row with Continue Watching's match for it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RowMatch {
    /// The row's id.
    pub id: String,
    /// The Kitsu id of the show it matches; `None` when it matches
    /// none, or the match could not be read.
    #[serde(default)]
    pub show: Option<String>,
    /// The match is not a guess.
    #[serde(default)]
    pub trusted: bool,
}

/// The rows a correction of the card whose row is `card`, shown as
/// `current`, covers among `rows`, the card's first: for a line the
/// user named as `saved`, the rows recorded as it. Which of them share
/// the card's listing is [`super::history_link::of_card`]'s to say.
#[must_use]
pub fn group_members(
    card: &str,
    current: &str,
    saved: Option<&str>,
    rows: &[RowMatch],
) -> Vec<String> {
    let covered = |r: &RowMatch| match saved {
        None => r.show.as_deref() == Some(current) && !r.trusted,
        Some(named) => r.show.as_deref() == Some(named),
    };
    std::iter::once(card.to_string())
        .chain(
            rows.iter()
                .filter(|r| r.id != card && covered(r))
                .map(|r| r.id.clone()),
        )
        .collect()
}

#[cfg(test)]
#[path = "link_verdict_test.rs"]
mod tests;
