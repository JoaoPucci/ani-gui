//! The reverse-mapping key reader.

use super::*;
use proptest::prelude::*;

#[test]
fn a_versioned_mapping_names_its_show_key() {
    assert_eq!(
        mapping_key_id("allmanga2kitsu:v8:hianime:naruto-600"),
        Some("hianime:naruto-600")
    );
    assert_eq!(
        mapping_key_id("allmanga2kitsu:v3:naruto-3687"),
        Some("naruto-3687")
    );
}

#[test]
fn a_played_mark_names_its_show_key() {
    assert_eq!(
        mapping_key_id("allmanga2kitsu:played:v1:hianime:x-1"),
        Some("hianime:x-1")
    );
}

#[test]
fn other_shapes_name_nothing() {
    for key in [
        "allmanga2kitsu:",
        "allmanga2kitsu:v:x",
        "allmanga2kitsu:vx:x",
        "allmanga2kitsu:played:x",
        "watched-at:v1:x",
    ] {
        assert_eq!(mapping_key_id(key), None, "{key}");
    }
}

#[test]
fn the_end_of_a_family_follows_every_key_in_it() {
    assert_eq!(past("play:"), "play;");
    assert!("play:\u{10FFFF}" < past("play:").as_str());
    assert!("playx" > past("play:").as_str());
}

proptest! {
    #[test]
    fn a_mapping_key_reads_back_the_key_it_was_written_for(
        version in 1u32..1000,
        id in "[a-z0-9:-]{1,40}",
        played in any::<bool>(),
    ) {
        let key = if played {
            format!("allmanga2kitsu:played:v1:{id}")
        } else {
            format!("allmanga2kitsu:v{version}:{id}")
        };
        prop_assert_eq!(mapping_key_id(&key), Some(id.as_str()));
    }

    #[test]
    fn every_key_under_a_family_sorts_before_its_end(
        family in "[a-z-]{1,12}:",
        rest in "\\PC{0,20}",
    ) {
        let key = format!("{family}{rest}");
        let end = past(&family);
        prop_assert!(family.as_str() <= key.as_str() && key.as_str() < end.as_str());
    }
}
