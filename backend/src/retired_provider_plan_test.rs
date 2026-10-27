//! The retired-row plan over hand-written histories.

use super::*;

fn row(id: &str, walked: bool, shown_as: &[&str], stamp: Option<i64>, ep: &str) -> PlanRow {
    PlanRow {
        id: id.to_string(),
        walked,
        shown_as: shown_as.iter().map(|s| (*s).to_string()).collect(),
        stamp,
        ep: ep.to_string(),
        own_number: false,
    }
}

#[test]
fn a_retired_row_a_later_walked_row_resumes_over_is_superseded() {
    let rows = [
        row("naruto-shippuden-3687", false, &["1555"], None, "388"),
        row(
            "hianime:naruto-shippuden-600",
            true,
            &["1555"],
            Some(1_792_499_281_363),
            "425",
        ),
    ];
    assert_eq!(
        plan(&rows),
        Plan {
            superseded: vec!["naruto-shippuden-3687".into()],
            renumber: vec![],
        }
    );
}

#[test]
fn unstamped_rows_are_separated_by_progress() {
    let rows = [
        row("chaoshead-931", false, &["3871"], None, "2"),
        row("hianime:chaoshead-3925", true, &["3871"], None, "3"),
    ];
    assert_eq!(plan(&rows).superseded, vec!["chaoshead-931".to_string()]);
}

#[test]
fn a_retired_row_with_no_walked_row_for_its_show_is_kept_and_renumbered() {
    let rows = [
        row(
            "bleach-tybw-the-separation-677",
            false,
            &["46903"],
            None,
            "8",
        ),
        row(
            "hianime:bleach-1369",
            true,
            &["244"],
            Some(1_791_088_188_438),
            "267",
        ),
    ];
    assert_eq!(
        plan(&rows),
        Plan {
            superseded: vec![],
            renumber: vec!["bleach-tybw-the-separation-677".into()],
        }
    );
}

#[test]
fn a_retired_row_still_resumed_from_stays() {
    // The retired row is the later watch: both surfaces resume from it,
    // so removing it would move the show's resume back.
    let rows = [
        row("hianime:show-1", false, &["9"], Some(2_000), "5"),
        row("test-a:show-2", true, &["9"], Some(1_000), "7"),
    ];
    assert_eq!(plan(&rows).superseded, Vec::<String>::new());
}

#[test]
fn a_retired_row_shown_as_nothing_is_kept() {
    let rows = [
        row("baki-the-grappler-505", false, &[], None, "7"),
        row("hianime:grappler-baki-4820", true, &[], Some(1), "1"),
    ];
    assert_eq!(
        plan(&rows).renumber,
        vec!["baki-the-grappler-505".to_string()]
    );
}

#[test]
fn every_entry_a_retired_row_is_shown_as_needs_a_walked_row_over_it() {
    // Shown as two entries, superseded only for one of them: the other
    // would lose the row it resumes from.
    let rows = [
        row("old-1", false, &["1", "2"], None, "3"),
        row("test-a:new-1", true, &["1"], Some(5), "4"),
    ];
    assert!(plan(&rows).superseded.is_empty());
}

#[test]
fn kitsu_ids_compare_as_ids() {
    let rows = [
        row("old-1", false, &[" 49444"], None, "6"),
        row("test-a:new-1", true, &["49444"], Some(5), "9"),
    ];
    assert_eq!(plan(&rows).superseded, vec!["old-1".to_string()]);
}

#[test]
fn walked_rows_and_rows_already_in_own_numbering_are_left_alone() {
    let mut kept = row("old-1", false, &[], None, "3");
    kept.own_number = true;
    let rows = [kept, row("test-a:new-1", true, &[], None, "4")];
    assert!(plan(&rows).is_empty());
}
