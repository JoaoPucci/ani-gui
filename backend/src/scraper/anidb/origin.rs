//! Whether an answer came from the provider's own origin — split
//! from the client so its file stays inside the complexity ratchet's
//! per-file bar.

use crate::error::{AniError, Result};
use crate::scraper::fetch::{Fetch, FetchRequest, FetchResponse};

/// GET `url`, holding a request on `home`'s origin to that origin.
/// Such a request follows only the redirects that stay on it
/// ([`FetchRequest::held_to_origin`]), so a site anidb redirects to
/// never receives it; the redirect that was not followed, or an
/// answer another origin served all the same, is a parse failure:
/// the walk fails over past it and the gate hears distress, rather
/// than a parser reading another site's page as anidb's answer. A
/// request off `home`'s origin — the CDN's playlist — follows its
/// redirects wherever they go.
///
/// # Errors
/// [`AniError::ParseFailed`] for a held request redirected off its
/// origin or [`answered_elsewhere`], plus the transport errors of
/// [`Fetch::fetch`].
pub(super) async fn get_from_origin<F: Fetch>(
    fetch: &F,
    home: &str,
    url: &str,
) -> Result<FetchResponse> {
    let held = on_origin(home, url);
    let req = FetchRequest::get(url);
    let req = if held { req.held_to_origin() } else { req };
    let resp = fetch.fetch(&req).await?;
    if held && ((300..400).contains(&resp.status) || answered_elsewhere(home, url, &resp.url)) {
        return Err(AniError::ParseFailed {
            detail: "anidb: a request to its origin was redirected off it or answered from another origin".into(),
        });
    }
    Ok(resp)
}

/// Whether `url` is on `home`'s origin.
fn on_origin(home: &str, url: &str) -> bool {
    let origin = |u: &str| url::Url::parse(u).ok().map(|u| u.origin());
    origin(home).is_some_and(|home| origin(url) == Some(home))
}

/// Whether a request to the provider's origin was answered from
/// another one. A held request follows no redirect off its origin,
/// so the URL the transfer ended on is normally the origin's; this is
/// the check that holds for any transport. When anidb.app began
/// redirecting its search to an unrelated site, the transport
/// followed it, and that site's home page was parsed as an empty
/// search and persisted as anidb's clean miss. A page
/// another origin served says nothing about anidb's catalogue,
/// whatever it contains, so every parser is spared from having to
/// tell. The embed page is held to its own origin the same way; the
/// CDN's playlist is not held at all. A landing URL that
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
