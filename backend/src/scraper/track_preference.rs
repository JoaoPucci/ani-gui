//! Which of a listing's subtitle tracks the user most likely wants,
//! for every place that can take only some of them: the listing past
//! the accept cap, the hand-off past its command-line budget, the one
//! track a player that takes a single file is given, and the tracks a
//! cached row's check reads.
//!
//! The order is the provider's default first, then the tracks in the
//! user's language — those for the user's region ahead of the rest —
//! then English, then everything else, a track whose language is
//! unknown last; ties keep the listing's order. A provider lists its
//! tracks in its own order (megaplay's is alphabetical by label), so a
//! cut by listing order alone keeps whatever sorts first.

use crate::scraper::provider::SubtitleTrack;

/// What the viewer wants of a listing: the choice remembered for the
/// show — a BCP 47 tag; `off` is a choice about showing, not about
/// order, and is left out — and the UI locale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wanted<'a> {
    /// The language last picked on the show, if one was.
    pub remembered: Option<&'a str>,
    /// The UI locale (a BCP 47 tag, the app's setting).
    pub locale: &'a str,
}

impl<'a> Wanted<'a> {
    /// The UI locale alone, nothing remembered.
    #[must_use]
    pub fn locale(locale: &'a str) -> Self {
        Self {
            remembered: None,
            locale,
        }
    }
}

#[path = "track_preference_tag.rs"]
mod tag;
use tag::{implied_script, parse_tag, Tag};

/// The language of a track: its `lang` read as a tag, or — for a
/// provider whose `lang` is a name rather than a tag — the tag that
/// name or the label reads as.
fn track_tag(track: &SubtitleTrack) -> Option<Tag> {
    parse_tag(&track.lang).or_else(|| {
        [track.lang.as_str(), track.label.as_str()]
            .into_iter()
            .find_map(|text| crate::scraper::subtitle_language::track_tag("", text))
            .and_then(|tag| parse_tag(&tag))
    })
}

/// Whether a track in the user's language is for the user's region:
/// the same region, or the script that region writes in.
fn in_users_region(track: &Tag, user: &Tag) -> bool {
    let Some(region) = user.region.as_deref() else {
        return false;
    };
    track.region.as_deref() == Some(region)
        || implied_script(region).is_some_and(|s| track.script.as_deref() == Some(s))
}

/// Where a track stands for `user`: 0 the provider's default, 1 the
/// user's language for the user's region, 2 the user's language, 3
/// English, 4 any other known language, 5 an unknown one.
fn rank(track: &SubtitleTrack, user: Option<&Tag>) -> u8 {
    if track.default {
        return 0;
    }
    let Some(tag) = track_tag(track) else {
        return 5;
    };
    if let Some(user) = user.filter(|u| u.language == tag.language) {
        return if in_users_region(&tag, user) { 1 } else { 2 };
    }
    if tag.language == "en" {
        return 3;
    }
    4
}

/// The listing's indices, most wanted first, for a user in `locale`
/// (a BCP 47 tag, the app's own setting).
#[must_use]
pub fn preference_order(tracks: &[SubtitleTrack], wanted: Wanted<'_>) -> Vec<usize> {
    let user = parse_tag(wanted.locale);
    let mut order: Vec<usize> = (0..tracks.len()).collect();
    order.sort_by_key(|&i| (rank(&tracks[i], user.as_ref()), i));
    order
}

/// The tracks in [`preference_order`].
#[must_use]
pub fn preferred_first<'a>(
    tracks: &'a [SubtitleTrack],
    wanted: Wanted<'_>,
) -> Vec<&'a SubtitleTrack> {
    preference_order(tracks, wanted)
        .into_iter()
        .map(|i| &tracks[i])
        .collect()
}

/// At most `cap` of the listing: all of it when it fits, otherwise the
/// `cap` most wanted ([`preference_order`]), kept in the listing's
/// order so the player lists them as the provider does; beside how
/// many were left behind.
#[must_use]
pub fn most_wanted_within(
    tracks: Vec<SubtitleTrack>,
    cap: usize,
    wanted: Wanted<'_>,
) -> (Vec<SubtitleTrack>, usize) {
    if tracks.len() <= cap {
        return (tracks, 0);
    }
    let dropped = tracks.len() - cap;
    let mut keep = vec![false; tracks.len()];
    for i in preference_order(&tracks, wanted).into_iter().take(cap) {
        keep[i] = true;
    }
    let kept = tracks
        .into_iter()
        .zip(keep)
        .filter_map(|(t, k)| k.then_some(t))
        .collect();
    (kept, dropped)
}

#[cfg(test)]
#[path = "track_preference_test.rs"]
mod tests;
