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
