//! The fifth column: the Kitsu id of the show the user played.

use super::*;
use proptest::prelude::*;

fn entry(title: &str, watched_at: Option<i64>, kitsu_id: Option<&str>) -> HistoryEntry {
    HistoryEntry {
        ep_no: "1".into(),
        id: "hianime:show-1".into(),
        title: title.into(),
        watched_at,
        kitsu_id: kitsu_id.map(str::to_owned),
    }
}

#[test]
fn a_row_carries_the_kitsu_id_after_its_moment() {
    let rows = parse("1\thianime:show-1\tThe Show\t1700000000000\tkitsu:49877\n");
    assert_eq!(
        rows,
        vec![entry("The Show", Some(1_700_000_000_000), Some("49877"))]
    );
}

#[test]
fn a_row_carries_the_kitsu_id_without_a_moment() {
    let rows = parse("1\thianime:show-1\tThe Show\tkitsu:49877\n");
    assert_eq!(rows, vec![entry("The Show", None, Some("49877"))]);
}

#[test]
fn only_a_marked_run_of_digits_is_the_kitsu_id() {
    // A title of its own that ends in something marker-shaped stays
    // the title: the marker must be followed by digits and nothing
    // else.
    for title in [
        "The Show\tkitsu:",
        "The Show\tkitsu:12a",
        "The Show\tKitsu:12",
        "The Show\t49877",
    ] {
        let rows = parse(&format!("1\thianime:show-1\t{title}\n"));
        assert_eq!(rows, vec![entry(title, None, None)], "{title:?}");
    }
}

#[test]
fn a_title_ending_in_a_number_is_still_the_title() {
    let rows = parse("1\thianime:show-1\tMobile Suit\t0080\tkitsu:7\n");
    assert_eq!(rows, vec![entry("Mobile Suit\t0080", None, Some("7"))]);
}

#[test]
fn the_kitsu_id_is_written_last_and_only_when_known() {
    assert_eq!(
        serialize(&[entry("The Show", Some(1_700_000_000_000), Some("49877"))]),
        "1\thianime:show-1\tThe Show\t1700000000000\tkitsu:49877\n"
    );
    assert_eq!(
        serialize(&[entry("The Show", None, Some("49877"))]),
        "1\thianime:show-1\tThe Show\tkitsu:49877\n"
    );
    assert_eq!(
        serialize(&[entry("The Show", None, None)]),
        "1\thianime:show-1\tThe Show\n",
        "a row without the id is written as it always was"
    );
}

#[test]
fn a_write_without_the_id_keeps_the_rows_and_a_new_one_replaces_it() {
    let mut rows = vec![entry("The Show", None, Some("49877"))];
    upsert(&mut rows, entry("The Show", Some(1_700_000_000_000), None));
    assert_eq!(rows[0].kitsu_id.as_deref(), Some("49877"));
    upsert(&mut rows, entry("The Show", None, Some("1623")));
    assert_eq!(
        rows[0].kitsu_id.as_deref(),
        Some("1623"),
        "the newest play wins"
    );
}

#[test]
fn only_digits_are_taken_as_a_kitsu_id() {
    assert_eq!(kitsu_id_of("49877").as_deref(), Some("49877"));
    assert_eq!(kitsu_id_of(" 49877 ").as_deref(), Some("49877"));
    for bad in ["", "abc", "12\t3", "kitsu:12", "-1", "1e3"] {
        assert_eq!(kitsu_id_of(bad), None, "{bad:?}");
    }
}

proptest! {
    /// Whatever the title, a row reads back as it was written.
    #[test]
    fn a_row_reads_back_as_written(
        title in "[^\t\n\r]{1,40}(\t[^\t\n\r]{1,10}){0,2}",
        moment in proptest::option::of(1_577_836_800_000_i64..4_102_444_800_000),
        kitsu in proptest::option::of("[1-9][0-9]{0,6}"),
    ) {
        // A title that itself ends in a plausible moment or a marked id
        // is the documented ambiguity; generate the others.
        prop_assume!(!title.contains("kitsu:"));
        let tail = title.rsplit('\t').next().unwrap_or_default();
        prop_assume!(tail.parse::<i64>().map_or(true, |n| !(1_577_836_800_000..4_102_444_800_000).contains(&n)));
        let row = HistoryEntry {
            ep_no: "3".into(),
            id: "hianime:x-1".into(),
            title: title.clone(),
            watched_at: moment,
            kitsu_id: kitsu.clone(),
        };
        prop_assert_eq!(parse(&serialize(std::slice::from_ref(&row))), vec![row]);
    }
}
