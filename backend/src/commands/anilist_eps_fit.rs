//! Whether an AniList streaming listing describes the show whose
//! episode page it would fill.
//!
//! AniList attaches Crunchyroll listings to the wrong entry often
//! enough to matter: every Gintama TV entry carries the 2006 series'
//! first 49 episodes, sequels carry their first season's listing, and
//! split cours carry the whole season. Filling stills from such a
//! listing puts another show's episodes on the page, so the backfill
//! asks this rule first. `docs/title-resolution.md` carries the case
//! table the rule was chosen from.

use crate::meta::anilist_streaming_eps::StreamingListing;
use crate::meta::kitsu::KitsuEpisode;

use super::titles::{is_placeholder, squashed, titles_agree};

/// Fewest compared titles that decide on their own. Below it a few
/// differently worded titles could refuse a right listing, so the
/// episode count decides instead.
const MIN_COMPARED: usize = 3;

/// `true` when `listing` may fill stills on `page`.
///
/// Where Kitsu titles at least [`MIN_COMPARED`] of the page's
/// episodes the listing also titles, the titles decide: the listing
/// fits when at least half of them agree ([`titles_agree`]), so a
/// minority worded differently never refuses it. Otherwise the shape
/// decides: a finished show refuses a listing numbered past its
/// episode count.
#[must_use]
pub fn listing_fits(listing: &StreamingListing, page: &[KitsuEpisode]) -> bool {
    let pairs = compared_titles(listing, page);
    if pairs.len() >= MIN_COMPARED {
        let agreeing = titles_agree(&pairs);
        return agreeing * 2 >= pairs.len();
    }
    shape_fits(listing)
}

/// A finished show with a known count refuses a listing whose highest
/// episode number is past it. An ongoing show, or one AniList has no
/// count for, keeps any listing.
fn shape_fits(listing: &StreamingListing) -> bool {
    let Some(count) = listing.episodes.filter(|_| listing.finished) else {
        return true;
    };
    listing
        .entries
        .last_key_value()
        .is_none_or(|(highest, _)| *highest <= count)
}

/// `(Kitsu title, listing title)` for every page episode both title.
/// A Kitsu placeholder ("Episode 5") is no title.
fn compared_titles<'a>(
    listing: &'a StreamingListing,
    page: &'a [KitsuEpisode],
) -> Vec<(&'a str, &'a str)> {
    page.iter()
        .filter_map(|ep| {
            let ours = ep.canonical_title.as_deref()?;
            let theirs = listing.entries.get(&ep.number?)?.title.as_str();
            (!is_placeholder(ours) && !squashed(theirs).is_empty()).then_some((ours, theirs))
        })
        .collect()
}

#[cfg(test)]
#[path = "anilist_eps_fit_test.rs"]
mod tests;
