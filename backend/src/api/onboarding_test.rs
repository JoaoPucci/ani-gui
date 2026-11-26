//! The feature tips' routes over a test profile, counted at 0.19.0 —
//! the release that shipped this build's one tip, the watch-order
//! strip.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

use super::*;
use crate::api::tests::{body_string, test_app_state};
use crate::onboarding::state::{load, path_in, Loaded};

fn r(text: &str) -> Release {
    Release::parse(text).expect("a release")
}

async fn call(
    td: &TempDir,
    at: &str,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let app = router_at(r(at)).with_state(Arc::new(test_app_state(td)));
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .expect("a request");
    let resp = app.oneshot(req).await.expect("an answer");
    let status = resp.status();
    let text = body_string(resp).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn record_path(td: &TempDir) -> std::path::PathBuf {
    path_in(&test_app_state(td).state_dir)
}

#[tokio::test]
async fn an_update_to_0_19_0_has_the_strip_tip_pending() {
    let td = TempDir::new().expect("tempdir");
    let (status, body) = call(&td, "0.19.0", "GET", "/api/onboarding", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "version": "0.19.0",
            "tips": [{
                "id": "watch-order",
                "kind": "single",
                "surface": "show",
                "moment": "arrive",
                "since": "0.19.0",
                "places": ["watch-order-strip"],
                "how_to": null,
                "pending": true,
            }],
        })
    );
}

#[tokio::test]
async fn before_and_after_its_window_the_strip_tip_is_not_offered() {
    for at in ["0.18.1", "0.21.0"] {
        let td = TempDir::new().expect("tempdir");
        let (_, body) = call(&td, at, "GET", "/api/onboarding", None).await;
        assert_eq!(body["tips"], json!([]), "{at}");
    }
}

#[tokio::test]
async fn a_tip_closed_is_never_pending_again() {
    let td = TempDir::new().expect("tempdir");
    let (status, _) = call(
        &td,
        "0.19.0",
        "POST",
        "/api/onboarding/seen",
        Some(json!({ "id": "watch-order" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = call(&td, "0.20.3", "GET", "/api/onboarding", None).await;
    assert_eq!(body["tips"][0]["pending"], json!(false));
    let Loaded::Record(rec) = load(&record_path(&td)) else {
        panic!("the record was written");
    };
    assert!(rec.seen.contains("watch-order"));
}

#[tokio::test]
async fn a_first_ever_install_is_offered_the_strip_tip_already_seen() {
    let td = TempDir::new().expect("tempdir");
    crate::onboarding::state::boot(&test_app_state(&td).state_dir, false, r("0.19.0"));
    let (_, body) = call(&td, "0.19.0", "GET", "/api/onboarding", None).await;
    assert_eq!(body["tips"][0]["id"], json!("watch-order"));
    assert_eq!(body["tips"][0]["pending"], json!(false));
    let Loaded::Record(rec) = load(&record_path(&td)) else {
        panic!("the record was written");
    };
    assert!(!rec.baseline_pending, "the baseline was taken and written");
    assert_eq!(rec.installed_in, Some(r("0.19.0")));
}

#[tokio::test]
async fn a_body_that_names_no_declared_tip_is_refused_with_the_keyed_envelope() {
    for body in [
        Some(json!({ "id": "no-such-tip" })),
        Some(json!({ "tip": "watch-order" })),
        Some(json!("watch-order")),
        None,
    ] {
        let td = TempDir::new().expect("tempdir");
        let (status, answer) =
            call(&td, "0.19.0", "POST", "/api/onboarding/seen", body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
        assert_eq!(
            answer["key"],
            json!("error.request.invalid_body"),
            "{body:?}"
        );
        assert_eq!(
            load(&record_path(&td)),
            Loaded::Absent,
            "nothing written for {body:?}"
        );
    }
}

#[tokio::test]
async fn a_corrupt_record_reads_as_an_update_and_the_next_close_replaces_it() {
    let td = TempDir::new().expect("tempdir");
    let path = record_path(&td);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    std::fs::write(&path, b"{not json").expect("written");
    let (_, body) = call(&td, "0.19.0", "GET", "/api/onboarding", None).await;
    assert_eq!(body["tips"][0]["pending"], json!(true));
    assert_eq!(load(&path), Loaded::Unreadable, "a read writes nothing new");
    call(
        &td,
        "0.19.0",
        "POST",
        "/api/onboarding/seen",
        Some(json!({ "id": "watch-order" })),
    )
    .await;
    assert!(matches!(load(&path), Loaded::Record(r) if r.seen.contains("watch-order")));
}

#[tokio::test]
async fn the_debug_list_shows_every_declared_tip_and_its_toggle_sets_seen_both_ways() {
    let td = TempDir::new().expect("tempdir");
    let (status, body) = call(&td, "0.19.0", "GET", "/api/onboarding/debug", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["version"], json!("0.19.0"));
    assert_eq!(body["installed_in"], Value::Null);
    let ids: Vec<&str> = body["tips"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|t| t["id"].as_str())
        .collect();
    let declared: Vec<&str> = TIPS.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, declared);
    let strip = &body["tips"][0];
    assert_eq!(strip["opens"], json!("0.19.0"));
    assert_eq!(strip["ends"], json!("0.21.0"));
    assert_eq!(strip["pending"], json!(true));

    for (seen, pending) in [(true, false), (false, true)] {
        let (status, _) = call(
            &td,
            "0.19.0",
            "PUT",
            "/api/onboarding/debug/seen",
            Some(json!({ "id": "watch-order", "seen": seen })),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, body) = call(&td, "0.19.0", "GET", "/api/onboarding", None).await;
        assert_eq!(
            body["tips"][0]["pending"],
            json!(pending),
            "after seen = {seen}"
        );
    }
}

#[tokio::test]
async fn a_record_that_cannot_be_written_fails_the_close_but_not_the_read() {
    let td = TempDir::new().expect("tempdir");
    let state_dir = test_app_state(&td).state_dir;
    // The state directory is a file, so nothing can be written in it.
    std::fs::write(&state_dir, b"").expect("written");
    let (status, _) = call(&td, "0.19.0", "GET", "/api/onboarding", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, answer) = call(
        &td,
        "0.19.0",
        "POST",
        "/api/onboarding/seen",
        Some(json!({ "id": "watch-order" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(answer["kind"], json!("io"));
}
