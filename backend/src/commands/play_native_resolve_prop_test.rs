//! Property coverage for the track cap the resolve applies once,
//! where it is built.

use super::tracks_within_cap;
use crate::proxy::upstream::SUBTITLE_TRACK_CAP;
use crate::scraper::provider::SubtitleTrack;
use proptest::prelude::*;

fn listing() -> impl Strategy<Value = Vec<SubtitleTrack>> {
    (0usize..SUBTITLE_TRACK_CAP * 3).prop_flat_map(|n| {
        proptest::option::of(0..n.max(1)).prop_map(move |default_at| {
            (0..n)
                .map(|i| SubtitleTrack {
                    lang: format!("l{i}"),
                    label: format!("Track {i}"),
                    default: default_at == Some(i),
                    url: format!("https://cdn.example/x/subs/{i}.vtt"),
                })
                .collect()
        })
    })
}

proptest! {
    /// The kept list is the listing itself when it fits, and its first
    /// cap-many in order otherwise — except that a default past the cap
    /// takes the last kept slot, so the provider's default is never
    /// dropped and never more than one track moves.
    #[test]
    fn the_first_cap_many_are_kept_in_order_with_the_default_among_them(tracks in listing()) {
        let kept = tracks_within_cap(tracks.clone());
        prop_assert_eq!(kept.len(), tracks.len().min(SUBTITLE_TRACK_CAP));
        let default_at = tracks.iter().position(|t| t.default);
        let expected: Vec<&str> = match default_at {
            Some(d) if tracks.len() > SUBTITLE_TRACK_CAP && d >= SUBTITLE_TRACK_CAP => tracks[..SUBTITLE_TRACK_CAP - 1]
                .iter()
                .chain(std::iter::once(&tracks[d]))
                .map(|t| t.lang.as_str())
                .collect(),
            _ => tracks.iter().take(SUBTITLE_TRACK_CAP).map(|t| t.lang.as_str()).collect(),
        };
        let got: Vec<&str> = kept.iter().map(|t| t.lang.as_str()).collect();
        prop_assert_eq!(got, expected);
        prop_assert_eq!(kept.iter().filter(|t| t.default).count(), usize::from(default_at.is_some()));
    }
}
