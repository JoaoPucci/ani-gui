//! Whether the rows past the count have names of their own, read
//! against the entry's episodes and the first ones of every aired TV
//! entry Kitsu relates it to ([`super`]).

use super::titles::{is_generic, same_title};
use super::{EntryTail, Related, SequelEpisode};
use crate::scraper::provider::EpisodeRef;

/// Whether every row past the count has a name of its own: one that is
/// not missing, not only its number, and not the name of an episode
/// Kitsu lists for the entry or on the first page of a TV entry Kitsu
/// relates the show to that has aired — its sequel, its prequel, a
/// spinoff it calls `other`: whatever the role, a TV entry's episodes
/// are what a provider merges into a listing. Such an entry's page
/// must have been read and carry a title for each of its first
/// episodes, as many as there are rows past the count — the episodes
/// the rows would be, were they that entry's — or nothing tells those
/// episodes from extras; a title Kitsu gives a later episode says
/// nothing about them. A non-TV relation is never read here: the
/// OVA, ONA or special that explains the rows is what the rows are,
/// and Kitsu titles its episodes as the provider titles the rows.
pub(super) fn own_names(past: &[EpisodeRef], tail: &EntryTail) -> bool {
    named_alone(past)
        && unlike_the_show_s(past, tail)
        && tail
            .related
            .iter()
            .filter(|r| is_tv(r) && !announced(r))
            .all(|r| {
                r.first_episodes
                    .as_deref()
                    .is_some_and(|page| leaves_names(past, page))
            })
}

/// Whether every row past the count has a name, and one that is more
/// than its number: read from the listing alone.
#[must_use]
pub fn named_alone(past: &[EpisodeRef]) -> bool {
    past.iter()
        .all(|row| row.title.as_deref().is_some_and(|t| !is_generic(t)))
}

/// Whether no row past the count is named as an episode Kitsu lists
/// for the entry on the pages read.
#[must_use]
pub fn unlike_the_show_s(past: &[EpisodeRef], tail: &EntryTail) -> bool {
    let taken: Vec<&str> = tail
        .episodes
        .iter()
        .filter_map(|e| e.title.as_deref())
        .collect();
    unnamed_by(past, &taken)
}

/// Whether a TV relation's first page, as Kitsu lists it, leaves the
/// rows past the count their names: it titles each of its first
/// episodes, as many as there are rows, and none of them as a row is
/// named.
#[must_use]
pub fn leaves_names(past: &[EpisodeRef], page: &[SequelEpisode]) -> bool {
    let taken: Vec<&str> = page.iter().filter_map(|e| e.title.as_deref()).collect();
    first_titled(page, past.len()) && unnamed_by(past, &taken)
}

/// Whether no row of `past` is named as any of `taken`.
fn unnamed_by(past: &[EpisodeRef], taken: &[&str]) -> bool {
    past.iter().all(|row| {
        row.title
            .as_deref()
            .is_none_or(|t| !taken.iter().any(|k| same_title(k, t)))
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

/// Whether `r` is a TV entry, in whatever role Kitsu relates it.
fn is_tv(r: &Related) -> bool {
    r.anime && r.subtype.as_deref() == Some("TV")
}
