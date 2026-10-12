//! The Japanese divisions a title ends on (第N期, 第N部, 第Nクール) —
//! split from `play_native_title_grammar` for the per-file complexity
//! bar.

use super::*;

/// The 第N期 / 第N部 / 第Nクール a title ends on, closing brackets
/// aside, with the text before its 第.
pub(crate) fn japanese_trailing(title: &str) -> Option<(Marker, &str)> {
    let trimmed = title.trim_end_matches(|c: char| c.is_whitespace() || ")）]】".contains(c));
    let (body, kind) = [
        ("期", Kind::Season),
        ("部", Kind::Part),
        ("クール", Kind::Part),
    ]
    .iter()
    .find_map(|(suffix, kind)| trimmed.strip_suffix(suffix).map(|b| (b, *kind)))?;
    let (before, number) = body.rsplit_once('第')?;
    let number: String = number.chars().map(half_width).collect();
    let marker = Marker {
        kind,
        ordinals: vec![japanese_number(&number)?],
    };
    Some((marker, before))
}

/// The number written after 第: ASCII digits, or a kanji number,
/// compound ones included ("十二").
pub(crate) fn japanese_number(text: &str) -> Option<u32> {
    number(text).or_else(|| kanji_number(text))
}
