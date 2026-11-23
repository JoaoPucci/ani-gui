//! The season a Continue row's show key names, for a row whose title
//! names none: hianime's "Jujutsu Kaisen: Specials" is
//! `jujutsu-kaisen-season-2-recaps`.

use super::season::names_cour;
use super::words::{names, words};
use crate::meta::kitsu::KitsuAnimeRef;

/// The season `text` names anywhere in its words: "Season 2", or an
/// ordinal before the word ("2nd Season"). hianime's key for its
/// "Jujutsu Kaisen: Specials" is `jujutsu-kaisen-season-2-recaps`.
#[must_use]
pub fn season_named(text: &str) -> Option<u32> {
    let w = words(text);
    w.windows(2).find_map(|p| {
        if p[0] == "season" {
            return p[1].parse().ok();
        }
        if p[1] != "season" {
            return None;
        }
        let digits = p[0].trim_end_matches(|c: char| c.is_ascii_alphabetic());
        let suffix = &p[0][digits.len()..];
        matches!(suffix, "st" | "nd" | "rd" | "th")
            .then(|| digits.parse().ok())
            .flatten()
    })
}

/// Whether the row's show key names a later season its title does not
/// and `hit` names no such season: the title names no season at all,
/// the key's words name one past the first, and none of the hit's
/// titles nor its slug names that one, anywhere or as the title's
/// season rule reads it ([`names_cour`]).
pub(super) fn a_season_the_key_names_and_the_hit_does_not(
    title: &str,
    key_words: &str,
    hit: &KitsuAnimeRef,
) -> bool {
    if crate::commands::cour::cour_from_title(title).is_some() || season_named(title).is_some() {
        return false;
    }
    let Some(season) = season_named(key_words).filter(|s| *s >= 2) else {
        return false;
    };
    let slug = hit.slug.as_deref().map(|s| s.replace('-', " "));
    let named = names(hit)
        .chain(slug.as_deref())
        .any(|t| season_named(t) == Some(season));
    !named && !names_cour(title, season, hit)
}
