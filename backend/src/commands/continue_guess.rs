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

/// A row's title words: runs of ASCII letters and digits, lowercase,
/// the articles left out.
fn words(title: &str) -> Vec<String> {
    title
        .to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
        .filter(|w| !w.is_empty() && !matches!(*w, "the" | "a" | "an"))
        .map(str::to_owned)
        .collect()
}

/// Every title a hit carries: canonical, localized, abbreviated.
fn names(hit: &KitsuAnimeRef) -> impl Iterator<Item = &str> {
    std::iter::once(hit.canonical_title.as_str())
        .chain(hit.titles.values().map(String::as_str))
        .chain(hit.abbreviated_titles.iter().map(String::as_str))
}

/// Whether a row at `episode` is past what `count` episodes hold under
/// the provider's numbering: past twice the count and two more. A hit
/// with no count is never past.
#[must_use]
pub fn past_reach(episode: f64, count: Option<u32>) -> bool {
    count.is_some_and(|n| episode > f64::from(n) * 2.0 + 2.0)
}

/// Whether a title names a series: it carries "TV" as a word.
#[must_use]
pub fn names_series(title: &str) -> bool {
    words(title).iter().any(|w| w == "tv")
}

/// Whether a Kitsu `subtype` is a series: TV, or a web series (ONA).
#[must_use]
pub fn is_series(subtype: Option<&str>) -> bool {
    subtype.is_some_and(|s| s.eq_ignore_ascii_case("tv") || s.eq_ignore_ascii_case("ona"))
}

/// Whether one of `hit`'s titles, or its slug's words, carries the
/// first word of `title`. A title with no words is carried by every
/// hit.
#[must_use]
pub fn carries_first_word(title: &str, hit: &KitsuAnimeRef) -> bool {
    let Some(lead) = words(title).into_iter().next() else {
        return true;
    };
    let slug = hit.slug.as_deref().map(|s| s.replace('-', " "));
    let carried = names(hit)
        .chain(slug.as_deref())
        .any(|t| words(t).contains(&lead));
    carried
}

/// Whether `hit` names cour `cour`: a title ending in it, a slug
/// ending in it or in the bare number, or a title that is `title`.
fn names_cour(title: &str, cour: u32, hit: &KitsuAnimeRef) -> bool {
    super::cour_same_name::names_the_entry(title, names(hit))
        || names(hit).any(|t| super::cour::cour_from_title(t) == Some(cour))
        || hit.slug.as_deref().is_some_and(|slug| {
            super::cour::cour_from_slug(slug) == Some(cour)
                || slug
                    .rsplit_once('-')
                    .is_some_and(|(_, tail)| tail == cour.to_string())
        })
}

/// Whether `t` ends in a part after the first of a season: "… Season
/// Part 2", "… 2nd Season Part 3", "… Season 2 Part 2".
fn later_part_of_a_season(t: &str) -> bool {
    let w = words(t);
    let n = w.len();
    n >= 3
        && w[n - 2] == "part"
        && w[n - 1].parse::<u32>().is_ok_and(|p| p >= 2)
        && w[..n - 2].iter().any(|x| x == "season")
}

/// Whether `title` names a season and no part of one.
fn names_a_whole_season(title: &str) -> bool {
    let w = words(title);
    w.iter().any(|x| x == "season") && !w.iter().any(|x| x == "part")
}

/// Whether the row — its search title and the episode it reached —
/// rules `hit` out.
#[must_use]
pub fn ruled_out(title: &str, episode: f64, hit: &KitsuAnimeRef) -> bool {
    let _ = (title, episode, hit);
    false
}

/// The ids of the `hits` the row rules out, in their order.
#[must_use]
pub fn ruled_out_ids(title: &str, episode: f64, hits: &[KitsuAnimeRef]) -> Vec<String> {
    hits.iter()
        .filter(|h| ruled_out(title, episode, h))
        .map(|h| h.id.clone())
        .collect()
}

/// A hit as the renderer sends it: the fields the rules read, any of
/// them but the id absent on an answer cached before Kitsu's titles
/// were kept, and then read as carrying none.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SentHit {
    /// The Kitsu id.
    pub id: String,
    /// The canonical title.
    #[serde(default)]
    pub canonical_title: String,
    /// The localized titles.
    #[serde(default)]
    pub titles: std::collections::HashMap<String, String>,
    /// The abbreviations.
    #[serde(default)]
    pub abbreviated_titles: Vec<String>,
    /// The slug.
    #[serde(default)]
    pub slug: Option<String>,
    /// The episode count.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// The subtype.
    #[serde(default)]
    pub subtype: Option<String>,
}

impl From<SentHit> for KitsuAnimeRef {
    fn from(h: SentHit) -> Self {
        Self {
            id: h.id,
            canonical_title: h.canonical_title,
            titles: h.titles,
            abbreviated_titles: h.abbreviated_titles,
            slug: h.slug,
            synopsis: None,
            start_date: None,
            end_date: None,
            episode_count: h.episode_count,
            average_rating: None,
            subtype: h.subtype,
            status: None,
            age_rating: None,
            popularity_rank: None,
            poster_image: None,
            cover_image: None,
            anilist_banner: None,
        }
    }
}

/// What the renderer asks: the row's search title, the episode it
/// reached, and the hits its search or slug lookup returned.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RuledOutRequest {
    /// The row's search title.
    pub title: String,
    /// The episode the row reached, in the provider's numbering.
    pub episode: f64,
    /// The hits.
    pub hits: Vec<SentHit>,
}

/// The ids of the request's hits its row rules out.
#[must_use]
pub fn answer(req: RuledOutRequest) -> Vec<String> {
    let hits: Vec<KitsuAnimeRef> = req.hits.into_iter().map(Into::into).collect();
    ruled_out_ids(&req.title, req.episode, &hits)
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
