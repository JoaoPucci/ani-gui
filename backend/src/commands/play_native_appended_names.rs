//! What the rows past the count are: names of their own, and the
//! related entry that explains them ([`super`]).

use super::titles::{is_generic, same_title};
use super::{EntryTail, Related};
use crate::scraper::provider::EpisodeRef;

/// Whether every row past the count has a name of its own: one that is
/// not missing, not only its number, and not the name of an episode
/// Kitsu lists for the entry or on a TV sequel's first page.
pub(super) fn own_names(past: &[EpisodeRef], tail: &EntryTail) -> bool {
    let sequels = tail.related.iter().filter(|r| is_tv_sequel(r));
    let Some(sequel_titles) = sequels
        .map(|r| r.episode_titles.as_deref())
        .collect::<Option<Vec<&[String]>>>()
    else {
        return false;
    };
    let taken: Vec<&str> = tail
        .episodes
        .iter()
        .filter_map(|e| e.title.as_deref())
        .chain(sequel_titles.into_iter().flatten().map(String::as_str))
        .collect();
    past.iter().all(|row| {
        row.title
            .as_deref()
            .is_some_and(|t| !is_generic(t) && !taken.iter().any(|k| same_title(k, t)))
    })
}

fn is_tv_sequel(r: &Related) -> bool {
    r.anime && r.role == "sequel" && r.subtype.as_deref() == Some("TV")
}

/// Whether Kitsu's relations, read whole, name what the `past` rows
/// are — a finished OVA, ONA or special exactly that long — and no
/// sequel still to finish, which the rows could be the start of.
pub(super) fn explained(past: usize, tail: &EntryTail) -> bool {
    let finished = |r: &Related| r.status.as_deref() == Some("finished");
    let length = u32::try_from(past).ok();
    let extra = |r: &Related| {
        r.anime
            && finished(r)
            && matches!(r.subtype.as_deref(), Some("OVA" | "ONA" | "special"))
            && r.episode_count == length
    };
    let airing_sequel = |r: &Related| r.anime && r.role == "sequel" && !finished(r);
    tail.related_complete
        && tail.related.iter().any(extra)
        && !tail.related.iter().any(airing_sequel)
}
