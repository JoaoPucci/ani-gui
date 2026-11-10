//! The search hits a Continue row's guess may not take.
//!
//! A history row that recorded no Kitsu id is matched back to Kitsu by
//! a search for its title, and the renderer picks among the hits
//! (`pickKitsuMatch` in `frontend/src/lib/history/resolve.ts`). Kitsu's
//! search answers with its closest entries whatever they are, and for a
//! title it reads poorly the show is often not among them: hianime's
//! "Jujutsu Kaisen (TV)" brings back the New Year's Special alone, and
//! "My Star", its name for [Oshi no Ko], nothing of the show at all.
//! The picker then took whatever was left.
//!
//! The row says more about its show than the search reads, and the
//! rules here rule hits out before the picker sees them. Each rests on
//! something the row carries — the episode it reached, its title and
//! its show key — and they ask nothing of Kitsu or the provider. A row
//! they leave with no hit makes the requests the steps after the
//! search always made for a row with none: the search after a later
//! season's slug lookup, then the search by its show key's words. A
//! hit is out when:
//!
//! - **the row's episode is past it**: past twice the hit's episode
//!   count and two more, for a row a provider the app walks wrote,
//!   which numbers each listing from its first episode (a row a retired
//!   provider wrote can speak a franchise's numbering, and is never
//!   past a hit). The provider sometimes lists two of Kitsu's
//!   cours as one show ("Attack on Titan Season 3", 22 episodes, is
//!   Kitsu's 12 and 10) and appends extras after the last episode, so
//!   only a row well past the count says the hit is another show — a
//!   row at episode 24 is not a one-episode special;
//! - **the title names a series and the hit is not one**: a title
//!   carrying "TV" as a word takes no special, movie, OVA or music
//!   entry, only a TV or web series;
//! - **none of the hit's titles carries the title's first word**: its
//!   canonical, localized and abbreviated titles and its slug's words.
//!   Kitsu answers a title it does not know with entries that share its
//!   later, commoner words. Words are runs of ASCII letters and digits,
//!   as in the shared title-words rule, so an accented letter splits
//!   one ("Ōkami-san" leads with "kami");
//! - **the title names a later season the hit does not**: a title
//!   ending in a cour ("Season 2", "2nd Season", "Part 2") takes a hit
//!   only when one of its titles ends in the same cour, its slug does
//!   (a bare trailing number included: Kitsu files the Slime's second
//!   season as `tensei-shitara-slime-datta-ken-2`), or one of its
//!   titles is the row's ([`super::cour_same_name::names_the_entry`]);
//! - **the hit is a later part of a season the title names whole**: a
//!   title naming a season and no part takes no hit titled as that
//!   season's "Part 2" or later — hianime's "That Time I Got
//!   Reincarnated as a Slime Season 2" is not Kitsu's "2nd Season
//!   Part 2";
//! - **the title names a movie the hit is not**: a title carrying
//!   "Movie" as a word takes only a movie, and one numbering its movie
//!   ("One Piece: The Movie 14 - Stampede") takes no hit whose titles
//!   number other movies and not that one. Kitsu titles the fourteenth
//!   "One Piece: Stampede", so a hit that numbers no movie is not
//!   another;
//! - **the hit is a recap the row does not name**: one of the hit's
//!   titles carries "Recap", and neither the title nor the show key's
//!   words do. hianime's "Haikyu!! Movie: Battle of the Garbage Dump"
//!   brings back the fourth recap movie alone;
//! - **the show key names a later season the title does not**: a key
//!   whose words name a season past the first anywhere in them, on a
//!   row whose title names no season, takes only a hit naming that
//!   season in a title or its slug, anywhere in it. hianime's
//!   "Jujutsu Kaisen: Specials" is `jujutsu-kaisen-season-2-recaps`,
//!   and its title's search brings back the New Year's Special, the
//!   third season and a movie.
//!
//! A hit ruled out is never offered, even when no hit is left: the row
//! then goes on to the resolve from its show key's words
//! ([`super::kitsu::resolve_allmanga_show_id`]), which searches the
//! show's name as the provider spells it in its address, rather than
//! taking a hit the row itself says is another show.

use crate::meta::kitsu::KitsuAnimeRef;

#[path = "continue_guess_words.rs"]
mod words;
pub use words::{carries_first_word, is_series, names_series, past_reach};

#[path = "continue_guess_season.rs"]
mod season;
pub use season::season_named;

#[path = "continue_guess_kind.rs"]
mod kind;
pub use kind::{
    a_recap_the_row_does_not_name, is_movie, movie_number, names_movie, names_recap,
    numbered_as_another_movie,
};

#[path = "continue_guess_request.rs"]
mod request;
pub use request::{answer, RuledOutRequest, SentHit};

/// Whether the row — its search title, and the episode it reached when
/// that says how far into a hit it got — rules `hit` out.
#[must_use]
pub fn ruled_out(title: &str, episode: Option<f64>, hit: &KitsuAnimeRef) -> bool {
    if episode.is_some_and(|e| past_reach(e, hit.episode_count)) {
        return true;
    }
    if names_series(title) && !is_series(hit.subtype.as_deref()) {
        return true;
    }
    if !carries_first_word(title, hit) || kind::another_movie(title, hit) {
        return true;
    }
    if let Some(cour) = super::cour::cour_from_title(title).filter(|c| *c >= 2) {
        if !season::names_cour(title, cour, hit) {
            return true;
        }
    }
    season::names_a_whole_season(title) && words::names(hit).any(season::later_part_of_a_season)
}

/// Whether the row's title and its show key's words (empty for a key
/// that carries none) rule `hit` out together: a recap neither names,
/// or a later season the key names and the title and hit do not.
#[must_use]
pub fn ruled_out_by_key(title: &str, key_words: &str, hit: &KitsuAnimeRef) -> bool {
    kind::a_recap_the_row_does_not_name(title, key_words, hit)
        || season::a_season_the_key_names_and_the_hit_does_not(title, key_words, hit)
}

/// The ids of the `hits` the row rules out, in their order: by its
/// title and episode ([`ruled_out`]), or by its title and its show
/// key's words ([`ruled_out_by_key`]).
#[must_use]
pub fn ruled_out_ids(
    title: &str,
    key_words: &str,
    episode: Option<f64>,
    hits: &[KitsuAnimeRef],
) -> Vec<String> {
    hits.iter()
        .filter(|h| ruled_out(title, episode, h) || ruled_out_by_key(title, key_words, h))
        .map(|h| h.id.clone())
        .collect()
}

#[cfg(test)]
#[path = "continue_guess_test.rs"]
mod tests;

#[cfg(test)]
#[path = "continue_guess_prop_test.rs"]
mod prop_tests;

#[cfg(test)]
#[path = "continue_guess_corpus_test.rs"]
mod corpus_tests;
