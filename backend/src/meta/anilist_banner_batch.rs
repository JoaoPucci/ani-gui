//! The banner kind of the batched lookup (stub).

use crate::error::Result;
use crate::meta::anilist_batch::Want;

/// Stub: the single lookup.
pub(crate) async fn banner_coalesced(
    client: &reqwest::Client,
    _lane: &str,
    want: Want,
    base_override: Option<&str>,
) -> Result<Option<String>> {
    super::banner_for_ids(client, want.mal, want.anilist, base_override).await
}

/// Stub: each show's single lookup.
pub(crate) async fn banners_batch(
    client: &reqwest::Client,
    wants: &[Want],
    base_override: Option<&str>,
) -> Vec<Result<Option<String>>> {
    let mut out = Vec::new();
    for w in wants {
        out.push(super::banner_for_ids(client, w.mal, w.anilist, base_override).await);
    }
    out
}

/// Stub.
#[cfg(test)]
pub(crate) fn parse_banner_page(
    _body: &[u8],
    _space: crate::meta::anilist_batch::Space,
) -> Result<crate::meta::anilist_pages::Page<crate::meta::anilist_batch::Keyed<Option<String>>>> {
    Err(crate::error::AniError::ParseFailed {
        detail: "stub".into(),
    })
}

#[cfg(test)]
#[path = "anilist_banner_batch_test.rs"]
mod tests;
