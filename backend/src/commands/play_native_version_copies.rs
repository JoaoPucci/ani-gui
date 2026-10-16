//! Which candidates are copies of the winner under another version
//! tag — split from `play_native_versions` for the per-file
//! complexity bar: reading a title's version tag, telling copies of
//! one listing apart from other listings, and the dead candidates
//! that leave a pick's versions unheard.

use crate::scraper::provider::{BrowseHit, EpisodeRef};

use super::play_native_title_grammar::normalized;
use super::play_native_title_marker::EntryTitles;
use super::play_native_versions::{ShowVersion, VersionKind};

/// The version tag's word, as a bracket holds it.
const UNCENSORED: &str = "uncensored";

/// `title` read for its version: the title without a bracketed
/// version tag — "(Uncensored)", "[UNCENSORED]", in ASCII or
/// full-width brackets, any case — compared as written
/// ([`normalized`]: case and spacing aside, catalogue tags kept), and
/// which version the tag names.
pub(crate) fn version_of(title: &str) -> (String, VersionKind) {
    let mut rest = title;
    let mut out = String::new();
    let mut kind = VersionKind::Standard;
    while let Some(open) = rest.find(['(', '[', '（', '［', '【']) {
        let after = &rest[open..];
        let inner_start = after.chars().next().map_or(1, char::len_utf8);
        let Some(close) = after.find([')', ']', '）', '］', '】']) else {
            break;
        };
        let inner: String = after[inner_start..close]
            .trim()
            .chars()
            .map(|c| match c {
                '！'..='～' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
                _ => c,
            })
            .collect();
        let closer = after[close..].chars().next().map_or(1, char::len_utf8);
        if inner.eq_ignore_ascii_case(UNCENSORED) {
            kind = VersionKind::Uncensored;
            out.push_str(&rest[..open]);
            out.push(' ');
        } else {
            out.push_str(&rest[..open + close + closer]);
        }
        rest = &after[close + closer..];
    }
    out.push_str(rest);
    (normalized(&out), kind)
}

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
            episodes: eps.clone(),
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
/// could have been picked — `could_win` by the dated-name rule,
/// `vouched` by the rules that read a count alone
/// (`play_native_count_alone::count_vouches`), admitted by the entry's
/// titles and not another entry of the same show — so a winner's copy
/// among them leaves the pick unheard ([`copy_died`]).
pub(crate) fn note_dead<'h>(
    dead: &mut Vec<&'h BrowseHit>,
    hit: &'h BrowseHit,
    could_win: bool,
    vouched: bool,
    entry: EntryTitles<'_>,
) {
    if could_win && vouched && entry.admits(&hit.title) && !entry.another_entry(&hit.title) {
        dead.push(hit);
    }
}

#[cfg(test)]
#[path = "play_native_version_tag_test.rs"]
mod tag_tests;
