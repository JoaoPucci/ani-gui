//! A title's name and how a title stands to a broad listing's stem —
//! split from `play_native_title_reading` for the per-file complexity
//! bar.

use super::*;

/// A title's name: its words up to the first division it names
/// ([`division_at`]), with what a Japanese word holds before a
/// division glued to it — the words a show's seasons and parts share
/// however each is numbered. "Show", "Show 2", "Show 2nd Season Part
/// 2" and "Show II Part 2: Arc" are all named "Show"; "Other Show
/// Season 2" is named "Other Show"; "ショー２" and "ショー第2期" are
/// named "ショー". A word ordinal standing alone with more than
/// divisions after it is a word of the name ([`inside_the_name`]):
/// "The Seventh Son" is named "The Seventh Son". A title that opens on
/// a division ("86") is named by its stem.
pub(crate) fn name(title: &str) -> Vec<String> {
    let words = words(title);
    let mut out = Vec::new();
    for (i, w) in words.iter().enumerate() {
        if division_at(&words, i).is_none() || inside_the_name(&words, i) {
            out.push(w.clone());
            continue;
        }
        let head = glued_divisions(w).map_or_else(
            || w.trim_end_matches(|c: char| c.is_ascii_digit()),
            |(_, before)| before,
        );
        if !head.is_empty() && head != w {
            out.push(head.to_string());
        }
        break;
    }
    if out.is_empty() {
        stem(title)
    } else {
        out
    }
}

/// Whether `title` starts with `other`'s name ([`name`]), on the terms
/// of [`past_stem`]: "Show 2nd Season" and "Show Final Arc 2nd Season"
/// start with the name of "Show Part 2"; "Other Show 2nd Season" and
/// "Showtime 2" do not.
pub(crate) fn starts_with_name_of(title: &str, other: &str) -> bool {
    rest_after(&words(title).join(" "), &name(other).join(" ")).is_some()
}

/// Whether `candidate` is `title`'s name ([`name`]) as `title` writes
/// it, marks included, followed by first divisions alone or by
/// nothing:
/// "Show", "Show Season 1" and "Haikyuu!!" for "Show 2" and "Haikyuu!!
/// 2nd Season" are; "Gintama'", "Kaguya-sama wa Kokurasetai?",
/// "Steins;Gate Zero" and "Show: Season 1" for "Gintama 2", "Kaguya-sama
/// wa Kokurasetai 2", "Steins;Gate 0" and "Show 2" are not — each adds
/// a word or a mark to the name.
pub(crate) fn is_name_and_first_divisions(candidate: &str, title: &str) -> bool {
    let own = normalized(title);
    let named = name(title);
    let literal = own
        .char_indices()
        .map(|(i, c)| i + c.len_utf8())
        .filter(|&end| words(&own[..end]) == named)
        .next_back()
        .map(|end| own[..end].trim_end());
    literal
        .and_then(|literal| rest_after(&normalized(candidate), literal))
        .is_some_and(|rest| first_divisions_only(&words(&rest)))
}

/// Whether `words` are first divisions and nothing else, each read by
/// `division_at`: "Season 1", "1st Season Part 1", or no words at all.
fn first_divisions_only(words: &[String]) -> bool {
    let mut j = 0;
    while j < words.len() {
        match division_at(words, j) {
            Some((m, taken)) if m.ordinals.iter().all(|&o| o == 1) => j += taken,
            _ => return false,
        }
    }
    true
}

/// Whether `title` opens what follows `wide`'s stem ([`past_stem`])
/// on first divisions alone — "Show Part 1", "Show Season 1", "Show
/// First Half" beside "Show" — leaving out any `wide` itself ends on:
/// `wide`'s own first part.
pub(crate) fn opens_on_a_first_division(title: &str, wide: &str) -> bool {
    let named = later_division_markers(title, wide);
    !named.is_empty() && named.iter().all(|m| m.ordinals.first() == Some(&1))
}

/// Whether `title` starts with `wide`'s stem ([`past_stem`]) and
/// opens what follows it with a division ([`scan`], a first one
/// included): "Show 2", "Show Part 1" and "Show 2nd Season Part 2"
/// beside "Show" do; "Show Side Story 3" (a spinoff), "Show: Final Arc
/// Season 2" and "Showtime 2" do not.
pub(crate) fn names_past_stem(title: &str, wide: &str) -> bool {
    past_stem(title, wide).is_some_and(|(rest, _)| !scan(&words(&rest), true).is_empty())
}

/// Whether the division at `words[i]` is a word ordinal standing alone
/// ("Seventh", not "Seventh Season", "VII" or "7th") with something
/// other than divisions after it — a word of the name, as in "The
/// Seventh Son", not the title's division.
fn inside_the_name(words: &[String], i: usize) -> bool {
    let w = &words[i];
    let word_ordinal = spelled_ordinal(w).is_some()
        && w.chars().all(|c| c.is_ascii_alphabetic())
        && !w.chars().all(|c| "ivx".contains(c));
    let standing_alone = division_at(words, i).is_some_and(|(_, taken)| taken == 1);
    word_ordinal && standing_alone && !divisions_only(&words[i + 1..])
}

/// Whether `words` are divisions and nothing else, each read by
/// `division_at`.
fn divisions_only(words: &[String]) -> bool {
    let mut j = 0;
    while j < words.len() {
        match division_at(words, j) {
            Some((_, taken)) => j += taken,
            None => return false,
        }
    }
    true
}
