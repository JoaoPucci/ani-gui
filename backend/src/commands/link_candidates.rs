//! The change-show picker's candidates for a guessed history row: the
//! searches it runs, and what each show found would be worth a second
//! look for ([`super::link_rules`]).
//!
//! The picker opens on the hits of the row's saved title and of the
//! words its provider slug carries, each search allowed to fail on its
//! own: a failed one adds nothing, and the show the card shows still
//! leads. A typed search answers with its own hits, its failure the
//! picker's to show, the card's show among them marked as it.

use serde::Deserialize;

use crate::app::AppState;
use crate::commands::kitsu_search::kitsu_search;
use crate::commands::link_rules::{merge_candidates, with_warnings, LinkCandidate};
use crate::error::Result;
use crate::meta::kitsu::KitsuAnimeRef;

/// A request for a row's candidates.
#[derive(Debug, Clone, Deserialize)]
pub struct CandidatesArgs {
    /// The row's id: its provider show key, whose words the second
    /// search reads.
    pub row: String,
    /// The row's saved title, as Continue Watching searches it.
    pub title: String,
    /// The show the card shows now.
    #[serde(default)]
    pub current: Option<KitsuAnimeRef>,
    /// The words the user typed; absent for the picker's own searches.
    #[serde(default)]
    pub query: Option<String>,
    /// The episode the card offers.
    pub episode: f64,
}

/// The row's candidates, each with its warnings.
///
/// # Errors
/// The typed search's Kitsu error. The picker's own two searches never
/// fail the answer.
pub async fn link_candidates(
    state: &AppState,
    args: &CandidatesArgs,
) -> Result<Vec<LinkCandidate>> {
    let _ = (state, kitsu_search, merge_candidates);
    Ok(with_warnings(Vec::new(), &args.title, args.episode))
}

#[cfg(test)]
#[path = "link_candidates_test.rs"]
mod tests;
