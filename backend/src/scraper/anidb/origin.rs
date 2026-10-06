//! Whether an answer came from the provider's own origin — split
//! from the client so its file stays inside the complexity ratchet's
//! per-file bar.

use crate::error::{AniError, Result};
use crate::scraper::fetch::{Fetch, FetchResponse};

/// GET `url`, refusing an answer to a request on the provider's
/// origin (`base`) that another origin served, as a parse failure:
/// the walk fails over past it and the gate hears distress, rather
/// than a parser reading another site's page as anidb's answer.
///
/// # Errors
/// [`AniError::ParseFailed`] when [`answered_elsewhere`], plus the
/// transport errors of [`Fetch::get`].
pub(super) async fn get_from_origin<F: Fetch>(
    fetch: &F,
    base: &str,
    url: &str,
) -> Result<FetchResponse> {
    let resp = fetch.get(url).await?;
    if answered_elsewhere(base, url, &resp.url) {
        return Err(AniError::ParseFailed {
            detail: "anidb: a request to its origin was answered from another origin".into(),
        });
    }
    Ok(resp)
}

/// Whether a request to the provider's origin was answered from
/// another one. The transport follows redirects and reports the URL
/// the transfer ended on; when anidb.app began redirecting its search
/// to an unrelated site, that site's home page was parsed as an empty
/// search and the walk persisted it as anidb's clean miss. A page
/// another origin served says nothing about anidb's catalogue,
/// whatever it contains, so every parser is spared from having to
/// tell. Requests the provider sends off its origin by design — the
/// embed page, the playlist — are not held to it. A landing URL that
/// does not parse cannot be shown to be the origin, and counts as
/// elsewhere.
pub(super) fn answered_elsewhere(base: &str, requested: &str, landed: &str) -> bool {
    let origin = |u: &str| url::Url::parse(u).ok().map(|u| u.origin());
    let Some(base) = origin(base) else {
        return false;
    };
    origin(requested).as_ref() == Some(&base) && origin(landed).as_ref() != Some(&base)
}

#[cfg(test)]
#[path = "origin_prop_test.rs"]
mod tests;
