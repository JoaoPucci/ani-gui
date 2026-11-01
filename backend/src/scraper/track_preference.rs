//! Which of a listing's subtitle tracks the user most likely wants,
//! for every place that can take only some of them: the listing past
//! the accept cap, the hand-off past its command-line budget, the one
//! track a player that takes a single file is given, and the tracks a
//! cached row's check reads.
//!
//! The order is the one the built-in player starts by
//! (`$lib/play/caption-start.ts`), so every player starts on the same
//! track: the language last picked on the show, then the user's
//! locale's language — each its own region first, then its script,
//! then any — then the provider's default, then English, then
//! everything else, a track whose language is unknown last; ties keep
//! the listing's order. A provider lists its tracks in its own order
//! (megaplay's is alphabetical by label), so a cut by listing order
//! alone keeps whatever sorts first.

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

/// The script a tag is written in: the one it names, or the one its
/// region writes Chinese in.
fn script_of(tag: &Tag) -> Option<&str> {
    tag.script
        .as_deref()
        .or_else(|| tag.region.as_deref().and_then(implied_script))
}

/// How close a track is to the language `want` names, or `None` when
/// it is in another: 0 the same region, 1 the same script, 2 the
/// language alone.
fn closeness(track: &Tag, want: &Tag) -> Option<u8> {
    if track.language != want.language {
        return None;
    }
    if want.region.is_some() && track.region == want.region {
        return Some(0);
    }
    if script_of(want).is_some() && script_of(track) == script_of(want) {
        return Some(1);
    }
    Some(2)
}

/// Where a track stands: 0–2 the show's remembered language (its
/// region, its script, any), 3–5 the user's locale's the same way, 6
/// the provider's default, 7 English, 8 any other known language, 9 an
/// unknown one — the order the built-in player starts by
/// (`$lib/play/caption-start.ts`), so the first track is the one it
/// would show.
fn rank(track: &SubtitleTrack, remembered: Option<&Tag>, user: Option<&Tag>) -> u8 {
    let tag = track_tag(track);
    if let Some(tag) = tag.as_ref() {
        if let Some(k) = remembered.and_then(|r| closeness(tag, r)) {
            return k;
        }
        if let Some(k) = user.and_then(|u| closeness(tag, u)) {
            return 3 + k;
        }
    }
    if track.default {
        return 6;
    }
    match tag {
        None => 9,
        Some(t) if t.language == "en" => 7,
        Some(_) => 8,
    }
}

/// The listing's indices, most wanted first, for a user in `locale`
/// (a BCP 47 tag, the app's own setting).
#[must_use]
pub fn preference_order(tracks: &[SubtitleTrack], wanted: Wanted<'_>) -> Vec<usize> {
    let remembered = wanted.remembered.and_then(parse_tag);
    let user = parse_tag(wanted.locale);
    let mut order: Vec<usize> = (0..tracks.len()).collect();
    order.sort_by_key(|&i| (rank(&tracks[i], remembered.as_ref(), user.as_ref()), i));
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
