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
//! something the row carries — the episode it reached and its title —
//! so the guess makes no request it did not make before. A hit is out
//! when:
//!
//! - **the row's episode is past it**: past twice the hit's episode
//!   count and two more. The provider sometimes lists two of Kitsu's
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
//!   later, commoner words;
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
//!   Part 2".
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

#[path = "continue_guess_request.rs"]
mod request;
pub use request::{answer, RuledOutRequest, SentHit};

/// Whether the row — its search title and the episode it reached —
/// rules `hit` out.
#[must_use]
pub fn ruled_out(title: &str, episode: f64, hit: &KitsuAnimeRef) -> bool {
    if past_reach(episode, hit.episode_count) {
        return true;
    }
    if names_series(title) && !is_series(hit.subtype.as_deref()) {
        return true;
    }
    if !carries_first_word(title, hit) {
        return true;
    }
    if let Some(cour) = super::cour::cour_from_title(title).filter(|c| *c >= 2) {
        if !season::names_cour(title, cour, hit) {
            return true;
        }
    }
    season::names_a_whole_season(title) && words::names(hit).any(season::later_part_of_a_season)
}

/// The ids of the `hits` the row rules out, in their order.
#[must_use]
pub fn ruled_out_ids(title: &str, episode: f64, hits: &[KitsuAnimeRef]) -> Vec<String> {
    hits.iter()
        .filter(|h| ruled_out(title, episode, h))
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
