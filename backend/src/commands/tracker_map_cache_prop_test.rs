//! Property coverage for the episode map's cache row: the key it is
//! stored under, and the window it is kept for.

use super::{map_key, map_ttl};
use crate::account::episode_map::{EpisodeMap, Identity, Segment, Shape};
use crate::account::provider::ProviderKind;
use crate::cache::ttl::{TRACKER_MAP_TTL, TRACKER_MAP_UNSETTLED_TTL};
use proptest::prelude::*;

fn any_kind() -> impl Strategy<Value = ProviderKind> {
    prop_oneof![
        Just(ProviderKind::AniList),
        Just(ProviderKind::MyAnimeList),
        Just(ProviderKind::InHouse),
    ]
}

/// What a key is built from, over ranges small enough that two draws
/// often share a part: the tracker, a Kitsu id as Kitsu sends one, and
/// the entry the show resolved to with its total, or nothing.
fn key_parts() -> impl Strategy<Value = (ProviderKind, String, Option<(u32, Option<u32>)>)> {
    (
        any_kind(),
        prop_oneof!["[0-9]{1,2}", "[0-9]{1,9}"],
        proptest::option::of((
            prop_oneof![0u32..4, any::<u32>()],
            proptest::option::of(prop_oneof![0u32..4, any::<u32>()]),
        )),
    )
}

fn any_shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        Just(Shape::Identity(Identity::CountsAgree)),
        Just(Shape::Identity(Identity::CountUnknown)),
        Just(Shape::Identity(Identity::Unexplained)),
        Just(Shape::Identity(Identity::Conflict)),
        Just(Shape::Chain),
        Just(Shape::SharedEntry),
        Just(Shape::SplitParts),
        Just(Shape::Extras),
    ]
}

fn segment(entry: u32, finished: bool) -> Segment {
    Segment {
        entry,
        entry_total: 12,
        entry_finished: finished,
        kitsu_from: 1,
        kitsu_to: 12,
        lead: 0,
        extras_after: Vec::new(),
    }
}

/// A map of any shape over zero to four segments, each finished or
/// still airing.
fn any_map() -> impl Strategy<Value = EpisodeMap> {
    (any_shape(), prop::collection::vec(any::<bool>(), 0..5)).prop_map(|(shape, finished)| {
        EpisodeMap {
            shape,
            segments: finished
                .iter()
                .enumerate()
                .map(|(i, f)| segment(u32::try_from(i).unwrap_or(0), *f))
                .collect(),
        }
    })
}

proptest! {
    /// Two shows, trackers or resolutions never share a row: the key
    /// is the same exactly when everything it is built from is, and it
    /// opens on the version and the tracker's slug. A collision would
    /// serve one show's map for another's episodes.
    #[test]
    fn a_key_is_distinct_for_distinct_parts_and_names_its_tracker(
        a in key_parts(), b in key_parts(), same in any::<bool>()
    ) {
        let b = if same { a.clone() } else { b };
        let ka = map_key(a.0, &a.1, a.2);
        let prefix = format!("tracker-map:v2:{}:", a.0.slug());
        prop_assert!(ka.starts_with(&prefix));
        prop_assert_eq!(ka == map_key(b.0, &b.1, b.2), a == b);
    }

    /// The long window is for a conclusive map whose every entry has
    /// finished airing; anything else is kept for the short one.
    #[test]
    fn the_long_window_is_for_a_settled_conclusive_map_alone(map in any_map()) {
        let settled = map.conclusive() && map.segments.iter().all(|s| s.entry_finished);
        let want = if settled { TRACKER_MAP_TTL } else { TRACKER_MAP_UNSETTLED_TTL };
        prop_assert_eq!(map_ttl(&map), want);
    }

    /// An entry that goes back to airing never keeps the map longer:
    /// flipping any one segment to unfinished leaves the window the
    /// same or shortens it.
    #[test]
    fn a_segment_still_airing_never_lengthens_the_window(map in any_map(), pick: prop::sample::Index) {
        if map.segments.is_empty() {
            return Ok(());
        }
        let mut airing = map.clone();
        airing.segments[pick.index(map.segments.len())].entry_finished = false;
        prop_assert!(map_ttl(&airing) <= map_ttl(&map));
    }
}
