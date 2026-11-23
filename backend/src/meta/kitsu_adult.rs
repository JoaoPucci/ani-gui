//! Kitsu's own line between the entries it serves a reader who is not
//! signed in and the ones it hides.
//!
//! Kitsu marks an entry `nsfw` from its rating alone, by a whitelist:
//! rated G, PG or R, or not rated at all, it is safe; anything else is
//! not, and R18 is the only other rating on its scale (kitsu-server's
//! `AgeRatings` concern, `sfw?` and `nsfw?`). Read without a login,
//! Kitsu applies that same list: search and browse never return an
//! `nsfw` entry, `filter[ageRating]=R18` answers an empty list, and
//! `/anime/<id>` answers 404. Its mappings lookup by MAL or AniList id
//! still inlines the entry (MAL 40750 gives Redo of Healer, Kitsu
//! 42732, `nsfw: true`, `ageRating: "R18"`), so a row built on that
//! lookup would list a show the app cannot open.
//!
//! The rule reads the rating every [`KitsuAnimeRef`] already carries,
//! rather than the `nsfw` attribute beside it: the two say the same
//! thing, and the rating is also on every ref a cache row kept before
//! anything read the flag.

use super::KitsuAnimeRef;

/// The ratings Kitsu counts as safe for work.
const SAFE_AGE_RATINGS: [&str; 3] = ["G", "PG", "R"];

/// Whether Kitsu marks an entry rated `age_rating` `nsfw`, and so hides
/// it from a reader who is not signed in.
#[must_use]
pub fn is_nsfw(age_rating: Option<&str>) -> bool {
    age_rating.is_some_and(|r| !SAFE_AGE_RATINGS.contains(&r))
}

/// `refs` less every entry Kitsu marks `nsfw`, the rest in order.
#[must_use]
pub fn drop_nsfw(refs: Vec<KitsuAnimeRef>) -> Vec<KitsuAnimeRef> {
    refs.into_iter()
        .filter(|r| !is_nsfw(r.age_rating.as_deref()))
        .collect()
}
