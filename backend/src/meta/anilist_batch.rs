//! Batched AniList lookups by show: many shows' answers in one
//! `Page { media(idMal_in:) }` request and one `Page { media(id_in:) }`
//! request, for the fills a strip or rail asks for several shows at once
//! — episode stills, banners. A [`Kind`] says what is asked for each
//! media, how a node reads, and what one show's single lookup is.
//!
//! Each show reads what its single lookup would read. That lookup asks
//! by MAL id first and by AniList id when AniList has no media for the
//! MAL id ([`crate::meta::anilist_media::with_missing_retry`]), so the
//! batch asks in the same order: the MAL page first, then the AniList-id
//! page with the shows Kitsu maps to AniList alone and every show whose
//! MAL id the first page answered without. The two cannot be one
//! request: AniList ANDs `idMal_in` with `id_in` on one `media` field.

use std::collections::{BTreeSet, HashMap};
use std::future::Future;

use crate::error::Result;
use crate::meta::anilist::{post_graphql_public, ANILIST_API};
use crate::meta::anilist_pages::Page;

#[path = "anilist_batch_answers.rs"]
mod answers;
use answers::{answers, single_once, Got};
#[path = "anilist_batch_coalesce.rs"]
mod coalesce;
#[cfg(test)]
pub(crate) use coalesce::WINDOW;
pub(crate) use coalesce::{coalesced, Lanes};
#[path = "anilist_batch_page.rs"]
mod page;
pub(crate) use page::parse_page;

/// A show as Kitsu's mappings identify it to AniList.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Want {
    /// Its MyAnimeList id.
    pub mal: Option<u32>,
    /// AniList's own id.
    pub anilist: Option<u32>,
}

/// Which id space a page addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Space {
    /// `idMal_in`.
    Mal,
    /// `id_in`.
    AniList,
}

/// One whole page's answers by key, and the keys several media answer.
pub(crate) type Keyed<T> = (HashMap<u32, T>, Vec<u32>);

/// What one kind of batched lookup asks and reads.
pub(crate) trait Kind: Send + Sync + 'static {
    /// One show's answer; its default is what a show AniList has no
    /// media for reads.
    type Out: Clone + Default + Send + 'static;
    /// The page query by MAL id: `$malIds`, selecting `idMal` and
    /// [`Self::FIELDS`].
    const BY_MAL_GQL: &'static str;
    /// The page query by AniList id: `$ids`, selecting `id` and
    /// [`Self::FIELDS`].
    const BY_ID_GQL: &'static str;
    /// The fields each node must carry beside its key.
    const FIELDS: &'static [&'static str];
    /// This kind's lanes.
    fn lanes() -> &'static Lanes<Self::Out>;
    /// One node's answer; `None` when the node cannot be read.
    fn read(node: serde_json::Value) -> Option<Self::Out>;
    /// One show's single lookup.
    fn single(
        client: &reqwest::Client,
        want: Want,
        base_override: Option<&str>,
    ) -> impl Future<Output = Result<Self::Out>> + Send;
}

/// The answer of each show in `wants`, in order, in as few requests as
/// AniList allows: one page per id space and fifty ids, plus a single
/// lookup for each MAL id several media record, each id a page is cut
/// for even alone, and each node that cannot be read. A show AniList
/// answered without reads what its single lookup reads for no media.
///
/// # Errors
/// Per show: the failure of the page it was asked in (Network,
/// Upstream, RateLimited, ParseFailed), or of its single lookup. A
/// failure answers only the shows its request covered.
pub(crate) async fn batch<K: Kind>(
    client: &reqwest::Client,
    wants: &[Want],
    base_override: Option<&str>,
) -> Vec<Result<K::Out>> {
    let url = base_override.unwrap_or(ANILIST_API);
    resolve(
        wants,
        |space, ids| ask_page::<K>(client, url, space, ids),
        |want| K::single(client, want, base_override),
    )
    .await
}

async fn ask_page<K: Kind>(
    client: &reqwest::Client,
    url: &str,
    space: Space,
    ids: Vec<u32>,
) -> Result<Page<Keyed<K::Out>>> {
    let (query, var) = match space {
        Space::Mal => (K::BY_MAL_GQL, "malIds"),
        Space::AniList => (K::BY_ID_GQL, "ids"),
    };
    let body = serde_json::json!({ "query": query, "variables": { var: ids } });
    let bytes = post_graphql_public(client, url, &body).await?;
    parse_page(&bytes, space, K::FIELDS, K::read)
}

/// [`batch`] over any transport: `ask` answers one page of ids in a
/// space, `single` one show's single lookup. A show AniList answered
/// without reads `T::default()`, as its single lookup reads absence.
pub(crate) async fn resolve<T: Clone + Default, P, PF, S, SF>(
    wants: &[Want],
    mut ask: P,
    mut single: S,
) -> Vec<Result<T>>
where
    P: FnMut(Space, Vec<u32>) -> PF,
    PF: Future<Output = Result<Page<Keyed<T>>>>,
    S: FnMut(Want) -> SF,
    SF: Future<Output = Result<T>>,
{
    let mut out: Vec<Option<Result<T>>> = vec![None; wants.len()];
    let mut singles: HashMap<(Space, u32), Result<T>> = HashMap::new();
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
            (Got::Single, _) => {
                let key = (Space::Mal, w.mal.unwrap_or_default());
                Some(single_once(&mut singles, &mut single, key, *w).await)
            }
            (Got::Absent, Some(_)) => {
                by_id_wanted.push(i);
                None
            }
            (Got::Absent, None) => Some(Ok(T::default())),
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
                single_once(&mut singles, &mut single, (Space::AniList, id), want).await
            }
            Got::Absent => Ok(T::default()),
        });
    }
    out.into_iter()
        .map(|o| o.unwrap_or_else(|| Ok(T::default())))
        .collect()
}
