//! The entry before this one, read from a title — split from
//! `play_native_title_name` for the per-file complexity bar.

use super::*;

/// Whether `candidate` is `title`'s name ([`name`]), compared word by
/// word, followed by first divisions alone or by nothing, with the
/// marks written right after the name the title's own — a separator
/// counting as none ([`marks_after`]): "Show", "Show Season 1" and
/// "Show: Season 1" for "Show 2", "Haikyuu!!" for "Haikyuu!! 2nd
/// Season" and "Steins Gate" for "Steins;Gate 0" are; "Gintama'",
/// "Kaguya-sama wa Kokurasetai?" and "Steins;Gate Zero" for "Gintama
/// 2", "Kaguya-sama wa Kokurasetai 2" and "Steins;Gate 0" are not —
/// each adds a mark or a word to the name.
pub(crate) fn is_name_and_first_divisions(candidate: &str, title: &str) -> bool {
    let (candidate, title) = (without_tags(candidate), without_tags(title));
    let named = name(&title);
    let (Some(c), Some(t)) = (name_end(&candidate, &named), name_end(&title, &named)) else {
        return false;
    };
    marks_after(&candidate, c) == marks_after(&title, t)
        && first_divisions_only(&words(&candidate[c..]))
}

/// `title` normalized ([`normalized`]) without the bracketed release
/// years and format tags the provider adds to a title — a four-digit
/// year ("Gintama (2011)") or TV, ONA, OVA or OAD ("JoJo no Kimyou na
/// Bouken (TV)", "[OVA]") in round, square or lenticular brackets,
/// brackets and tag alike ASCII or full-width ("（ＴＶ）"). Other
/// bracketed words are left as they are.
fn without_tags(title: &str) -> String {
    let text = normalized(title);
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find(['(', '[', '（', '【']) {
        let after = &rest[open..];
        let inner_start = after.chars().next().map_or(1, char::len_utf8);
        let tag_end = after.find([')', ']', '）', '】']).filter(|&close| {
            let inner: String = after[inner_start..close]
                .trim()
                .chars()
                .map(ascii_form)
                .collect();
            (inner.len() == 4 && inner.chars().all(|c| c.is_ascii_digit()))
                || ["tv", "ona", "ova", "oad"].contains(&inner.as_str())
        });
        let Some(close) = tag_end else {
            out.push_str(&rest[..open + inner_start]);
            rest = &rest[open + inner_start..];
            continue;
        };
        out.push_str(&rest[..open]);
        let closer = after[close..].chars().next().map_or(1, char::len_utf8);
        rest = &after[close + closer..];
    }
    out.push_str(rest);
    normalized(&out)
}

/// A full-width form's ASCII letter, digit or mark, lowercased; any
/// other character as it is.
fn ascii_form(c: char) -> char {
    match c {
        '！'..='～' => char::from_u32(c as u32 - 0xFEE0).map_or(c, |a| a.to_ascii_lowercase()),
        _ => c,
    }
}

/// Where `named` ends at the start of `text`: the shortest prefix whose
/// words are `named`, ending where its last word does — before a mark,
/// a space, a 第, or a digit glued to a name that does not itself end
/// in one ("ショー２", not "Show 2" read inside "Show 20").
fn name_end(text: &str, named: &[String]) -> Option<usize> {
    let ends_in_digit = named
        .last()
        .and_then(|w| w.chars().last())
        .is_some_and(|c| c.is_ascii_digit());
    text.char_indices()
        .map(|(i, c)| i + c.len_utf8())
        .filter(|&end| words(&text[..end]) == named)
        .find(|&end| {
            text[end..].chars().next().is_none_or(|c| {
                let digit = half_width(c).is_ascii_digit();
                !c.is_alphanumeric() || c == '第' || (digit && !ends_in_digit)
            })
        })
}

/// The marks written right after the name ending at `end`, up to the
/// next letter or digit, without spaces and [`SEPARATORS`]: "'" for
/// "Gintama'", "!!" for "Haikyuu!! 2nd Season", nothing for "Show:
/// Season 1", "ショー：第1期" or "[Oshi no Ko] 2nd Season".
fn marks_after(text: &str, end: usize) -> String {
    text[end..]
        .chars()
        .take_while(|c| !c.is_alphanumeric())
        .filter(|c| !c.is_whitespace() && !SEPARATORS.contains(*c))
        .collect()
}

/// The marks that separate a name from what follows it rather than
/// belong to it, full-width forms and a closing bracket around the
/// name included.
const SEPARATORS: &str = ":-–—~,.・、：－～，．]）)】」』";

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
