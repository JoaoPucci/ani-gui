//! The batched streaming-listing lookup (stub).

use std::collections::HashMap;
use std::future::Future;

use super::{streaming_listing_for_ids, StreamingListing};
use crate::error::{AniError, Result};
use crate::meta::anilist_pages::{Page, PAGE_CAP};

/// A show as Kitsu maps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Want {
    /// Its MAL id.
    pub mal: Option<u32>,
    /// AniList's own id.
    pub anilist: Option<u32>,
}

/// Which id space a page addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Space {
    /// `idMal_in`.
    Mal,
    /// `id_in`.
    AniList,
}

/// One whole page's answers by key, and the keys several media answer.
pub(crate) type Keyed = (HashMap<u32, StreamingListing>, Vec<u32>);

/// Stub.
pub async fn streaming_listings_batch(
    client: &reqwest::Client,
    wants: &[Want],
    base_override: Option<&str>,
) -> Vec<Result<StreamingListing>> {
    let mut out = Vec::with_capacity(wants.len());
    for w in wants {
        out.push(streaming_listing_for_ids(client, w.mal, w.anilist, base_override).await);
    }
    out
}

/// Stub.
pub(crate) async fn resolve<P, PF, S, SF>(
    wants: &[Want],
    _ask: P,
    _single: S,
) -> Vec<Result<StreamingListing>>
where
    P: FnMut(Space, Vec<u32>) -> PF,
    PF: Future<Output = Result<Page<Keyed>>>,
    S: FnMut(Want) -> SF,
    SF: Future<Output = Result<StreamingListing>>,
{
    let _ = PAGE_CAP;
    wants.iter().map(|_| Err(AniError::Network)).collect()
}

/// Stub.
pub(crate) fn parse_streaming_page(_body: &[u8], _space: Space) -> Result<Page<Keyed>> {
    Err(AniError::ParseFailed {
        detail: "stub".into(),
    })
}

#[cfg(test)]
#[path = "anilist_streaming_eps_batch_test.rs"]
mod tests;

#[cfg(test)]
#[path = "anilist_streaming_eps_batch_prop_test.rs"]
mod prop_tests;
