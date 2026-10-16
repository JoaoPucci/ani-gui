//! How two episode titles are compared when deciding whether an
//! AniList streaming listing is the show's (`super::fit`): their
//! words, squashed forms, and whether a pair names the same episode.

use std::collections::{HashMap, HashSet};

use super::contain::{contains_other, Squashed};
use crate::commands::play_native_title_grammar::ascii_form;

/// Words too common in episode titles to say two titles are the same
/// episode.
const STOP_WORDS: &[&str] = &["the", "and", "for", "from", "with", "part", "episode"];

/// Numbers as titles spell them out, by value: "Part One" is "Part 1".
const NUMBER_WORDS: &[&str] = &[
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

/// A title with no words, or only the word "episode" and a number,
/// a decimal one included ("Episode 5", "Episode 5.5").
pub(super) fn is_placeholder(title: &str) -> bool {
    let words: Vec<String> = tokens(title).collect();
    match words.as_slice() {
        [] => true,
        [word, number @ ..] => {
            word == "episode"
                && (1..=2).contains(&number.len())
                && number.iter().all(|n| n.chars().all(|c| c.is_ascii_digit()))
        }
    }
}

/// How many of `pairs` name the same episode: one title's letters and
/// digits contain the other's ([`contains_other`]), or the two share a
/// distinctive word. A word in more than a quarter of the pairs — in
/// more than one, when fewer than eight are compared — is the show's
/// own vocabulary ("Kaguya Wants to …") and is not distinctive.
pub(super) fn titles_agree(pairs: &[(&str, &str)]) -> usize {
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

/// Lowercased runs of letters and digits, full-width forms read as
/// ASCII ([`ascii_form`]) and a spelled-out number ([`NUMBER_WORDS`])
/// written as digits.
fn tokens(title: &str) -> impl Iterator<Item = String> {
    let ascii: String = title.chars().map(ascii_form).collect();
    let words: Vec<String> = ascii
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let w = w.to_lowercase();
            NUMBER_WORDS
                .iter()
                .position(|n| *n == w)
                .map_or(w, |n| n.to_string())
        })
        .collect();
    words.into_iter()
}

/// The title's letters and digits, lowercased, with nothing between,
/// and where its words split.
fn squashed(title: &str) -> Squashed {
    Squashed::of_words(tokens(title))
}

/// The title's words of three or more characters, stop words out.
fn words_of(title: &str) -> HashSet<String> {
    tokens(title)
        .filter(|w| w.chars().count() >= 3 && !STOP_WORDS.contains(&w.as_str()))
        .collect()
}
