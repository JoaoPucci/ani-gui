//! Property coverage for the window a cached mapping is kept, beside
//! the pure `external_ids_ttl` in `external_ids.rs`.

use super::external_ids_ttl;
use crate::cache::ttl::{EXTERNAL_IDS_PARTIAL_TTL, EXTERNAL_IDS_TTL};
use crate::meta::kitsu::ExternalIds;

proptest::proptest! {
    /// A mapping is kept for the long window exactly when both ids
    /// are present. What is present does not move; what is missing may
    /// appear, and must be asked for again soon.
    #[test]
    fn the_long_window_is_for_a_complete_mapping_alone(
        mal in proptest::option::of(1u32..),
        anilist in proptest::option::of(1u32..),
    ) {
        let ids = ExternalIds { mal, anilist };
        let complete = mal.is_some() && anilist.is_some();
        let want = if complete { EXTERNAL_IDS_TTL } else { EXTERNAL_IDS_PARTIAL_TTL };
        proptest::prop_assert_eq!(external_ids_ttl(&ids), want);
        proptest::prop_assert!(external_ids_ttl(&ids) >= EXTERNAL_IDS_PARTIAL_TTL);
    }
}
