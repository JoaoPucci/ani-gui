//! A year and a count are no identity either — split from
//! `play_native_titled` for the per-file complexity bar.
//!
//! For a dated entry with a count, a hit whose page named the entry's
//! year and whose count fit used to be picked on those two alone. A
//! search for a title the provider does not list answers with a page
//! of other shows, and among them one from the same year and of the
//! right length is common: HenSemi (2010, two episodes) played as a
//! Sayonara Zetsubou Sensei OVA, HenSemi (TV) (2011, 13) as Medaka Box
//! Abnormal, which its alias "Abnormal Physiology Seminar" surfaced.
//!
//! So for such an entry a probed candidate is picked only when a name
//! vouches for it: the title searched, or a name of the show — its
//! listing's title, the romanized name its search card carries, or a
//! name its entry page gives it (its Japanese title, its synonyms) —
//! that the entry names ([`EntryTitles::names`]) or that shares an
//! entry title's name ([`EntryTitles::shares_name`]). Every name here
//! is evidence of the show and nothing more: the rules that read a
//! candidate's season and part read its listing's title alone, and run
//! before this one, so a page that writes a sequel's Japanese title
//! without its season vouches for the show and leaves the season to
//! them.

use super::play_native::ep_count_threshold;
use super::play_native_count_alone::{count_stands_alone, vouched_beyond_count};
use super::play_native_numbering::regular_episode_count;
use super::play_native_title_grammar::normalized;
use super::play_native_title_marker::EntryTitles;
use super::play_native_wide_listing::{Probed, UNFIT};
use super::play_native_year::PageNames;
use crate::scraper::provider::BrowseHit;

/// Whether the entry's year and count need a name beside them: a
/// count above one and a premiere year are known. Without a year the
/// count-alone rule reads the pool; for a count of one, the
/// single-video rule.
#[must_use]
pub(crate) fn needs_a_name(expected: u32, year: Option<u32>) -> bool {
    expected > 1 && year.is_some()
}

/// Whether a name vouches for `hit`: it is the title searched
/// (`needle`, as [`normalized`]), or one of the show's names — its
/// title, its card's romanized name, the names its entry page gave
/// (`pages`) — is named by the entry or shares an entry title's name.
pub(crate) fn named_for_entry(
    hit: &BrowseHit,
    needle: &str,
    entry: EntryTitles<'_>,
    pages: &PageNames,
) -> bool {
    if normalized(&hit.title) == needle {
        return true;
    }
    std::iter::once(hit.title.as_str())
        .chain(hit.jname.as_deref())
        .chain(
            pages
                .get(&hit.slug)
                .into_iter()
                .flatten()
                .map(String::as_str),
        )
        .any(|name| entry.names(name) || entry.shares_name(name))
}

/// Whether the rule that reads how a hit is named leaves `hit` able to
/// win: for a dated entry with a count, a name vouches for it
/// ([`named_for_entry`]); for an entry one video long, or one with a
/// count and no year, something beyond its count does — its matched
/// year (`confirmed`), the title searched, or the entry's name for it
/// ([`vouched_beyond_count`]); any other entry reads no name. A
/// candidate that could not, dying unheard, blocks no winner, and
/// answering not-found withdraws no refusal.
pub(super) fn may_win(
    hit: &BrowseHit,
    expected: u32,
    needle: &str,
    entry: EntryTitles<'_>,
    year: Option<u32>,
    pages: &PageNames,
    confirmed: bool,
) -> bool {
    if needs_a_name(expected, year) {
        named_for_entry(hit, needle, entry, pages)
    } else if expected == 1 || count_stands_alone(expected, year) {
        vouched_beyond_count(&hit.title, needle, entry, confirmed)
    } else {
        true
    }
}

/// For a dated entry with a count ([`needs_a_name`]), scores [`UNFIT`]
/// every probed candidate no name vouches for ([`named_for_entry`]).
/// Every other entry is left exactly as it is, as is every candidate
/// already out.
///
/// Returns whether a candidate the year and count would have accepted
/// was scored out — one within the tolerance, or one from the entry's
/// year listing fewer episodes than the entry has, which the airing
/// rescue or the split stitching would have taken: the refusal is an
/// inference from how the candidate is named, so a pool rejected
/// after it is [`super::play_native_title_verdict::refused_by_title`],
/// not a clean miss.
pub(super) fn unnamed_dated(
    probed: &mut [Probed<'_>],
    expected: u32,
    needle: &str,
    entry: EntryTitles<'_>,
    year: Option<u32>,
    pages: &PageNames,
) -> bool {
    if !needs_a_name(expected, year) {
        return false;
    }
    let tolerance = ep_count_threshold(expected);
    let mut refused_a_fit = false;
    for (h, eps, distance, confirmed) in probed.iter_mut() {
        if *distance == UNFIT || named_for_entry(h, needle, entry, pages) {
            continue;
        }
        // What the year and count would have taken: a fit, or a listing
        // from the entry's year shorter than the entry — an airing part
        // the rescue takes, a part of a split the stitching takes.
        let short = *confirmed && regular_episode_count(eps) < expected;
        refused_a_fit |= *distance <= tolerance || short;
        *distance = UNFIT;
    }
    refused_a_fit
}

#[cfg(test)]
#[path = "play_native_dated_name_test.rs"]
mod tests;
