//! What follows a broad listing's stem in a title, and the number a
//! title carries where the broad listing's own stands — split from
//! `play_native_title_reading` for the per-file complexity bar.

use super::*;

/// What follows `wide`'s stem in `title`, when `title` starts with it
/// at a word's end or with a digit or 第 glued to it — as Japanese
/// writes them ("ショー２", "ショー第2期"), though any glued digit
/// counts ("Show2") — except after a stem that itself ends in a digit
/// ([`rest_after`]). "Showtime 2" does not start with "Show". With it,
/// the divisions `wide` ends on, which are `wide`'s own.
///
/// A bare number `wide`'s stem ends on stays in the stem, so "Lucky 2
/// 2nd Season" is read past "Lucky 2". A title that does not start
/// with the whole stem is read past it without the number, then one of
/// `wide`'s own divisions: "Show Season 3" beside "Show 2" names a
/// season 3, as it does beside "Show 2nd Season". A title whose number
/// in its place only continues it or is lower ("Show 20 Part 2") is not
/// read past it ([`leading_number`]); the same or a higher one is.
pub(crate) fn past_stem(title: &str, wide: &str) -> Option<(String, Vec<Marker>)> {
    let own = stem(wide);
    let text = words(title).join(" ");
    let mut wide_markers = trailing_markers(wide);
    if let Some(rest) = rest_after(&text, &own.join(" ")) {
        return Some((rest, wide_markers));
    }
    let (shorter, n) = without_stem_number(&own)?;
    let rest = rest_after(&text, &shorter)?;
    // The number a title carries where W's stands: one that only
    // continues W's digits ("Show 20 Part 2", "ショー２０第2部" beside
    // "Show 2", "ショー２") or is lower is another number, not W's stem
    // without one; the same number or a higher one is read on.
    let other =
        |(m, digits): (u32, String)| m < n || (m != n && digits.starts_with(&n.to_string()));
    if leading_number(&rest).is_some_and(other) {
        return None;
    }
    wide_markers.push(Marker {
        kind: Kind::Season,
        ordinals: vec![n],
    });
    Some((rest, wide_markers))
}

/// The number `rest` opens on, and its digits: a run of digits
/// standing as a number — ending the text, before a space, or before a
/// Japanese letter glued to it ("20第2部") — not an ordinal's ("2nd")
/// nor a word's ("1é"). Compared as a number against W's, so a
/// zero-padded "02" is the number 2.
pub(crate) fn leading_number(rest: &str) -> Option<(u32, String)> {
    let rest = rest.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let after = rest[digits.len()..].chars().next();
    (!digits.is_empty() && after.is_none_or(|c| c == ' ' || is_japanese(c)))
        .then(|| digits.parse().ok().map(|m| (m, digits)))
        .flatten()
}

/// What follows `own` at the start of `text`, when `own` ends there
/// at a word's end or with a digit or 第 glued to it. A digit glued to
/// an `own` that itself ends in one continues that number rather than
/// following it: "Show 20" does not start with "Show 2".
pub(crate) fn rest_after(text: &str, own: &str) -> Option<String> {
    let rest = text.strip_prefix(own)?;
    let numbered = own.chars().last().is_some_and(|c| c.is_ascii_digit());
    rest.chars()
        .next()
        .is_none_or(|c| c == ' ' || c == '第' || (c.is_ascii_digit() && !numbered))
        .then(|| rest.to_string())
}

/// A stem with the bare number it ends on taken off — a word of its
/// own ("Show 2") or glued to a Japanese word ("ショー２") — and that
/// number, when something of the stem is left.
pub(crate) fn without_stem_number(stem: &[String]) -> Option<(String, u32)> {
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
