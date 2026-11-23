//! The batched streaming-listing lookup: many shows' listings in one
//! `Page { media(idMal_in:) }` request and one `Page { media(id_in:) }`
//! request, for the Continue Watching strip and any rail filling several
//! shows' episode stills at once. Split from [`super`] so each file
//! stays inside the CRAP gate's per-file bar.
//!
//! Each show reads what its single lookup ([`streaming_listing_for_ids`])
//! would read. That lookup asks by MAL id first and by AniList id when
//! AniList has no media for the MAL id, so the batch asks in the same
//! order: the MAL page first, then the AniList-id page with the shows
//! Kitsu maps to AniList alone and every show whose MAL id the first
//! page answered without. The two cannot be one request: AniList ANDs
//! `idMal_in` with `id_in` on one `media` field.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::future::Future;

use super::{streaming_listing_for_ids, StreamingListing};
use crate::error::{AniError, Result};
use crate::meta::anilist::{post_graphql_public, ANILIST_API};
use crate::meta::anilist_pages::{whole_pages, Page, PAGE_CAP};

#[path = "anilist_streaming_eps_batch_page.rs"]
mod page;
pub(crate) use page::parse_streaming_page;

/// [`super::STREAMING_EPS_BY_MAL_GQL`] for a page of MAL ids. Selects
/// `idMal` so each node maps back to the shows that asked for it.
const BATCH_BY_MAL_GQL: &str = "query StreamingEpsBatchByMal($malIds: [Int]) { \
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(idMal_in: $malIds, type: ANIME) { \
                idMal episodes status streamingEpisodes { title thumbnail } \
            } \
        } \
    }";

/// [`BATCH_BY_MAL_GQL`] by AniList's own id.
const BATCH_BY_ID_GQL: &str = "query StreamingEpsBatchById($ids: [Int]) { \
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(id_in: $ids, type: ANIME) { \
                id episodes status streamingEpisodes { title thumbnail } \
            } \
        } \
    }";

/// A show as Kitsu's mappings identify it to AniList.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Want {
    /// Its MyAnimeList id.
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

/// One whole page's listings by key, and the keys several media answer.
pub(crate) type Keyed = (HashMap<u32, StreamingListing>, Vec<u32>);

/// The streaming listing of each show in `wants`, in order, in as few
/// requests as AniList allows: one page per id space and fifty ids,
/// plus a single lookup for each MAL id several media record and each
/// id a page is cut for even alone. A show AniList answered without
/// reads the empty listing, as its single lookup does.
///
/// # Errors
/// Per show: the failure of the page it was asked in (Network,
/// Upstream, RateLimited, ParseFailed), or of its single lookup. A
/// failure answers only the shows its request covered.
pub async fn streaming_listings_batch(
    client: &reqwest::Client,
    wants: &[Want],
    base_override: Option<&str>,
) -> Vec<Result<StreamingListing>> {
    let url = base_override.unwrap_or(ANILIST_API);
    resolve(
        wants,
        |space, ids| ask_page(client, url, space, ids),
        |want| streaming_listing_for_ids(client, want.mal, want.anilist, base_override),
    )
    .await
}

async fn ask_page(
    client: &reqwest::Client,
    url: &str,
    space: Space,
    ids: Vec<u32>,
) -> Result<Page<Keyed>> {
    let (query, var) = match space {
        Space::Mal => (BATCH_BY_MAL_GQL, "malIds"),
        Space::AniList => (BATCH_BY_ID_GQL, "ids"),
    };
    let body = serde_json::json!({ "query": query, "variables": { var: ids } });
    let bytes = post_graphql_public(client, url, &body).await?;
    parse_streaming_page(&bytes, space)
}

/// What the pages of one id space said about each key asked.
#[derive(Default)]
struct Answers {
    found: HashMap<u32, StreamingListing>,
    /// Keys only a single lookup can settle.
    single: HashSet<u32>,
    failed: HashMap<u32, AniError>,
}

/// What one key reads as.
enum Got {
    Listing(StreamingListing),
    Failed(AniError),
    Single,
    /// The page answered without it.
    Absent,
}

impl Answers {
    fn get(&self, key: u32) -> Got {
        if let Some(l) = self.found.get(&key) {
            return Got::Listing(l.clone());
        }
        if let Some(e) = self.failed.get(&key) {
            return Got::Failed(e.clone());
        }
        if self.single.contains(&key) {
            return Got::Single;
        }
        Got::Absent
    }
}

/// Ask `keys` in `space`, a walk of whole pages per fifty ids: a walk's
/// failure answers its own keys only.
async fn answers<P, PF>(space: Space, keys: &[u32], ask: &mut P) -> Answers
where
    P: FnMut(Space, Vec<u32>) -> PF,
    PF: Future<Output = Result<Page<Keyed>>>,
{
    let mut out = Answers::default();
    for chunk in keys.chunks(PAGE_CAP) {
        let mut alone = Vec::new();
        let walked = whole_pages(
            chunk,
            |ids| ask(space, ids.to_vec()),
            |one| {
                alone.push(one);
                Ok(())
            },
        )
        .await;
        match walked {
            Ok(pages) => {
                for (once, several) in pages {
                    out.found.extend(once);
                    out.single.extend(several);
                }
                out.single.extend(alone);
            }
            Err(e) => out.failed.extend(chunk.iter().map(|k| (*k, e.clone()))),
        }
    }
    out
}

/// [`streaming_listings_batch`] over any transport: `ask` answers one
/// page of ids in a space, `single` one show's single lookup.
pub(crate) async fn resolve<P, PF, S, SF>(
    wants: &[Want],
    mut ask: P,
    mut single: S,
) -> Vec<Result<StreamingListing>>
where
    P: FnMut(Space, Vec<u32>) -> PF,
    PF: Future<Output = Result<Page<Keyed>>>,
    S: FnMut(Want) -> SF,
    SF: Future<Output = Result<StreamingListing>>,
{
    let mut out: Vec<Option<Result<StreamingListing>>> = vec![None; wants.len()];
    let mut singles: HashMap<Want, Result<StreamingListing>> = HashMap::new();
    let mal_keys: Vec<u32> = wants
        .iter()
        .filter_map(|w| w.mal)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let by_mal = answers(Space::Mal, &mal_keys, &mut ask).await;
    // The shows asked by AniList id: those Kitsu maps to AniList alone,
    // and those whose MAL id AniList answered without.
    let mut by_id_wanted = Vec::new();
    for (i, w) in wants.iter().enumerate() {
        let got = match w.mal {
            Some(mal) => by_mal.get(mal),
            None => Got::Absent,
        };
        out[i] = match (got, w.anilist) {
            (Got::Listing(l), _) => Some(Ok(l)),
            (Got::Failed(e), _) => Some(Err(e)),
            (Got::Single, _) => Some(single_once(&mut singles, &mut single, *w).await),
            (Got::Absent, Some(_)) => {
                by_id_wanted.push(i);
                None
            }
            (Got::Absent, None) => Some(Ok(StreamingListing::default())),
        };
    }
    let id_keys: Vec<u32> = by_id_wanted
        .iter()
        .filter_map(|i| wants[*i].anilist)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let by_id = answers(Space::AniList, &id_keys, &mut ask).await;
    for i in by_id_wanted {
        let Some(id) = wants[i].anilist else { continue };
        out[i] = Some(match by_id.get(id) {
            Got::Listing(l) => Ok(l),
            Got::Failed(e) => Err(e),
            Got::Single => {
                let want = Want {
                    mal: None,
                    anilist: Some(id),
                };
                single_once(&mut singles, &mut single, want).await
            }
            Got::Absent => Ok(StreamingListing::default()),
        });
    }
    out.into_iter()
        .map(|o| o.unwrap_or_else(|| Ok(StreamingListing::default())))
        .collect()
}

/// `want`'s single lookup, asked once however many shows need it.
async fn single_once<S, SF>(
    singles: &mut HashMap<Want, Result<StreamingListing>>,
    single: &mut S,
    want: Want,
) -> Result<StreamingListing>
where
    S: FnMut(Want) -> SF,
    SF: Future<Output = Result<StreamingListing>>,
{
    if let Some(done) = singles.get(&want) {
        return done.clone();
    }
    let got = single(want).await;
    singles.insert(want, got.clone());
    got
}

#[cfg(test)]
#[path = "anilist_streaming_eps_batch_test.rs"]
mod tests;

#[cfg(test)]
#[path = "anilist_streaming_eps_batch_prop_test.rs"]
mod prop_tests;
