//! The kind rules of a Continue row's guess: the movie a title names,
//! and the recap a row does not.

use super::words::{names, words};
use crate::meta::kitsu::KitsuAnimeRef;

/// Whether a title names a movie: it carries "movie" as a word.
#[must_use]
pub fn names_movie(title: &str) -> bool {
    words(title).iter().any(|w| w == "movie")
}

/// Whether a Kitsu `subtype` is a movie.
#[must_use]
pub fn is_movie(subtype: Option<&str>) -> bool {
    subtype.is_some_and(|s| s.eq_ignore_ascii_case("movie"))
}

/// The number a title gives its movie: the number right after "movie"
/// ("One Piece: The Movie 14 - Stampede" is 14, "Haikyuu!! Movie 4:
/// Concept no Tatakai" 4), or none.
#[must_use]
pub fn movie_number(title: &str) -> Option<u32> {
    let w = words(title);
    w.windows(2)
        .find(|p| p[0] == "movie")
        .and_then(|p| p[1].parse().ok())
}

/// Whether `hit` is numbered as another movie than the one `title`
/// numbers: the title numbers its movie, and the hit's titles number
/// one or more movies, none of them that one. A hit that numbers no
/// movie is not another: Kitsu titles One Piece's fourteenth by its
/// name alone ("One Piece: Stampede").
#[must_use]
pub fn numbered_as_another_movie(title: &str, hit: &KitsuAnimeRef) -> bool {
    let Some(n) = movie_number(title) else {
        return false;
    };
    let mut numbered = names(hit).filter_map(movie_number).peekable();
    numbered.peek().is_some() && numbered.all(|m| m != n)
}

/// Whether a title names a recap: it carries "recap" or "recaps" as a
/// word.
#[must_use]
pub fn names_recap(title: &str) -> bool {
    words(title).iter().any(|w| w == "recap" || w == "recaps")
}

/// Whether `hit` is a recap the row does not name: one of its titles
/// names a recap, and neither the row's title nor its show key's words
/// do. hianime's "Haikyu!! Movie: Battle of the Garbage Dump" brings
/// back the fourth recap movie alone, its "Jujutsu Kaisen 0 Movie" the
/// second season's recap.
#[must_use]
pub fn a_recap_the_row_does_not_name(title: &str, key_words: &str, hit: &KitsuAnimeRef) -> bool {
    !names_recap(title) && !names_recap(key_words) && names(hit).any(names_recap)
}

/// Whether the title names a movie and `hit` is another: not a movie,
/// or numbered as another movie.
pub(super) fn another_movie(title: &str, hit: &KitsuAnimeRef) -> bool {
    names_movie(title)
        && (!is_movie(hit.subtype.as_deref()) || numbered_as_another_movie(title, hit))
}
