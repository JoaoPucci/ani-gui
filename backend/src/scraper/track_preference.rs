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
use tag::parse_tag;

#[path = "track_preference_rank.rs"]
mod rank;
use rank::rank;

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

/// Whether the listing has a track the built-in player would start on:
/// one in the remembered or the locale's language, or the provider's
/// default. Without one it starts with none, and so does a hand-off.
#[must_use]
pub fn has_start(tracks: &[SubtitleTrack], wanted: Wanted<'_>) -> bool {
    let remembered = wanted.remembered.and_then(parse_tag);
    let user = parse_tag(wanted.locale);
    tracks
        .iter()
        .any(|t| rank(t, remembered.as_ref(), user.as_ref()) <= rank::DEFAULT)
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
