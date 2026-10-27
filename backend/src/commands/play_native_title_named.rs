//! What a title names: every ordinal anywhere in it, its parts, the
//! number its stem ends on and the one ordinal a part's tail names —
//! split from `play_native_title_grammar` for the per-file complexity
//! bar.

use super::*;

/// The ordinal `rest` names when it is nothing but one marker naming
/// one ordinal — "2nd Stage", "Part 3", ": Season 2", "II", "第2期".
/// `None` for anything else, a span or two markers included.
pub(crate) fn sole_ordinal(rest: &str) -> Option<u32> {
    let (before, markers) = parse_tail(rest);
    match (before.is_empty(), markers.as_slice()) {
        (true, [only]) if only.ordinals.len() == 1 => Some(only.ordinals[0]),
        _ => None,
    }
}

/// Every ordinal a title names anywhere: each season or part marker,
/// every small number or numeral in it ("Tokyo Ghoul:re 2", "Mushoku
/// Tensei II: Isekai Ittara Honki Dasu"), a Japanese 第N, and a number
/// written before a Japanese counter (the ８ of "怪獣８号"). Read as
/// evidence of what an entry names, so reading too much only ever
/// admits more.
pub(crate) fn named_ordinals(title: &str) -> BTreeSet<u32> {
    let words = apart_glued(words(title));
    let mut out: BTreeSet<u32> = words
        .windows(2)
        .filter_map(|w| marker_of(&w[0], &w[1]))
        .flat_map(|m| m.ordinals)
        .collect();
    out.extend(
        words
            .iter()
            .filter_map(|w| small_number(w).or_else(|| spelled_ordinal(w))),
    );
    // Japanese writes a number glued to the word beside it, with no
    // space to make it a word of its own ("…生活２", "86第4部").
    out.extend(words.iter().flat_map(|w| standing_numbers(w)));
    let half = with_kanji_numbers(
        &untagged(title)
            .trim_end()
            .chars()
            .map(half_width)
            .collect::<String>(),
    );
    out.extend(counted_numbers(&half));
    for (_, after) in half
        .match_indices('第')
        .map(|(i, m)| half.split_at(i + m.len()))
    {
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        out.extend(number(&digits));
    }
    out
}

/// The part ordinals a title ends on.
pub(crate) fn part_ordinals(title: &str) -> BTreeSet<u32> {
    trailing_markers(title)
        .into_iter()
        .filter(|m| m.kind == Kind::Part)
        .flat_map(|m| m.ordinals)
        .collect()
}

/// The bare number a title's stem ends on, one or two digits: "Show
/// 2", "Show 2 Part 2", the ２ glued to "ショー２". A sequel's number,
/// as a spelled ordinal standing alone is.
pub(crate) fn stem_number(title: &str) -> Option<u32> {
    let own = stem(title);
    let last = own.last()?;
    // a stem's words are read apart, so a number glued to a Japanese
    // word is a word of its own by now
    small_number(last)
}
