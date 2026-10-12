//! The entry before this one, read from a title — split from
//! `play_native_title_name` for the per-file complexity bar.

use super::*;

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
/// `division_at`, with the "Half" a first half writes after its
/// ordinal: "Season 1", "1st Season Part 1", "First Half", or no words
/// at all.
fn first_divisions_only(words: &[String]) -> bool {
    let mut j = 0;
    while j < words.len() {
        match division_at(words, j) {
            Some((m, taken)) if m.ordinals.iter().all(|&o| o == 1) => {
                j += taken;
                j += usize::from(words.get(j).is_some_and(|w| w == "half"));
            }
            _ => return false,
        }
    }
    true
}
