//! Tests for `crate::commands::history_number`.

use super::*;

const SHOW: &str = "hianime:stitched-show-4";

fn state(td: &tempfile::TempDir) -> AppState {
    let mut state = crate::commands::play::tests::state_with_proxy_origin();
    state.history_path = td.path().join("history");
    state
}

/// The row a watch of the stitched recap left before it was
/// renumbered: slot 2 in the file, (2, "2.5") in the sidecar.
fn seed_former_recap_row(state: &AppState) {
    let row = crate::history::parse(&format!("2\t{SHOW}\tStitched Show\n"))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    crate::commands::numbering_offset::put_display(state, SHOW, 0, 2, "2.5");
}

fn repair(state: &AppState) {
    crate::commands::numbering_offset::retag_display(
        state,
        SHOW,
        &[(2, "2.5".into(), "1.5".into())],
        &|_| false,
    );
}

#[test]
fn a_repaired_row_reads_as_the_episode_it_is_and_names_what_it_read_as() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    assert_eq!(
        history_number(&state, SHOW).expect("read"),
        Some(HistoryNumber {
            ep_no: "1.5".into(),
            former_ep_no: Some("2.5".into()),
        })
    );
}

#[test]
fn a_row_no_repair_touched_reads_as_it_did() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    assert_eq!(
        history_number(&state, SHOW).expect("read"),
        Some(HistoryNumber {
            ep_no: "2.5".into(),
            former_ep_no: None,
        })
    );
}

#[test]
fn a_row_on_another_episode_names_no_former_number() {
    // The stamp was repaired, but the row has moved on to episode 5:
    // the former number is the recap's, not this episode's.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    let row = crate::history::parse(&format!("6\t{SHOW}\tStitched Show\n"))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    let got = history_number(&state, SHOW).expect("read").expect("a row");
    assert_eq!(got.former_ep_no, None);
}

#[test]
fn a_show_with_no_row_has_no_number() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    assert_eq!(history_number(&state, SHOW).expect("read"), None);
}

#[tokio::test]
async fn the_route_answers_with_the_rows_number() {
    use tower::ServiceExt as _;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    repair(&state);
    let router = crate::api::build_api_router(std::sync::Arc::new(state));
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/history/hianime:stitched-show-4/number")
                .body(axum::body::Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1 << 16)
        .await
        .expect("body");
    let got: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(
        got,
        serde_json::json!({ "ep_no": "1.5", "former_ep_no": "2.5" })
    );
}

/// What [`history_number`] answers when `write` lands just before its
/// `n`th read of the numbering store.
fn number_with_a_write_before_read(
    n: usize,
    write: impl FnOnce(&AppState) + 'static,
) -> Option<HistoryNumber> {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_former_recap_row(&state);
    let writing = state.clone();
    crate::commands::numbering_offset::read_tap::before_read(n, move || write(&writing));
    let got = history_number(&state, SHOW).expect("read");
    crate::commands::numbering_offset::read_tap::clear();
    got
}

#[test]
fn a_number_read_while_a_repair_lands_is_the_row_before_it_or_after_it() {
    // A probe or a download repairs the stamp without the history
    // guard. Wherever the repair lands among the number read's reads,
    // the answer is the row as it was before the repair — "2.5", no
    // former number — or as it is after — "1.5", formerly "2.5" —
    // never one read of each.
    let before = Some(HistoryNumber {
        ep_no: "2.5".into(),
        former_ep_no: None,
    });
    let after = Some(HistoryNumber {
        ep_no: "1.5".into(),
        former_ep_no: Some("2.5".into()),
    });
    for n in 1..=6 {
        let got = number_with_a_write_before_read(n, repair);
        assert!(
            got == before || got == after,
            "a repair before read {n} gave {got:?}"
        );
    }
}

#[test]
fn a_list_read_while_the_offset_and_stamp_move_reads_one_of_them() {
    // The offset and the stamp move together in one write. A row read
    // across it reads both from before (episode "2.5") or both from
    // after (offset 1, slot 2 tagged "4.5": episode "3.5").
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    for n in 1..=3 {
        seed_former_recap_row(&state);
        let writing = state.clone();
        crate::commands::numbering_offset::read_tap::before_read(n, move || {
            crate::commands::numbering_offset::put_display(&writing, SHOW, 1, 2, "4.5");
        });
        let read = crate::commands::history::history_list(&state).expect("list");
        crate::commands::numbering_offset::read_tap::clear();
        let ep = read.into_iter().find(|e| e.id == SHOW).expect("row").ep_no;
        assert!(
            ep == "2.5" || ep == "3.5",
            "a write before read {n} gave {ep}"
        );
    }
}

#[test]
fn a_resume_read_while_the_offset_and_stamp_move_reads_one_of_them() {
    // The detail page's resume row, read by its Kitsu entry, reads the
    // store as the list does: both from before ("2.5") or both from
    // after ("3.5").
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    for n in 1..=3 {
        crate::history::upsert_and_write(
            &state.history_path,
            crate::history::HistoryEntry {
                ep_no: "2".into(),
                id: SHOW.into(),
                title: "Stitched Show".into(),
                watched_at: None,
                kitsu_id: Some("77".into()),
                own_number: false,
            },
        )
        .expect("row");
        crate::commands::numbering_offset::put_display(&state, SHOW, 0, 2, "2.5");
        let writing = state.clone();
        crate::commands::numbering_offset::read_tap::before_read(n, move || {
            crate::commands::numbering_offset::put_display(&writing, SHOW, 1, 2, "4.5");
        });
        let read = crate::commands::history::history_by_kitsu(&state, "77").expect("read");
        crate::commands::numbering_offset::read_tap::clear();
        let ep = read.expect("the row").ep_no;
        assert!(
            ep == "2.5" || ep == "3.5",
            "a write before read {n} gave {ep}"
        );
    }
}
