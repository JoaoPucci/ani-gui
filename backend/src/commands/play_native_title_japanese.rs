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

/// Each of `words` read apart ([`apart`]).
pub(crate) fn apart_glued(words: Vec<String>) -> Vec<String> {
    words.iter().flat_map(|w| apart(w)).collect()
}

/// One word read apart: what it holds before the Japanese divisions
/// glued to its end and each division in turn, and a number glued to
/// the end of a Japanese letter — a division's last letter included —
/// as a word of its own after it: "fourth第十二部" is "fourth" and
/// "第十二部", "ショー第2期第2部" is "ショー", "第2期" and "第2部",
/// "第3期2" is "第3期" and "2", "ショー2" is "ショー" and "2".
fn apart(word: &str) -> Vec<String> {
    let mut glued = Vec::new();
    let mut head = word;
    while let Some((_, before)) = japanese_trailing(head) {
        glued.push(head[before.len()..].to_string());
        head = before;
    }
    let digits = head.len() - head.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (front, number) = head.split_at(head.len() - digits);
    let mut out = if digits > 0 && front.chars().last().is_some_and(is_japanese) {
        let mut out = apart(front);
        out.push(number.to_string());
        out
    } else if head.is_empty() {
        Vec::new()
    } else {
        vec![head.to_string()]
    };
    out.extend(glued.into_iter().rev());
    out
}
