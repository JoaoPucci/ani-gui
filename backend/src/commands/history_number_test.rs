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
