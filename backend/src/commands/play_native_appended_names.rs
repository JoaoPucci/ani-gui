//! What the rows past the count are: names of their own, and the
//! related entry that explains them ([`super`]).

use super::titles::{is_generic, same_title};
use super::{EntryTail, Related, SequelEpisode};
use crate::scraper::provider::EpisodeRef;

/// Whether every row past the count has a name of its own: one that is
/// not missing, not only its number, and not the name of an episode
/// Kitsu lists for the entry or on the first page of a TV sequel that
/// has aired. Such a sequel's page must have been read and carry a
/// title for each of its first episodes, as many as there are rows
/// past the count — the episodes the rows would be, were they the
/// sequel's — or nothing tells those episodes from extras; a title
/// Kitsu gives a later episode says nothing about them.
pub(super) fn own_names(past: &[EpisodeRef], tail: &EntryTail) -> bool {
    let aired = tail
        .related
        .iter()
        .filter(|r| is_tv_sequel(r) && !announced(r));
    let Some(sequel_titles) = aired
        .map(|r| {
            r.first_episodes
                .as_deref()
                .filter(|eps| first_titled(eps, past.len()))
        })
        .collect::<Option<Vec<&[SequelEpisode]>>>()
    else {
        return false;
    };
    let taken: Vec<&str> = tail
        .episodes
        .iter()
        .filter_map(|e| e.title.as_deref())
        .chain(
            sequel_titles
                .into_iter()
                .flatten()
                .filter_map(|e| e.title.as_deref()),
        )
        .collect();
    past.iter().all(|row| {
        row.title
            .as_deref()
            .is_some_and(|t| !is_generic(t) && !taken.iter().any(|k| same_title(k, t)))
    })
}

/// Whether each of `page`'s first `n` episodes, by number, is there
/// and titled.
fn first_titled(page: &[SequelEpisode], n: usize) -> bool {
    (1..=n).all(|number| {
        u32::try_from(number)
            .is_ok_and(|number| page.iter().any(|e| e.number == number && titled(e)))
    })
}

/// Whether Kitsu gives the episode a title.
fn titled(e: &SequelEpisode) -> bool {
    e.title.as_deref().is_some_and(|t| !t.trim().is_empty())
}

/// Whether Kitsu has `r` only announced: no episode of it has aired.
fn announced(r: &Related) -> bool {
    is_announced(r.status.as_deref())
}

/// Whether a Kitsu `status` is an entry's only announced: upcoming,
/// to be announced, or unreleased — nothing of it has aired, so its
/// episodes are nowhere a listing could hold them.
#[must_use]
pub fn is_announced(status: Option<&str>) -> bool {
    matches!(status, Some("upcoming" | "tba" | "unreleased"))
}

fn is_tv_sequel(r: &Related) -> bool {
    r.anime && r.role == "sequel" && r.subtype.as_deref() == Some("TV")
}

/// Whether Kitsu's relations, read whole, name what the `past` rows
/// are — a finished OVA, ONA or special exactly that long, whatever
/// Kitsu calls its relation — and no sequel airing now, which the rows
/// could be the start of. A sequel only announced has aired nothing.
pub(super) fn explained(past: usize, tail: &EntryTail) -> bool {
    let finished = |r: &Related| r.status.as_deref() == Some("finished");
    let length = u32::try_from(past).ok();
    let extra = |r: &Related| {
        r.anime
            && finished(r)
            && matches!(r.subtype.as_deref(), Some("OVA" | "ONA" | "special"))
            && r.episode_count == length
    };
    let airing_sequel =
        |r: &Related| r.anime && r.role == "sequel" && r.status.as_deref() == Some("current");
    tail.related_complete
        && tail.related.iter().any(extra)
        && !tail.related.iter().any(airing_sequel)
}
