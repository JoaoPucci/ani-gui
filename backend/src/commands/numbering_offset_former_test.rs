//! Tests for `crate::commands::numbering_offset::former`.

use super::*;

fn state() -> AppState {
    let dir = Box::leak(Box::new(tempfile::tempdir().expect("tmp")));
    let mut state = crate::commands::play::tests::state_with_proxy_origin();
    state.history_path = dir.path().join("history");
    state
}

const SHOW: &str = "hianime:the-show-1";

/// A stitched recap's stamp, (13, "24.5"), put right to "12.5".
fn repaired(state: &AppState) {
    crate::commands::numbering_offset::put_display(state, SHOW, 0, 13, "24.5");
    retag_display(state, SHOW, &[(13, "24.5".into(), "12.5".into())], &|_| {
        false
    });
}

#[test]
fn a_repaired_stamp_still_names_its_row_by_the_tag_it_replaced() {
    let state = state();
    repaired(&state);
    assert_eq!(display_stamp(&state, SHOW), Some((13, "12.5".to_string())));
    assert_eq!(former_stamp(&state, SHOW), Some((13, "24.5".to_string())));
}

#[test]
fn the_tag_a_repaired_stamp_replaced_survives_an_offset_put() {
    let state = state();
    repaired(&state);
    crate::commands::numbering_offset::put(&state, SHOW, 0);
    assert_eq!(former_stamp(&state, SHOW), Some((13, "24.5".to_string())));
}

#[test]
fn a_stamp_on_another_slot_drops_the_tag_the_last_one_replaced() {
    // The former tag named the row the stamp named; a stamp moved to
    // another row says nothing about it.
    let state = state();
    repaired(&state);
    crate::commands::numbering_offset::put_display(&state, SHOW, 0, 15, "14.5");
    assert_eq!(former_stamp(&state, SHOW), None);
    assert_eq!(display_stamp(&state, SHOW), Some((15, "14.5".to_string())));
}

#[test]
fn a_stamp_no_repair_touched_names_its_row_by_its_tag_alone() {
    let state = state();
    crate::commands::numbering_offset::put_display(&state, SHOW, 0, 8, "7.5");
    assert_eq!(former_stamp(&state, SHOW), None);
    assert_eq!(former_stamp(&state, "hianime:unstamped"), None);
}

#[test]
fn a_build_that_reads_four_columns_still_reads_a_repaired_row() {
    // Written beside the stamp, after it: a build from before reads
    // the slug, the offset and the stamp as it always has.
    let state = state();
    repaired(&state);
    let body = std::fs::read_to_string(state.history_path.with_file_name("ani-gui-offsets"))
        .expect("store");
    let cols: Vec<&str> = body.lines().next().expect("a row").split('\t').collect();
    assert_eq!(&cols[..4], &[SHOW, "0", "13", "12.5"]);
}

#[test]
fn the_stamp_beside_a_history_file_is_the_one_its_sidecar_keeps() {
    let state = state();
    crate::commands::numbering_offset::put_display(&state, SHOW, 0, 13, "24.5");
    assert_eq!(
        stamp_beside(&state.history_path, SHOW),
        Some((13, "24.5".to_string()))
    );
    assert_eq!(stamp_beside(&state.history_path, "hianime:unstamped"), None);
}

#[test]
fn a_repair_keeps_no_former_tag_a_row_carries_now() {
    // The listing tags another row "24.5" now: a position or a file
    // under "24.5" may be that row's, so the repaired stamp names no
    // former tag a reader would take for this one's.
    let state = state();
    crate::commands::numbering_offset::put_display(&state, SHOW, 0, 13, "24.5");
    retag_display(&state, SHOW, &[(13, "24.5".into(), "12.5".into())], &|f| {
        f == "24.5"
    });
    assert_eq!(display_stamp(&state, SHOW), Some((13, "12.5".to_string())));
    assert_eq!(former_stamp(&state, SHOW), None);
}
