//! The words a title carries past its name outside its divisions —
//! split from `play_native_title_name` for the per-file complexity
//! bar.

use super::*;

/// The words `title` carries past its name ([`name`]) that name no
/// division: "Extra Arc" in "Show 2nd Season: Extra Arc" and "The
/// Movie" in "Show 2: The Movie", nothing in "Show 2nd Season" or
/// "Show". A word ordinal inside the name stays in the name, as
/// [`name`] reads it.
pub(crate) fn undivided_rest(title: &str) -> Vec<String> {
    let words = read_words(title);
    let start = (0..words.len())
        .find(|&i| division_at(&words, i).is_some() && !inside_the_name(&words, i))
        .unwrap_or(words.len());
    let mut rest = Vec::new();
    let mut i = start;
    while i < words.len() {
        if glued_divisions(&words[i]).is_some() {
            i += 1;
            continue;
        }
        match division_at(&words, i).filter(|_| !inside_the_name(&words, i)) {
            Some((_, taken)) => {
                i += taken + if taken == 1 { half_after(&words, i) } else { 0 };
            }
            None => {
                rest.push(words[i].clone());
                i += 1;
            }
        }
    }
    rest
}
