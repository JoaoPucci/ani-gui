//! The picker's year-identity filter — split from `play_native` so
//! each file stays inside the complexity ratchet's per-file bar.

use crate::error::Result;
use crate::scraper::anidb::{AnidbClient, AnidbFetch, BrowseHit};

use super::play_native::MAX_PROBED_CANDIDATES;

/// The considered head of `hits`, narrowed by Kitsu's year when it
/// is known: each candidate's detail page names its premiere year,
/// and a known year more than one off Kitsu's excludes the
/// candidate — the identity signal applies even to a lone candidate.
/// Only an exact year counts as positive identity; one year off is
/// tolerated (December premieres straddle catalogue years) without
/// vouching for the candidate.
/// Unknown years (a 404ing detail page, a page without a season
/// link) never exclude, so a provider markup change degrades to
/// year-blind picking rather than emptying pools. A pool whose every
/// candidate carries a known mismatched year contains the show
/// nowhere — that is a rejection ([`crate::error::AniError::NoResults`]),
/// so the walk can try the next alias. Each survivor carries whether
/// its own year positively matched.
pub(crate) async fn year_filtered<'a, F: AnidbFetch>(
    client: &AnidbClient<F>,
    hits: &'a [BrowseHit],
    year: Option<u32>,
) -> Result<(Vec<(&'a BrowseHit, bool)>, bool)> {
    let head = hits.iter().take(MAX_PROBED_CANDIDATES);
    let Some(year) = year else {
        return Ok((head.map(|h| (h, false)).collect(), false));
    };
    let mut kept = Vec::new();
    let mut excluded_any = false;
    for h in head {
        // `?`: a refusal, rate limit, or transport failure on a
        // detail page is the provider blocking this client — probing
        // the rest of the pool would turn one block into a burst.
        match client.detail_year(&h.slug).await? {
            // Exact match is positive identity; one year off is the
            // December-premiere allowance — enough to stay in the
            // pool and compete on count, never enough to vouch for
            // a candidate (the rescue and the countless preference
            // key on the flag, and a boundary-tolerated movie must
            // not ride them past a large count mismatch).
            Some(y) if y == year => kept.push((h, true)),
            Some(y) if y.abs_diff(year) <= 1 => kept.push((h, false)),
            Some(_) => excluded_any = true,
            None => kept.push((h, false)),
        }
    }
    if kept.is_empty() {
        return Err(crate::error::AniError::NoResults);
    }
    Ok((kept, excluded_any))
}
