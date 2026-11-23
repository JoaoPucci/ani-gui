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
use crate::scraper::provider::key_search_term;

/// A request for a row's candidates.
#[derive(Debug, Clone, Deserialize)]
pub struct CandidatesArgs {
    /// The row's id: its provider show key, whose words the second
    /// search reads.
    pub row: String,
    /// The row's saved title, as Continue Watching searches it: with a
    /// legacy `(N episodes)` tail, in any of the forms the renderer's
    /// matcher strips, already taken off, since the season is read from
    /// what is left.
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
    let shows = match args.query.as_deref() {
        Some(typed) => {
            let hits = kitsu_search(state, typed.trim()).await?;
            let current = args.current.as_ref().map(|c| c.id.as_str());
            merge_candidates(None, &hits, &[])
                .into_iter()
                .map(|(show, _)| {
                    let is_current = current == Some(show.id.as_str());
                    (show, is_current)
                })
                .collect()
        }
        None => {
            let slug = key_search_term(&args.row);
            let (title_hits, slug_hits) = tokio::join!(
                quiet(state, Some(&args.title)),
                quiet(state, slug.as_deref())
            );
            merge_candidates(args.current.as_ref(), &title_hits, &slug_hits)
        }
    };
    Ok(with_warnings(shows, &args.title, args.episode))
}

/// The hits of a search the picker runs on its own: none for no words,
/// and none when it fails.
async fn quiet(state: &AppState, query: Option<&str>) -> Vec<KitsuAnimeRef> {
    match query.map(str::trim) {
        Some(q) if !q.is_empty() => kitsu_search(state, q).await.unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[path = "link_candidates_test.rs"]
mod tests;
