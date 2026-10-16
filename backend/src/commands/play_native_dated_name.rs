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

/// Whether this rule leaves `hit` able to win: the entry needs no name
/// ([`needs_a_name`]), or a name vouches for it ([`named_for_entry`]).
/// A candidate that could not, dying unheard, blocks no winner.
pub(super) fn may_win(
    hit: &BrowseHit,
    expected: u32,
    needle: &str,
    entry: EntryTitles<'_>,
    year: Option<u32>,
    pages: &PageNames,
) -> bool {
    !needs_a_name(expected, year) || named_for_entry(hit, needle, entry, pages)
}

/// For a dated entry with a count ([`needs_a_name`]), scores [`UNFIT`]
/// every probed candidate no name vouches for ([`named_for_entry`]).
/// Every other entry is left exactly as it is, as is every candidate
/// already out.
///
/// Returns whether a candidate within the tolerance — one the year
/// and count would have accepted — was scored out: the refusal is an
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
    for (h, _, distance, _) in probed.iter_mut() {
        if *distance == UNFIT || named_for_entry(h, needle, entry, pages) {
            continue;
        }
        refused_a_fit |= *distance <= tolerance;
        *distance = UNFIT;
    }
    refused_a_fit
}

#[cfg(test)]
#[path = "play_native_dated_name_test.rs"]
mod tests;
