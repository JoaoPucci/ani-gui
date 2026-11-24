//! The IO loop that feeds the walk's first hop
//! ([`crate::commands::franchise_walk::first_hop`]) the listings and
//! details it asks for, through the walk's own reads and caches, so the
//! walk that follows finds them cached and the two never read different
//! data.

use std::collections::BTreeMap;

use crate::app::AppState;
use crate::commands::franchise::{entry_from_detail, franchise_listing};
use crate::commands::franchise_walk::first_hop::{first_hop, FirstHop, Step};
use crate::error::Result;

/// What the show page will draw for `anchor` once the walk answers.
///
/// # Errors
/// [`crate::error::AniError::InvalidKitsuId`] for a value that is not
/// an id, and the errors of the reads the first hop asks for.
pub async fn franchise_first_hop(state: &AppState, anchor: &str) -> Result<FirstHop> {
    let anchor = crate::kitsu_id::require(anchor)?;
    let mut known = BTreeMap::new();
    let mut details = BTreeMap::new();
    loop {
        let (listings, wanted) = match first_hop(anchor, &known, &details) {
            Step::Done(hop) => return Ok(hop),
            Step::Read { listings, details } => (listings, details),
        };
        let read =
            futures_util::future::join_all(listings.iter().map(|id| franchise_listing(state, id)))
                .await;
        for (id, listing) in listings.into_iter().zip(read) {
            known.insert(id, listing?);
        }
        for id in wanted {
            let detail = crate::commands::kitsu::kitsu_anime_detail(state, &id).await?;
            details.insert(id, entry_from_detail(&detail));
        }
    }
}
