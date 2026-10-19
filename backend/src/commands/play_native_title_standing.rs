//! The numbers a title writes standing as numbers, where Japanese glues
//! them to the words beside them — split from `play_native_title_named`
//! for the per-file complexity bar.

use super::*;

/// Every number of one or two digits `word` writes standing as a
/// number — each run of digits with nothing but a Japanese letter, or
/// the "+" of a span, glued to it on either side ("ショー2", "86第4部",
/// "4+5"), a suffixed one ("ショー2nd") included — and never one glued
/// to any other letter, which is that word's own ("Arc2", "Café2").
pub(crate) fn standing_numbers(word: &str) -> Vec<u32> {
    let chars: Vec<char> = word.chars().collect();
    let apart = |c: Option<&char>| c.is_none_or(|&c| c == '+' || is_japanese(c));
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let digits: String = chars[start..i].iter().collect();
        let suffixed = ["st", "nd", "rd", "th"].iter().find_map(|s| {
            let s: Vec<char> = s.chars().collect();
            (chars.get(i..i + 2) == Some(&s[..])).then_some(i + 2)
        });
        let end = if apart(chars.get(i)) {
            Some(i)
        } else {
            suffixed
        };
        let left = start == 0 || apart(chars.get(start - 1));
        if left && end.is_some_and(|e| apart(chars.get(e))) {
            // a suffixed ordinal is read up to three digits, a bare
            // number up to two
            out.extend(if end == Some(i) {
                small_number(&digits)
            } else {
                number(&digits)
            });
        }
    }
    out
}

/// Every number of one or two digits written straight before a
/// Japanese counter — 号, 期 or 話 — in `half` (a title with its
/// full-width digits already read as ASCII): the 8 of "怪獣8号".
pub(crate) fn counted_numbers(half: &str) -> Vec<u32> {
    const COUNTERS: &[char] = &['号', '期', '話'];
    let mut out = Vec::new();
    let mut run = String::new();
    for c in half.chars() {
        if c.is_ascii_digit() {
            run.push(c);
            continue;
        }
        if COUNTERS.contains(&c) {
            out.extend(small_number(&run));
        }
        run.clear();
    }
    out
}
