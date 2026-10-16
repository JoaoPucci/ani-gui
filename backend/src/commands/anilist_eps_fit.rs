//! Whether an AniList streaming listing describes the show whose
//! episode page it would fill.
//!
//! AniList attaches Crunchyroll listings to the wrong entry often
//! enough to matter: every Gintama TV entry carries the 2006 series'
//! first 49 episodes, sequels carry their first season's listing, and
//! split cours carry the whole season. Filling stills from such a
//! listing puts another show's episodes on the page, so the backfill
//! asks this rule first. `docs/title-resolution.md` carries the case
//! table the rule was chosen from.

use std::collections::{HashMap, HashSet};

use crate::meta::anilist_streaming_eps::StreamingListing;
use crate::meta::kitsu::KitsuEpisode;

/// Fewest compared titles that decide on their own. Below it a few
/// differently worded titles could refuse a right listing, so the
/// episode count decides instead.
const MIN_COMPARED: usize = 3;

/// Shortest squashed title that counts as contained in another.
const MIN_CONTAINED: usize = 3;

/// Words too common in episode titles to say two titles are the same
/// episode.
const STOP_WORDS: &[&str] = &["the", "and", "for", "from", "with", "part", "episode"];

/// `true` when `listing` may fill stills on `page`.
///
/// Where Kitsu titles at least [`MIN_COMPARED`] of the page's
/// episodes the listing also titles, the titles decide: the listing
/// fits when at least half of them agree ([`titles_agree`]), so a
/// minority worded differently never refuses it. Otherwise the shape
/// decides: a finished show refuses a listing numbered past its
/// episode count.
#[must_use]
pub fn listing_fits(listing: &StreamingListing, page: &[KitsuEpisode]) -> bool {
    let pairs = compared_titles(listing, page);
    if pairs.len() >= MIN_COMPARED {
        let agreeing = titles_agree(&pairs);
        return agreeing * 2 >= pairs.len();
    }
    shape_fits(listing)
}

/// A finished show with a known count refuses a listing whose highest
/// episode number is past it. An ongoing show, or one AniList has no
/// count for, keeps any listing.
fn shape_fits(listing: &StreamingListing) -> bool {
    let Some(count) = listing.episodes.filter(|_| listing.finished) else {
        return true;
    };
    listing
        .entries
        .last_key_value()
        .is_none_or(|(highest, _)| *highest <= count)
}

/// `(Kitsu title, listing title)` for every page episode both title.
/// A Kitsu placeholder ("Episode 5") is no title.
fn compared_titles<'a>(
    listing: &'a StreamingListing,
    page: &'a [KitsuEpisode],
) -> Vec<(&'a str, &'a str)> {
    page.iter()
        .filter_map(|ep| {
            let ours = ep.canonical_title.as_deref()?;
            let theirs = listing.entries.get(&ep.number?)?.title.as_str();
            (!is_placeholder(ours) && !squashed(theirs).is_empty()).then_some((ours, theirs))
        })
        .collect()
}

/// A title with no words, or only the word "episode" and a number.
fn is_placeholder(title: &str) -> bool {
    let words: Vec<String> = tokens(title).collect();
    match words.as_slice() {
        [] => true,
        [word, number] => word == "episode" && number.chars().all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

/// One squashed title holds the other, and the held one is at least
/// [`MIN_CONTAINED`] characters long: "go" is inside nearly anything.
fn contains_other(a: &str, b: &str) -> bool {
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    short.chars().count() >= MIN_CONTAINED && long.contains(short)
}

/// How many of `pairs` name the same episode: one title's letters and
/// digits contain the other's ([`contains_other`]), or the two share a
/// distinctive word. A word in more than a quarter of the pairs — in
/// more than one, when fewer than eight are compared — is the show's
/// own vocabulary ("Kaguya Wants to …") and is not distinctive.
fn titles_agree(pairs: &[(&str, &str)]) -> usize {
    let words: Vec<(HashSet<String>, HashSet<String>)> = pairs
        .iter()
        .map(|(a, b)| (words_of(a), words_of(b)))
        .collect();
    let mut spread: HashMap<&str, usize> = HashMap::new();
    for (a, b) in &words {
        for w in a.union(b) {
            *spread.entry(w.as_str()).or_default() += 1;
        }
    }
    let common = (pairs.len() / 4).max(1);
    pairs
        .iter()
        .zip(&words)
        .filter(|((a, b), (wa, wb))| {
            contains_other(&squashed(a), &squashed(b))
                || wa
                    .intersection(wb)
                    .any(|w| spread.get(w.as_str()).is_some_and(|n| *n <= common))
        })
        .count()
}

/// Lowercased runs of letters and digits.
fn tokens(title: &str) -> impl Iterator<Item = String> + '_ {
    title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
}

/// The title's letters and digits, lowercased, with nothing between.
fn squashed(title: &str) -> String {
    tokens(title).collect()
}

/// The title's words of three or more characters, stop words out.
fn words_of(title: &str) -> HashSet<String> {
    tokens(title)
        .filter(|w| w.chars().count() >= 3 && !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

#[cfg(test)]
#[path = "anilist_eps_fit_test.rs"]
mod tests;
