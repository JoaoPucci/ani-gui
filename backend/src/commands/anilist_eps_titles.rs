//! How two episode titles are compared when deciding whether an
//! AniList streaming listing is the show's (`super::fit`): their
//! words, squashed forms, and whether a pair names the same episode.

use std::collections::{HashMap, HashSet};

/// Shortest squashed title that counts as contained in another.
const MIN_CONTAINED: usize = 3;

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

/// Lowercased runs of letters and digits, a spelled-out number
/// ([`NUMBER_WORDS`]) written as digits.
fn tokens(title: &str) -> impl Iterator<Item = String> + '_ {
    title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let w = w.to_lowercase();
            NUMBER_WORDS
                .iter()
                .position(|n| *n == w)
                .map_or(w, |n| n.to_string())
        })
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
