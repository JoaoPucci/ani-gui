//! Whether an answer to one of hianime's own requests came from its
//! origin — split from the client so its file stays inside the
//! complexity ratchet's per-file bar.

use crate::error::{AniError, Result};
use crate::scraper::fetch::{FetchRequest, FetchResponse};

/// Refuse the answer to a request held to the client's origin
/// (`base`) when it is not the origin's: a redirect the transport did
/// not follow — one that left the origin, or one with no target it
/// could follow — which comes back as the 3xx itself, or a page
/// another origin served all the same. Either is
/// a parse failure — the walk fails over past it and the gate hears
/// distress — rather than a parser reading another site's page as
/// hianime's answer. A request that is not held, to the embed host or
/// the CDN, is not judged here.
///
/// # Errors
/// [`AniError::ParseFailed`] for a held request redirected off its
/// origin or [`answered_elsewhere`].
pub(super) fn refuse_foreign(base: &str, req: &FetchRequest, resp: &FetchResponse) -> Result<()> {
    if !req.held_to_origin {
        return Ok(());
    }
    if (300..400).contains(&resp.status) || answered_elsewhere(base, &req.url, &resp.url) {
        return Err(AniError::ParseFailed {
            detail: "hianime: a request to its origin ended on a redirect it did not follow, or was answered from another origin".into(),
        });
    }
    Ok(())
}

/// Whether a request to the provider's origin was answered from
/// another one. A held request follows no redirect off its origin, so
/// the URL the transfer ended on is normally the origin's; this is the
/// check that holds for any transport. A page another origin served
/// says nothing about hianime's catalogue, whatever it contains, so
/// no parser has to tell. A landing URL that does not parse cannot be
/// shown to be the origin, and counts as elsewhere.
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
