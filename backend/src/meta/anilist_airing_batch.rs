//! The batched airing queries: many shows' [`AiringStatus`] per
//! request, by AniList id or by MAL id, for the home rail's seed. Split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

use super::{airing_status, AiringStatus};
use crate::error::Result;
use crate::meta::anilist::{post_graphql_public, ANILIST_API};

#[path = "anilist_airing_batch_page.rs"]
mod page;
pub use page::parse_airing_batch_response;
use page::parse_batch;
#[cfg(test)]
use page::BatchPage;

/// Batch variant of [`super::AIRING_GQL`]: one `Page` request answers
/// airing for a whole rail. Includes `id` so the response maps back
/// to the requested shows.
const AIRING_BATCH_GQL: &str = "query AiringBatch($ids: [Int]) { \
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(id_in: $ids, type: ANIME) { \
                id status episodes nextAiringEpisode { episode airingAt } \
                airingSchedule(notYetAired: true, perPage: 25) { \
                    nodes { episode airingAt } \
                } \
            } \
        } \
    }";

/// [`AIRING_BATCH_GQL`] addressed by MAL id, for shows Kitsu maps to
/// MAL alone. A separate request rather than a second filter on the
/// same `media` field: AniList ANDs `id_in` with `idMal_in`. Selects
/// `idMal` so the response maps back.
const AIRING_BATCH_BY_MAL_GQL: &str = "query AiringBatchByMal($malIds: [Int]) { \
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(idMal_in: $malIds, type: ANIME) { \
                idMal status episodes nextAiringEpisode { episode airingAt } \
                airingSchedule(notYetAired: true, perPage: 25) { \
                    nodes { episode airingAt } \
                } \
            } \
        } \
    }";

/// Fetch [`AiringStatus`] for many AniList ids in as few requests as
/// possible (chunks of 50 — the `Page` cap). Ids AniList doesn't
/// return are simply absent from the map. Empty input skips the
/// network entirely.
///
/// # Errors
/// Network / Upstream / ParseFailed from the underlying client.
pub async fn airing_status_batch(
    client: &reqwest::Client,
    anilist_ids: &[u32],
    base_override: Option<&str>,
) -> Result<std::collections::HashMap<u32, AiringStatus>> {
    fetch_batch(client, BatchIds::AniList(anilist_ids), base_override).await
}

/// [`airing_status_batch`] keyed by MAL id: the map is keyed by the
/// MAL ids AniList returned. Several media can record one MAL id: such
/// an id is answered by the single lookup by MAL id
/// ([`airing_status`]), as the detail page asks it, never by whichever
/// node the page listed last — and left out when that lookup fails. A
/// page AniList cut (`hasNextPage`) is split and asked again.
///
/// # Errors
/// As [`airing_status_batch`].
pub async fn airing_status_batch_by_mal(
    client: &reqwest::Client,
    mal_ids: &[u32],
    base_override: Option<&str>,
) -> Result<std::collections::HashMap<u32, AiringStatus>> {
    fetch_batch(client, BatchIds::Mal(mal_ids), base_override).await
}

/// Which id space a batch request addresses.
#[derive(Clone, Copy)]
enum BatchIds<'a> {
    AniList(&'a [u32]),
    Mal(&'a [u32]),
}

async fn fetch_batch(
    client: &reqwest::Client,
    ids: BatchIds<'_>,
    base_override: Option<&str>,
) -> Result<std::collections::HashMap<u32, AiringStatus>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let (all, query, var, by_mal) = match ids {
        BatchIds::AniList(ids) => (ids, AIRING_BATCH_GQL, "ids", false),
        BatchIds::Mal(ids) => (ids, AIRING_BATCH_BY_MAL_GQL, "malIds", true),
    };
    let mut out = std::collections::HashMap::with_capacity(all.len());
    let mut singly: Vec<u32> = Vec::new();
    let mut chunks: Vec<&[u32]> = all.chunks(50).rev().collect();
    while let Some(chunk) = chunks.pop() {
        let body = serde_json::json!({
            "query": query,
            "variables": { var: chunk },
        });
        let bytes = post_graphql_public(client, url, &body).await?;
        let page = parse_batch(&bytes, by_mal)?;
        if page.cut {
            match chunk.len() {
                1 => singly.extend_from_slice(chunk),
                n => {
                    let (first, rest) = chunk.split_at(n / 2);
                    chunks.push(rest);
                    chunks.push(first);
                }
            }
            continue;
        }
        let (once, several) = page.by_key();
        out.extend(once);
        singly.extend(several);
    }
    singly.sort_unstable();
    singly.dedup();
    for key in singly {
        let (anilist_id, mal_id) = if by_mal {
            (None, Some(key))
        } else {
            (Some(key), None)
        };
        match airing_status(client, anilist_id, mal_id, base_override).await {
            Ok(Some(status)) => {
                out.insert(key, status);
            }
            Ok(None) => {}
            Err(err) => tracing::debug!(key, error = ?err, "anilist airing: single lookup failed"),
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "anilist_airing_batch_prop_test.rs"]
mod prop_tests;
