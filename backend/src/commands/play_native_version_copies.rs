//! Which candidates are copies of the winner under another version
//! tag — split from `play_native_versions` for the per-file
//! complexity bar: reading a title's version tag, telling copies of
//! one listing apart from other listings, and the dead candidates
//! that leave a pick's versions unheard.

use crate::scraper::provider::{BrowseHit, EpisodeRef};

use super::play_native_title_marker::EntryTitles;
pub(crate) use super::play_native_version_tag::{version_of, without_version_tag};
use super::play_native_versions::ShowVersion;

/// Whether two hits list one title under any version tag, as far as
/// their cards tell: the same title once the tag is set aside, and the
/// same badge.
fn same_title(a: &BrowseHit, b: &BrowseHit) -> bool {
    version_of(&a.title).0 == version_of(&b.title).0 && a.kind == b.kind
}

/// Whether two hits are copies of one listing under different version
/// tags: [`same_title`], another version.
fn copies(a: &BrowseHit, b: &BrowseHit) -> bool {
    same_title(a, b) && version_of(&a.title).1 != version_of(&b.title).1
}

/// The rows of a listing as a viewer reads them: each episode's number
/// and tag.
fn numbering(episodes: &[EpisodeRef]) -> Vec<(u32, Option<&str>)> {
    episodes
        .iter()
        .map(|e| (e.number, e.number2.as_deref()))
        .collect()
}

/// The versions of the winner at `winner` among the probed candidates
/// `probed` (hit, listing, distance, year confirmed), in the
/// provider's order: every candidate that could be picked at the best
/// distance `best` — the winner's — whose card lists the winner's
/// title ([`same_title`]) and whose listing numbers the same episodes.
/// Empty unless they are under different version tags, one each; two
/// listings under one tag cannot be told apart, and the pick stays as
/// it is.
pub(crate) fn versions_of(
    probed: &[(&BrowseHit, Vec<EpisodeRef>, u32, bool)],
    winner: usize,
    best: u32,
) -> Vec<ShowVersion> {
    let (w, w_eps, _, _) = &probed[winner];
    let rows = numbering(w_eps);
    let group: Vec<ShowVersion> = probed
        .iter()
        .enumerate()
        .filter(|(i, (h, eps, d, _))| {
            *i == winner || (*d == best && same_title(w, h) && numbering(eps) == rows)
        })
        .map(|(_, (h, eps, _, _))| ShowVersion {
            kind: version_of(&h.title).1,
            hit: (*h).clone(),
            // Each copy's rows carry the tags an earlier version gave
            // them, as the listed pick's do: a copy is a show key of its
            // own, read the same way.
            episodes: crate::commands::play_native_merge::mark_offset_recaps(eps.clone()),
        })
        .collect();
    let distinct = group
        .iter()
        .enumerate()
        .all(|(i, v)| group[..i].iter().all(|u| u.kind != v.kind));
    if group.len() < 2 || !distinct {
        return Vec::new();
    }
    group
}

/// Whether a candidate whose probe died unheard (`dead`, those that
/// could have been picked) copies `winner` under another version tag:
/// the pick's versions cannot be told then, and playing one copy now
/// and the other once the dead one answers would split the show's
/// history across two keys on the strength of weather.
pub(crate) fn copy_died(dead: &[&BrowseHit], winner: &BrowseHit) -> bool {
    dead.iter().any(|d| copies(d, winner))
}

/// Note a candidate whose probe died in transport among `dead` when it
/// could have been picked — `could_win` by the name rule that reads the
/// entry (`play_native_dated_name::may_win`), admitted by the entry's
/// titles and not another entry of the same show
/// ([`super::play_native_choice::could_have_won`]) — so a winner's copy
/// among them leaves the pick unheard ([`copy_died`]).
pub(crate) fn note_dead<'h>(
    dead: &mut Vec<&'h BrowseHit>,
    hit: &'h BrowseHit,
    could_win: bool,
    entry: EntryTitles<'_>,
) {
    if super::play_native_choice::could_have_won(entry, &hit.title, could_win) {
        dead.push(hit);
    }
}

#[cfg(test)]
#[path = "play_native_version_tag_test.rs"]
mod tag_tests;
