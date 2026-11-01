//! Which of a listing's subtitle tracks are kept, and in what order,
//! wherever only some of them can be: the listing past the accept
//! cap, the hand-off's command line, the one track a player that
//! takes a single file is given, and the tracks a cached row's check
//! reads.

use crate::scraper::provider::SubtitleTrack;

/// The listing's indices, most wanted first: the provider's default,
/// then the rest in the listing's order.
#[must_use]
pub fn preference_order(tracks: &[SubtitleTrack], _locale: &str) -> Vec<usize> {
    let mut order: Vec<usize> = (0..tracks.len()).collect();
    order.sort_by_key(|&i| (!tracks[i].default, i));
    order
}

/// The tracks in [`preference_order`].
#[must_use]
pub fn preferred_first<'a>(tracks: &'a [SubtitleTrack], locale: &str) -> Vec<&'a SubtitleTrack> {
    preference_order(tracks, locale)
        .into_iter()
        .map(|i| &tracks[i])
        .collect()
}

/// At most `cap` of the listing: all of it when it fits, otherwise the
/// first `cap` in listing order — a default the listing put past them
/// taking the last kept slot — beside how many were left behind.
#[must_use]
pub fn most_wanted_within(
    mut tracks: Vec<SubtitleTrack>,
    cap: usize,
    _locale: &str,
) -> (Vec<SubtitleTrack>, usize) {
    if tracks.len() <= cap {
        return (tracks, 0);
    }
    let dropped = tracks.len() - cap;
    if cap > 0 {
        if let Some(default_at) = tracks.iter().position(|t| t.default) {
            if default_at >= cap {
                let default = tracks.swap_remove(default_at);
                tracks.truncate(cap - 1);
                tracks.push(default);
                return (tracks, dropped);
            }
        }
    }
    tracks.truncate(cap);
    (tracks, dropped)
}

#[cfg(test)]
#[path = "track_preference_test.rs"]
mod tests;
