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
use crate::meta::anilist_gate::{Ask, Purpose};
use crate::meta::anilist_pages::Page;

#[path = "anilist_batch_answers.rs"]
mod answers;
use answers::{answers, Got, Settles, Singles};
#[path = "anilist_batch_coalesce.rs"]
mod coalesce;
#[cfg(test)]
pub(crate) use coalesce::WINDOW;
pub(crate) use coalesce::{coalesced, gathered, Lanes};
#[path = "anilist_batch_log.rs"]
mod log;
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
    /// What the kind's requests are for; its label, `stills` or
    /// `banner`, is also the kind's name in the lane's line.
    const PURPOSE: Purpose;
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
        |want| single::<K>(client, want, base_override),
    )
    .await
}

/// [`batch`], handing each show's answer to `on_answer` as soon as it is
/// known ([`resolve_each`]).
pub(crate) async fn batch_each<K: Kind>(
    client: &reqwest::Client,
    wants: &[Want],
    base_override: Option<&str>,
    on_answer: impl FnMut(usize, Result<K::Out>),
) {
    let url = base_override.unwrap_or(ANILIST_API);
    resolve_each(
        wants,
        |space, ids| ask_page::<K>(client, url, space, ids),
        |want| single::<K>(client, want, base_override),
        on_answer,
    )
    .await;
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
    let sent = match space {
        Space::Mal => log::Sent::MalPage,
        Space::AniList => log::Sent::AniListIdPage,
    };
    let count = ids.len();
    let body = serde_json::json!({ "query": query, "variables": { var: ids } });
    log::logged(K::PURPOSE.name(), sent, count, async {
        let bytes = post_graphql_public(client, url, &body, Ask::shows(K::PURPOSE, count)).await?;
        parse_page(&bytes, space, K::FIELDS, K::read)
    })
    .await
}

/// One show's single lookup, logged as a lane's request.
pub(crate) fn single<'a, K: Kind>(
    client: &'a reqwest::Client,
    want: Want,
    base_override: Option<&'a str>,
) -> impl Future<Output = Result<K::Out>> + Send + 'a {
    log::logged(
        K::PURPOSE.name(),
        log::Sent::Single,
        1,
        K::single(client, want, base_override),
    )
}

/// [`batch`] over any transport: `ask` answers one page of ids in a
/// space, `single` one show's single lookup. A show AniList answered
/// without reads `T::default()`, as its single lookup reads absence.
pub(crate) async fn resolve<T: Clone + Default, P, PF, S, SF>(
    wants: &[Want],
    ask: P,
    single: S,
) -> Vec<Result<T>>
where
    P: FnMut(Space, Vec<u32>) -> PF,
    PF: Future<Output = Result<Page<Keyed<T>>>>,
    S: FnMut(Want) -> SF,
    SF: Future<Output = Result<T>>,
{
    let mut out: Vec<Option<Result<T>>> = vec![None; wants.len()];
    resolve_each(wants, ask, single, |i, answer| out[i] = Some(answer)).await;
    out.into_iter()
        .map(|o| o.unwrap_or_else(|| Ok(T::default())))
        .collect()
}

/// [`resolve`], handing each show's answer to `on_answer` (by its index
/// in `wants`) as soon as it is known: the shows a page answered as the
/// page comes in, those a single lookup answers as that lookup does, so
/// no show waits on another's lookup.
pub(crate) async fn resolve_each<T: Clone + Default, P, PF, S, SF>(
    wants: &[Want],
    mut ask: P,
    single: S,
    on_answer: impl FnMut(usize, Result<T>),
) where
    P: FnMut(Space, Vec<u32>) -> PF,
    PF: Future<Output = Result<Page<Keyed<T>>>>,
    S: FnMut(Want) -> SF,
    SF: Future<Output = Result<T>>,
{
    // Answers land from the MAL page's single lookups and the AniList-id
    // page at once: one caller-facing callback, shared.
    let on_answer = std::sync::Mutex::new(on_answer);
    let answer = |i: usize, a: Result<T>| {
        (on_answer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))(i, a);
    };
    // Both branches start single lookups; each call only makes the
    // lookup's future, so the lock is never held across an await.
    let single = std::sync::Mutex::new(single);
    let single = |want: Want| {
        (single
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))(want)
    };
    let mal_keys: Vec<u32> = wants
        .iter()
        .filter_map(|w| w.mal)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let by_mal = answers(Space::Mal, &mal_keys, &mut ask).await;
    // The shows asked by AniList id: those Kitsu maps to AniList alone,
    // and those whose MAL id AniList answered without.
    let mut mal_singles = Singles::new();
    let mut by_id_wanted = Vec::new();
    for (i, w) in wants.iter().enumerate() {
        let got = match w.mal {
            Some(mal) => by_mal.get(mal),
            None => Got::Absent,
        };
        match (got, w.anilist) {
            (Got::Listing(l), _) => answer(i, Ok(l)),
            (Got::Failed(e), _) => answer(i, Err(e)),
            (Got::Single, _) => {
                mal_singles.add(Settles::Key(Space::Mal, w.mal.unwrap_or_default()), *w, i);
            }
            (Got::Unread, _) => mal_singles.add(Settles::Show(*w), *w, i),
            (Got::Absent, Some(_)) => by_id_wanted.push(i),
            (Got::Absent, None) => answer(i, Ok(T::default())),
        }
    }
    let id_keys: Vec<u32> = by_id_wanted
        .iter()
        .filter_map(|i| wants[*i].anilist)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    // The MAL page's single lookups go while the AniList-id page is
    // asked, and the id page's answers are handed out as it comes in:
    // neither branch waits on the other's shows.
    let id_branch = async {
        let by_id = answers(Space::AniList, &id_keys, &mut ask).await;
        let mut id_singles = Singles::new();
        for i in by_id_wanted {
            let Some(id) = wants[i].anilist else { continue };
            let want = Want {
                mal: None,
                anilist: Some(id),
            };
            match by_id.get(id) {
                Got::Listing(l) => answer(i, Ok(l)),
                Got::Failed(e) => answer(i, Err(e)),
                Got::Single | Got::Unread => {
                    id_singles.add(Settles::Key(Space::AniList, id), want, i);
                }
                Got::Absent => answer(i, Ok(T::default())),
            }
        }
        id_singles.run(&single, &answer).await;
    };
    futures_util::future::join(id_branch, mal_singles.run(&single, &answer)).await;
}
