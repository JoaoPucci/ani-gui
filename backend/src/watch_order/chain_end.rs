//! The player's Up next card at the end of a chain outside the main
//! story that leads nowhere (the whole franchise), and the one reading
//! of which card a finale shows.

use serde::Serialize;

use super::chain_view::chain_franchise;
use super::run::run_of;
use super::strip_view::{franchise_summary, FranchiseSummary};
use super::up_next::{finale_card, is_finale, FinaleCard, FinaleSelf};
use super::{Franchise, HistoryRow, WatchOrder};

/// The card at the last title of a chain outside the main story that
/// leads nowhere: the franchise page, by its name, fronted by the entry
/// the name comes from, with the poster stack and the counts the strip's
/// franchise link shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChainEndCard {
    /// The franchise's name.
    pub name: String,
    /// The id the franchise page is opened for.
    pub id: String,
    /// The main-story entry the name comes from, which fronts the stack
    /// and gives the card its accent.
    pub front: String,
    /// The franchise link's stack: its posters and its counts.
    pub summary: FranchiseSummary,
}

/// What a finale needs to know beyond the franchise and the rows.
#[derive(Debug, Clone, Copy)]
pub struct Finale<'a> {
    /// The id the player is on.
    pub id: &'a str,
    /// The order setting.
    pub order: WatchOrder,
    /// The viewer's local day.
    pub today: &'a str,
    /// The episode playing.
    pub episode: f64,
    /// What the player read about the entry, where it read it.
    pub own: Option<&'a FinaleSelf>,
}

/// The chain-end card the finale shows, or none: anywhere the Up next
/// card names something, or on anything but the finale of a chain's
/// last title that leads nowhere, read whole.
#[must_use]
pub fn chain_end_card(
    franchise: &Franchise,
    rows: &[HistoryRow],
    at: Finale<'_>,
) -> Option<ChainEndCard> {
    if finale_card(
        franchise, rows, at.id, at.order, at.today, at.episode, at.own,
    )
    .is_some()
    {
        return None;
    }
    if franchise.anchor != at.id || !franchise.complete {
        return None;
    }
    let run = chain_franchise(franchise).as_ref().and_then(run_of)?;
    let entry = run.entries.iter().find(|e| e.id == at.id)?;
    let main = run_of(franchise)?;
    let decides = at.own.cloned().unwrap_or_else(|| entry.into());
    if !is_finale(&decides, at.episode) {
        return None;
    }
    let summary = franchise_summary(&main, &franchise.name_from);
    // The franchise page's art: the title the name comes from, else the
    // main story's first, as the summary fronts the stack.
    let front = main
        .entries
        .iter()
        .find(|e| e.id == franchise.name_from)
        .unwrap_or(&main.entries[0]);
    Some(ChainEndCard {
        name: franchise.name.clone(),
        id: at.id.to_string(),
        front: front.id.clone(),
        summary,
    })
}

/// The one card the player shows at a finale.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "card", rename_all = "lowercase")]
pub enum PlayerUpNext {
    /// An entry card: what comes next.
    Entry(FinaleCard),
    /// The franchise card, at a chain's dead end.
    Franchise(ChainEndCard),
}

/// The one card the finale shows, or none: the finale card where it
/// names a next title, else the franchise card at a chain's dead end.
#[must_use]
pub fn player_up_next(
    franchise: &Franchise,
    rows: &[HistoryRow],
    at: Finale<'_>,
) -> Option<PlayerUpNext> {
    if let Some(card) = finale_card(
        franchise, rows, at.id, at.order, at.today, at.episode, at.own,
    ) {
        return Some(PlayerUpNext::Entry(card));
    }
    chain_end_card(franchise, rows, at).map(PlayerUpNext::Franchise)
}
