//! Whether a provider's title names a Kitsu entry outright — one of
//! the entry's own titles, up to case, punctuation and how the cour
//! suffix is spelled.
//!
//! The cross-cour guard reads a Kitsu entry's cour off its slug, and a
//! slug without a suffix reads as cour 1. That is the signal that
//! caught the Stone Ocean Part 2 → Part 1 pairing, and it is wrong for
//! every entry whose slug marks a later season some other way:
//! Gintama' is `gintama-2`, Gintama° `gintama-2015-tv`, Attack on
//! Titan Season 2 `shingeki-no-kyojin-2`. Kitsu titles those entries
//! "Gintama Season 2", "Gintama Season 4", "Attack on Titan Season 2"
//! — the very names the provider lists them under. A title the entry
//! itself carries is no evidence of another cour, so the guard does
//! not refuse it.
//!
//! The comparison is the whole name, not the cour number alone. Read
//! the number by itself and "Attack on Titan Season 3 Part 2" agrees
//! with the Season 2 entry (both "2"), and "Stone Ocean Part 3" with a
//! Stardust Crusaders entry Kitsu also calls "JoJo's Bizarre Adventure
//! Part 3".

/// The ten spelled ordinals the cour parsers read.
const ORDINAL_WORDS: [&str; 10] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
];

/// Whether `provider_title` is one of `entry_titles`: the same words
/// once case and punctuation are folded, or the same name before a
/// trailing cour suffix with the same cour number however the suffix
/// is spelled ("Season 2", "2nd Season", "Second Season", "Part 2",
/// "Cour 2"). A title with no words names nothing.
#[must_use]
pub fn names_the_entry<'a>(
    provider_title: &str,
    entry_titles: impl IntoIterator<Item = &'a str>,
) -> bool {
    let wanted = words(provider_title);
    if wanted.is_empty() {
        return false;
    }
    let wanted_cour = split_cour(&wanted);
    entry_titles.into_iter().any(|t| {
        let have = words(t);
        have == wanted || (wanted_cour.is_some() && split_cour(&have) == wanted_cour)
    })
}

/// The title's words: lowercased, split on everything that is not a
/// letter or a digit.
fn words(title: &str) -> Vec<String> {
    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

/// The name before a trailing cour suffix, and the cour it names —
/// `[.., season|part|cour, N]`, `[.., Nst|Nnd|Nrd|Nth, keyword]` or
/// `[.., second, keyword]`. None without a suffix, or when nothing
/// precedes it.
fn split_cour(words: &[String]) -> Option<(&[String], u32)> {
    let [name @ .., a, b] = words else {
        return None;
    };
    if name.is_empty() {
        return None;
    }
    let cour = if super::cour::is_cour_keyword(a) {
        b.parse().ok()
    } else if super::cour::is_cour_keyword(b) {
        ordinal(a)
    } else {
        None
    }?;
    Some((name, cour))
}

/// A cour written as an ordinal: a number with its suffix, or one of
/// the spelled ordinals.
fn ordinal(word: &str) -> Option<u32> {
    if let Some(i) = ORDINAL_WORDS.iter().position(|o| *o == word) {
        return u32::try_from(i + 1).ok();
    }
    let digits = word.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &word[digits.len()..];
    if digits.is_empty() || !matches!(suffix, "st" | "nd" | "rd" | "th") {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
#[path = "cour_same_name_test.rs"]
mod tests;
