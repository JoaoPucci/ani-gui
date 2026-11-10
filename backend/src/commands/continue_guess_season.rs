//! The season rules of a Continue row's guess: a later season the
//! title names, and a later part of a season the title names whole.

use super::words::{names, words};
use crate::meta::kitsu::KitsuAnimeRef;

/// Whether `hit` names cour `cour`: a title ending in it, a slug
/// ending in it or in the bare number, or a title that is `title`.
pub(super) fn names_cour(title: &str, cour: u32, hit: &KitsuAnimeRef) -> bool {
    crate::commands::cour_same_name::names_the_entry(title, names(hit))
        || names(hit).any(|t| crate::commands::cour::cour_from_title(t) == Some(cour))
        || hit.slug.as_deref().is_some_and(|slug| {
            crate::commands::cour::cour_from_slug(slug) == Some(cour)
                || slug
                    .rsplit_once('-')
                    .is_some_and(|(_, tail)| tail == cour.to_string())
        })
}

/// Whether `t` ends in a part after the first of a season: "… Season
/// Part 2", "… 2nd Season Part 3", "… Season 2 Part 2".
pub(super) fn later_part_of_a_season(t: &str) -> bool {
    let w = words(t);
    let n = w.len();
    n >= 3
        && w[n - 2] == "part"
        && w[n - 1].parse::<u32>().is_ok_and(|p| p >= 2)
        && w[..n - 2].iter().any(|x| x == "season")
}

/// Whether `title` names a season and no part of one.
pub(super) fn names_a_whole_season(title: &str) -> bool {
    let w = words(title);
    w.iter().any(|x| x == "season") && !w.iter().any(|x| x == "part")
}

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
