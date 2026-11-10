//! The kind rules of a Continue row's guess: the movie a title names,
//! and the recap a row does not.

use crate::meta::kitsu::KitsuAnimeRef;

/// Whether a title names a movie: it carries "movie" as a word.
#[must_use]
pub fn names_movie(_title: &str) -> bool {
    false
}

/// Whether a Kitsu `subtype` is a movie.
#[must_use]
pub fn is_movie(_subtype: Option<&str>) -> bool {
    false
}

/// The number a title gives its movie: the number right after "movie".
#[must_use]
pub fn movie_number(_title: &str) -> Option<u32> {
    None
}

/// Whether `hit` is numbered as another movie than the one `title`
/// numbers.
#[must_use]
pub fn numbered_as_another_movie(_title: &str, _hit: &KitsuAnimeRef) -> bool {
    false
}

/// Whether a title names a recap.
#[must_use]
pub fn names_recap(_title: &str) -> bool {
    false
}

/// Whether `hit` is a recap the row does not name.
#[must_use]
pub fn a_recap_the_row_does_not_name(_title: &str, _key_words: &str, _hit: &KitsuAnimeRef) -> bool {
    false
}

/// Whether the title names a movie and `hit` is another.
pub(super) fn another_movie(_title: &str, _hit: &KitsuAnimeRef) -> bool {
    false
}
