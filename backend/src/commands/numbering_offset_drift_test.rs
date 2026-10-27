//! Tests for `crate::commands::numbering_offset::drift`.

use super::*;
use crate::commands::numbering_offset::kitsu_ep_no;
use proptest::prelude::*;

fn row(number: u32, tag: Option<&str>) -> EpisodeRef {
    EpisodeRef {
        id: u64::from(number),
        number,
        number2: tag.map(str::to_string),
        former_tag: None,
        title: None,
    }
}

/// Seven episodes with a recap after the third: slots 1 to 3 are
/// episodes 1 to 3, slot 4 is the recap "3.5", and slots 5 to 8 are
/// episodes 4 to 7, each tagged one below its slot.
fn recap_after_three() -> Vec<EpisodeRef> {
    vec![
        row(1, None),
        row(2, None),
        row(3, None),
        row(4, Some("3.5")),
        row(5, Some("4")),
        row(6, Some("5")),
        row(7, Some("6")),
        row(8, Some("7")),
    ]
}

fn stamp(slot: u32, tag: &str) -> (u32, String) {
    (slot, tag.to_string())
}

#[test]
fn a_row_past_the_recap_whose_stamp_a_warm_moved_gets_it_back() {
    // Episode 5 was watched (slot 6), then a warm of episode 6 moved
    // the stamp to slot 7: the row read as 6.
    assert_eq!(
        restamp("6", 0, &stamp(7, "6"), &recap_after_three(), None),
        Some(Restamp {
            display: stamp(6, "5"),
            former: None,
        })
    );
}

#[test]
fn a_row_the_stamp_still_names_is_left_as_it_is() {
    assert_eq!(
        restamp("6", 0, &stamp(6, "5"), &recap_after_three(), None),
        None
    );
}

#[test]
fn a_row_whose_tag_is_its_slot_is_left_as_it_is() {
    // Slot 2 is episode 2: it reads right without the stamp.
    assert_eq!(
        restamp("2", 0, &stamp(7, "6"), &recap_after_three(), None),
        None
    );
}

#[test]
fn a_listing_that_numbers_the_show_on_another_offset_proves_nothing() {
    // The stamps were written against a listing shifted by 12; this one
    // is not that listing.
    assert_eq!(
        restamp("6", 12, &stamp(7, "6"), &recap_after_three(), None),
        None
    );
}

#[test]
fn a_listing_that_tags_the_stamped_row_otherwise_proves_nothing() {
    // The stamp says slot 7 is "6"; a listing where it is "5" was
    // re-listed since, and its slot 6 need not be the row's episode.
    assert_eq!(
        restamp("6", 0, &stamp(7, "5"), &recap_after_three(), None),
        None
    );
    // Nor one that no longer lists the stamped slot at all.
    assert_eq!(
        restamp("6", 0, &stamp(9, "8"), &recap_after_three(), None),
        None
    );
}

#[test]
fn a_row_the_listing_does_not_carry_proves_nothing() {
    assert_eq!(
        restamp("12", 0, &stamp(7, "6"), &recap_after_three(), None),
        None
    );
    assert_eq!(
        restamp("x", 0, &stamp(7, "6"), &recap_after_three(), None),
        None
    );
    assert_eq!(
        restamp("5.5", 0, &stamp(7, "6"), &recap_after_three(), None),
        None
    );
}

#[test]
fn a_listing_that_lists_the_row_s_slot_twice_proves_nothing() {
    let mut listing = recap_after_three();
    listing.push(row(6, Some("9")));
    assert_eq!(restamp("6", 0, &stamp(7, "6"), &listing, None), None);
}

#[test]
fn a_listing_that_lists_the_stamped_slot_twice_proves_nothing() {
    let mut listing = recap_after_three();
    listing.push(row(7, Some("9")));
    assert_eq!(restamp("6", 0, &stamp(7, "6"), &listing, None), None);
}

#[test]
fn a_row_tagged_with_its_own_slot_is_left_as_it_is() {
    // Numbered on from 2, past a recap: episode 4 sits in slot 5 and is
    // tagged "5", which reads as 4 under the offset without a stamp.
    let listing = vec![
        row(1, Some("2")),
        row(2, Some("3")),
        row(3, Some("3.5")),
        row(4, Some("4")),
        row(5, Some("5")),
    ];
    assert_eq!(restamp("5", 1, &stamp(2, "3"), &listing, None), None);
}

#[test]
fn a_row_an_earlier_version_tagged_otherwise_keeps_that_tag_beside_it() {
    // A stitched recap an earlier version called "4.5": a position the
    // player kept then is under that number.
    let mut listing = recap_after_three();
    listing[3].former_tag = Some("4.5".into());
    assert_eq!(
        restamp("4", 0, &stamp(7, "6"), &listing, None),
        Some(Restamp {
            display: stamp(4, "3.5"),
            former: Some(stamp(4, "4.5")),
        })
    );
}

#[test]
fn a_former_tag_another_row_carries_now_is_not_kept() {
    let mut listing = recap_after_three();
    listing[3].former_tag = Some("6".into());
    assert_eq!(
        restamp("4", 0, &stamp(7, "6"), &listing, None),
        Some(Restamp {
            display: stamp(4, "3.5"),
            former: None,
        })
    );
}

/// A listing of `count` episodes with a recap after episode `recap`,
/// numbered on from `offset`: episode `e` sits in slot `e`, or `e + 1`
/// past the recap, tagged `e + offset` — untagged before the recap
/// when there is no offset — and the recap is tagged
/// `"{recap + offset}.5"`.
fn listing(count: u32, recap: u32, offset: u32) -> Vec<EpisodeRef> {
    let mut rows = Vec::new();
    for e in 1..=count {
        let slot = if e <= recap { e } else { e + 1 };
        let tag = (offset > 0 || e > recap).then(|| (e + offset).to_string());
        rows.push(row(slot, tag.as_deref()));
    }
    rows.push(row(recap + 1, Some(&format!("{}.5", recap + offset))));
    rows.sort_by_key(|r| r.number);
    rows
}

fn shown(e: &EpisodeRef) -> String {
    e.number2.clone().unwrap_or_else(|| e.number.to_string())
}

/// The store row a show's stamp and offset make.
fn store(offset: u32, display: &(u32, String)) -> Numbering {
    Numbering::from_body(&format!("show\t{offset}\t{}\t{}\n", display.0, display.1))
}

proptest! {
    #[test]
    fn a_legacy_row_reads_as_the_episode_its_slot_is(
        count in 2u32..30,
        recap_at in 0u32..1000,
        offset in prop_oneof![Just(0u32), 1u32..60],
        row_at in 0usize..1000,
        stamp_at in 0usize..1000,
    ) {
        let recap = 1 + recap_at % (count - 1);
        let rows = listing(count, recap, offset);
        let held = &rows[row_at % rows.len()];
        // The stamp a resolve of a row whose tag is not its slot left.
        let stamped: Vec<&EpisodeRef> = rows
            .iter()
            .filter(|e| e.number != held.number && shown(e) != e.number.to_string())
            .collect();
        prop_assume!(!stamped.is_empty());
        let other = stamped[stamp_at % stamped.len()];
        let before = (other.number, shown(other));
        let got = restamp(&held.number.to_string(), offset, &before, &rows, None);
        if let Some(fix) = &got {
            prop_assert_eq!(fix.display.0, held.number);
        }
        let after = got.map_or(before, |fix| fix.display);
        prop_assert_eq!(
            store(offset, &after).read_ep_no("show", &held.number.to_string()),
            kitsu_ep_no(&shown(held), offset)
        );
    }

    #[test]
    fn a_listing_numbered_on_another_offset_restamps_nothing(
        count in 2u32..30,
        recap_at in 0u32..1000,
        offset in 0u32..60,
        stored in 0u32..60,
        row_at in 0usize..1000,
        stamp_at in 0usize..1000,
    ) {
        prop_assume!(stored != offset);
        let recap = 1 + recap_at % (count - 1);
        let rows = listing(count, recap, offset);
        let held = &rows[row_at % rows.len()];
        let other = &rows[stamp_at % rows.len()];
        prop_assert_eq!(
            restamp(&held.number.to_string(), stored, &(other.number, shown(other)), &rows, None),
            None
        );
    }

    #[test]
    fn a_stamp_the_listing_tags_otherwise_restamps_nothing(
        count in 2u32..30,
        recap_at in 0u32..1000,
        offset in 0u32..60,
        row_at in 0usize..1000,
        stamp_slot in 0u32..80,
        stamp_tag in "[0-9]{1,3}(\\.5)?",
    ) {
        let recap = 1 + recap_at % (count - 1);
        let rows = listing(count, recap, offset);
        let held = &rows[row_at % rows.len()];
        prop_assume!(!rows.iter().any(|e| e.number == stamp_slot
            && crate::commands::play_native_episode::tag_matches(&shown(e), &stamp_tag)));
        prop_assert_eq!(
            restamp(&held.number.to_string(), offset, &(stamp_slot, stamp_tag), &rows, None),
            None
        );
    }
}
