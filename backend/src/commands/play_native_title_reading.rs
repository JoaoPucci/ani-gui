//! The one reader the spanning cut compares titles with: what a title
//! names past a broad listing's stem, and how a title reads as a
//! sequence of divisions — split from `play_native_title_grammar`,
//! whose marker parser it reads through, for the per-file complexity
//! bar.

use super::play_native_title_grammar::{
    glued_number, japanese_trailing, kind_of, marker_of, ordinals_after_marker, small_number,
    spelled_ordinal, stem, trailing_markers, words, Kind, Marker,
};

/// The ordinals `sibling` names as divisions of its own right after
/// `wide`'s stem, leaving out any division `wide` itself ends on:
/// "Attack on Titan Season 3 Part 2" beside "Attack on Titan Season
/// 3" names part 2; "Gintama.: Silver Soul Arc - Second Half War"
/// beside "Gintama.: Silver Soul Arc" names 2; "Show Season 3 Recap"
/// beside "Show Season 3" and "Show Side Story 2" beside "Show" name
/// nothing. Empty when `sibling` does not start with the stem.
pub(super) fn later_divisions(sibling: &str, wide: &str) -> Vec<u32> {
    later_division_markers(sibling, wide)
        .into_iter()
        .flat_map(|m| m.ordinals)
        .collect()
}

/// The divisions behind [`later_divisions`], with their kinds: those
/// [`divisions`] reads at the start of what follows `wide`'s stem
/// ([`past_stem`]), up to the first word that names none, leaving out
/// any `wide` itself ends on. The one reader every title goes through.
pub(super) fn later_division_markers(sibling: &str, wide: &str) -> Vec<Marker> {
    let Some((rest, wide_markers)) = past_stem(sibling, wide) else {
        return Vec::new();
    };
    scan(&words(&rest), true)
        .into_iter()
        .filter(|m| !wide_markers.contains(m))
        .collect()
}

/// What follows `wide`'s stem in `title`, when `title` starts with it
/// at a word's end or with a digit or 第 glued to it — as Japanese
/// writes them ("ショー２", "ショー第2期"), though any glued digit
/// counts ("Show2"). "Showtime 2" does not start with "Show". With it,
/// the divisions `wide` ends on, which are `wide`'s own.
///
/// A bare number `wide`'s stem ends on stays in the stem, so "Lucky 2
/// 2nd Season" is read past "Lucky 2". A title that does not start
/// with the whole stem is read past it without the number, then one of
/// `wide`'s own divisions: "Show Season 3" beside "Show 2" names a
/// season 3, as it does beside "Show 2nd Season".
fn past_stem(title: &str, wide: &str) -> Option<(String, Vec<Marker>)> {
    let own = stem(wide);
    let text = words(title).join(" ");
    let mut wide_markers = trailing_markers(wide);
    if let Some(rest) = rest_after(&text, &own.join(" ")) {
        return Some((rest, wide_markers));
    }
    let (shorter, n) = without_stem_number(&own)?;
    let rest = rest_after(&text, &shorter)?;
    wide_markers.push(Marker {
        kind: Kind::Season,
        ordinals: vec![n],
    });
    Some((rest, wide_markers))
}

/// What follows `own` at the start of `text`, when `own` ends there
/// at a word's end or with a digit or 第 glued to it.
fn rest_after(text: &str, own: &str) -> Option<String> {
    let rest = text.strip_prefix(own)?;
    rest.chars()
        .next()
        .is_none_or(|c| c == ' ' || c.is_ascii_digit() || c == '第')
        .then(|| rest.to_string())
}

/// A stem with the bare number it ends on taken off — a word of its
/// own ("Show 2") or glued to a Japanese word ("ショー２") — and that
/// number, when something of the stem is left.
fn without_stem_number(stem: &[String]) -> Option<(String, u32)> {
    let (last, before) = stem.split_last()?;
    if let Some(n) = small_number(last) {
        return (!before.is_empty()).then(|| (before.join(" "), n));
    }
    let n = glued_number(last)?;
    let head = last.trim_end_matches(|c: char| c.is_ascii_digit());
    let mut words = before.to_vec();
    words.push(head.to_string());
    Some((words.join(" "), n))
}

/// Every division `words` name, first word to last: a marker pair, a
/// Japanese division (apart or glued to the word before it), and an
/// ordinal standing alone — spelled ("Second", "II"), bare ("2") or a
/// number glued to a Japanese title ("ショー2"). A numeral right
/// before a marker that carries its own ordinal is a division of its
/// own: "II Part 2" is a season 2 and a part 2, not a part 2 and a
/// stray 2.
fn divisions(words: &[String]) -> Vec<Marker> {
    scan(words, false)
}

/// [`divisions`], stopping at the first word that names none when
/// `leading` holds: the divisions a title opens with.
fn scan(words: &[String], leading: bool) -> Vec<Marker> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = &words[i];
        let own_marker_follows = kind_of(w).is_none()
            && words.get(i + 1).is_some_and(|k| kind_of(k).is_some())
            && words
                .get(i + 2)
                .is_some_and(|o| ordinals_after_marker(o).is_some());
        let pair = words.get(i + 1).and_then(|next| marker_of(w, next));
        if let (Some(m), false) = (pair, own_marker_follows) {
            out.push(m);
            i += 2;
            continue;
        }
        if let Some((m, _)) = japanese_trailing(w) {
            out.push(m);
        } else if let Some(n) = spelled_ordinal(w)
            .or_else(|| small_number(w))
            .or_else(|| glued_number(w))
        {
            out.push(Marker {
                kind: Kind::Season,
                ordinals: vec![n],
            });
        } else if leading {
            break;
        }
        i += 1;
    }
    out
}

/// How a title reads: the ordinals of every division it names
/// ([`divisions`]), in order, a division whose first ordinal is 1 left
/// out as the whole ("Part 1", "First Half", "(Part 1+2)"). The one
/// reader the spanning cut compares titles with: "Show 2", "Show 2nd
/// Season" and "ショー 第2期" read `[2]`; "Show", "Show Part 1" and
/// "Show First Half" read `[]`; "Lucky 2 2nd Season" reads `[2, 2]`;
/// "Show II Part 2: Arc" reads `[2, 2]`. What follows a division is
/// read too, so "Show 2nd Season Recap" reads `[2]`.
pub(super) fn reading(title: &str) -> Vec<u32> {
    divisions(&words(title))
        .into_iter()
        .filter(|m| m.ordinals.first() != Some(&1))
        .flat_map(|m| m.ordinals)
        .collect()
}

/// Whether `title` opens what follows `wide`'s stem ([`past_stem`])
/// on first divisions alone — "Show Part 1", "Show Season 1", "Show
/// First Half" beside "Show" — leaving out any `wide` itself ends on:
/// `wide`'s own first part.
pub(super) fn opens_on_a_first_division(title: &str, wide: &str) -> bool {
    let named = later_division_markers(title, wide);
    !named.is_empty() && named.iter().all(|m| m.ordinals.first() == Some(&1))
}

/// Whether `title` starts with `wide`'s stem ([`past_stem`]) and
/// opens what follows it with a division ([`scan`], a first one
/// included): "Show 2", "Show Part 1" and "Show 2nd Season Part 2"
/// beside "Show" do; "Show Side Story 3" (a spinoff), "Show: Final Arc
/// Season 2" and "Showtime 2" do not.
pub(super) fn names_past_stem(title: &str, wide: &str) -> bool {
    past_stem(title, wide).is_some_and(|(rest, _)| !scan(&words(&rest), true).is_empty())
}
