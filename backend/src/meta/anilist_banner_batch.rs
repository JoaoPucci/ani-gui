//! The banner kind of the batched lookup ([`crate::meta::anilist_batch`]):
//! AniList's `bannerImage` for the details of a strip's shows Kitsu has
//! no cover for, read at once. Split from [`super`] so each file stays
//! inside the CRAP gate's per-file bar.

use crate::error::Result;
#[cfg(test)]
use crate::meta::anilist_batch::batch;
use crate::meta::anilist_batch::{coalesced, Kind, Lanes, Want};
use crate::meta::anilist_gate::Purpose;

/// The banner lookup, batched. A show's answer is its banner, `None`
/// where AniList's media has none or AniList has no media for it — as
/// [`super::banner_for_ids`] reads both.
pub(crate) struct Banner;

impl Kind for Banner {
    const PURPOSE: Purpose = Purpose::Banner;
    type Out = Option<String>;
    const BY_MAL_GQL: &'static str = "query BannerBatchByMal($malIds: [Int]) { \
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(idMal_in: $malIds, type: ANIME) { idMal bannerImage } \
        } \
    }";
    const BY_ID_GQL: &'static str = "query BannerBatchById($ids: [Int]) { \
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(id_in: $ids, type: ANIME) { id bannerImage } \
        } \
    }";
    const FIELDS: &'static [&'static str] = &["bannerImage"];

    fn lanes() -> &'static Lanes<Option<String>> {
        static LANES: std::sync::LazyLock<Lanes<Option<String>>> =
            std::sync::LazyLock::new(Default::default);
        &LANES
    }

    fn read(node: serde_json::Value) -> Option<Option<String>> {
        match node.get("bannerImage")? {
            serde_json::Value::Null => Some(None),
            serde_json::Value::String(url) => Some(Some(url.clone())),
            _ => None,
        }
    }

    fn single(
        client: &reqwest::Client,
        want: Want,
        base_override: Option<&str>,
    ) -> impl std::future::Future<Output = Result<Option<String>>> + Send {
        super::banner_for_ids(client, want.mal, want.anilist, base_override)
    }
}

/// `want`'s banner, asked with the other lookups of `lane` that miss at
/// the same time.
///
/// # Errors
/// As [`super::banner_for_ids`] for the show, from whichever request
/// asked for it; [`crate::error::AniError::Network`] when the batch was
/// lost before it answered.
pub(crate) async fn banner_coalesced(
    client: &reqwest::Client,
    lane: &str,
    want: Want,
    base_override: Option<&str>,
) -> Result<Option<String>> {
    coalesced::<Banner>(client, lane, want, base_override).await
}

/// The banner of each show in `wants`, in order — see [`batch`].
#[cfg(test)]
pub(crate) async fn banners_batch(
    client: &reqwest::Client,
    wants: &[Want],
    base_override: Option<&str>,
) -> Vec<Result<Option<String>>> {
    batch::<Banner>(client, wants, base_override).await
}

/// One page of a banner batch.
#[cfg(test)]
pub(crate) fn parse_banner_page(
    body: &[u8],
    space: crate::meta::anilist_batch::Space,
) -> Result<crate::meta::anilist_pages::Page<crate::meta::anilist_batch::Keyed<Option<String>>>> {
    crate::meta::anilist_batch::parse_page(body, space, Banner::FIELDS, Banner::read)
}

#[cfg(test)]
#[path = "anilist_banner_batch_test.rs"]
mod tests;
