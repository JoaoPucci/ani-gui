//! Whether a Kitsu search hit may be the show a row's titles name, by
//! the words its titles share with them. Kitsu's text search answers a
//! title it does not carry with its closest words whatever they are:
//! "There Is Also a Hole in the Student Organization!" brings back Here
//! is Greenwood, which shares only "is" with it. A resolve that stores
//! its answer as a show's mapping first refuses a hit that shares too
//! few words with the row, by the rule below.
//!
//! The frontend's Continue resolution applies the same rule
//! (`frontend/src/lib/history/title-words.ts`); both run the vectors in
//! `tests/fixtures/title-words/vectors.json`. The rule:
//!
//! - A title's words are its runs of ASCII letters and digits, ASCII
//!   letters folded to lowercase, articles (the, a, an) left out. Any
//!   other character separates words, so a title in another script has
//!   none.
//! - Sequel markers are not words either: season, part and cour (and
//!   their plurals), first to tenth, the roman numerals ii to ix but
//!   for v, numbers, and ordinals written with digits (2nd). Two
//!   shows' second seasons share those and nothing else.
//! - The hit's titles are its canonical title, its localized titles,
//!   its abbreviations and its slug's words.
//! - A row title and a hit title share words when the shared words
//!   reach a third of both, when either title's words (two or more) all
//!   appear in the other, or when the hit title is one word that is the
//!   row title's first, five letters or longer.
//! - A hit is refused only when titles were compared and no pair shares
//!   words. With nothing to compare, nothing is refused.

use std::collections::HashSet;

use crate::meta::kitsu::KitsuAnimeRef;

const MIN_SHARE: f64 = 0.34;
const LEADING_WORD_MIN: usize = 5;
const ARTICLES: [&str; 3] = ["the", "a", "an"];
const SEQUEL_MARKERS: [&str; 23] = [
    "season", "seasons", "part", "parts", "cour", "cours", "first", "second", "third", "fourth",
    "fifth", "sixth", "seventh", "eighth", "ninth", "tenth", "ii", "iii", "iv", "vi", "vii",
    "viii", "ix",
];

/// A number, or an ordinal written with digits (`2nd`).
fn is_number_or_ordinal(w: &str) -> bool {
    let digits = w.trim_end_matches(|c: char| c.is_ascii_lowercase());
    let suffix = &w[digits.len()..];
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && matches!(suffix, "" | "st" | "nd" | "rd" | "th")
}

fn is_word(w: &str) -> bool {
    !w.is_empty()
        && !ARTICLES.contains(&w)
        && !SEQUEL_MARKERS.contains(&w)
        && !is_number_or_ordinal(w)
}

/// A title's words, in order.
fn words(title: &str) -> Vec<String> {
    title
        .to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
        .filter(|w| is_word(w))
        .map(str::to_owned)
        .collect()
}

fn hit_titles(hit: &KitsuAnimeRef) -> Vec<String> {
    std::iter::once(hit.canonical_title.clone())
        .chain(hit.titles.values().cloned())
        .chain(hit.abbreviated_titles.iter().cloned())
        .chain(hit.slug.as_deref().map(|s| s.replace('-', " ")))
        .collect()
}

fn pair_shares(row: &HashSet<String>, hit: &HashSet<String>, lead: &str) -> bool {
    let shared = row.intersection(hit).count();
    let share = (shared as f64 / row.len() as f64).min(shared as f64 / hit.len() as f64);
    share >= MIN_SHARE
        || (shared == row.len() && row.len() >= 2)
        || (shared == hit.len() && hit.len() >= 2)
        || (hit.len() == 1 && lead.len() >= LEADING_WORD_MIN && hit.contains(lead))
}

/// Whether `hit` may be the show the row's `titles` name.
#[must_use]
pub(crate) fn shares_words(titles: &[&str], hit: &KitsuAnimeRef) -> bool {
    let candidates: Vec<HashSet<String>> = hit_titles(hit)
        .iter()
        .map(|t| words(t).into_iter().collect::<HashSet<_>>())
        .filter(|c| !c.is_empty())
        .collect();
    let mut compared = false;
    for title in titles {
        let words = words(title);
        let Some(lead) = words.first() else {
            continue;
        };
        let row: HashSet<String> = words.iter().cloned().collect();
        for c in &candidates {
            compared = true;
            if pair_shares(&row, c, lead) {
                return true;
            }
        }
    }
    !compared
}

#[cfg(test)]
#[path = "kitsu_title_words_test.rs"]
mod tests;
