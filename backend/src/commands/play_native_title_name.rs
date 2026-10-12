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

/// The divisions `sibling` opens what follows `wide`'s stem with,
/// leaving out any `wide` itself ends on, and the season `wide`
/// carries — the highest season among the divisions it ends on, the
/// bare number its stem ends on included when `sibling` is read past
/// the stem without it ([`past_stem`]); the first when it carries
/// none. `None` when `sibling` does not start with the stem.
pub(crate) fn divisions_beside(sibling: &str, wide: &str) -> Option<(Vec<Marker>, u32)> {
    let (rest, wide_markers) = past_stem(sibling, wide)?;
    let season = wide_markers
        .iter()
        .filter(|m| m.kind == Kind::Season)
        .flat_map(|m| m.ordinals.iter().copied())
        .max()
        .unwrap_or(1);
    let named = scan(&words(&rest), true)
        .into_iter()
        .filter(|m| !wide_markers.contains(m))
        .collect();
    Some((named, season))
}
