//! `POST /api/home/hero` makes the home hero's pick under the launch's
//! flags and keeps its walk; `GET /api/home/hero/walk` hands the Debug
//! page that walk and the flag it ran under.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

use super::test_router;
use super::tests::{body_string, test_app_state};
use crate::app::AppState;
use crate::flags::store::Origin;
use crate::flags::FlagStore;

async fn send(router: &axum::Router, method: &str, uri: &str, body: &str) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if !body.is_empty() {
        req = req.header(header::CONTENT_TYPE, "application/json");
    }
    let response = router
        .clone()
        .oneshot(req.body(Body::from(body.to_owned())).expect("req"))
        .await
        .expect("oneshot");
    let status = response.status();
    let text = body_string(response).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

/// Three finished shows with covers, then an airing one with a banner.
fn trending() -> String {
    let cover = |id: &str| {
        json!({ "id": id, "title": format!("Show {id}"), "status": "finished",
                "cover_image": { "large": "https://media.kitsu.app/c.jpg" } })
    };
    json!({ "shows": [
        cover("1"),
        cover("2"),
        cover("3"),
        { "id": "4", "title": "Show 4", "status": "current",
          "anilist_banner": "https://s4.anilist.co/b.jpg" },
    ] })
    .to_string()
}

fn state_with(td: &TempDir, flags: FlagStore) -> Arc<AppState> {
    Arc::new(AppState {
        flags,
        ..test_app_state(td)
    })
}

/// A store settled from a file turning the airing preference on.
fn on_from(origin: Origin) -> FlagStore {
    let store = FlagStore::pending(crate::flags::DECLARED);
    let doc = crate::flags::resolve::parse_document(br#"{"flags":{"hero_airing_first":true}}"#);
    store.settle(doc.as_ref(), origin);
    store
}

#[tokio::test]
async fn off_by_default_the_hero_is_the_first_shows_with_art() {
    let td = TempDir::new().expect("tempdir");
    let router = test_router(state_with(&td, FlagStore::fixed(crate::flags::DECLARED)));
    let (status, body) = send(&router, "POST", "/api/home/hero", &trending()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["hero"], json!(["1", "2", "3"]));
    assert_eq!(body["row"], json!(["4"]));
    assert_eq!(body["walk"]["airing_first"], json!(false));
    assert_eq!(body["walk"]["count"], json!(3));
    assert_eq!(body["walk"]["entries"], json!(4));
    assert_eq!(
        body["walk"]["steps"][0],
        json!({
            "rank": 1, "id": "1", "title": "Show 1", "art": "cover",
            "status": "finished", "airing": false, "preference_applied": false,
            "verdict": { "kind": "taken", "slot": 0, "because": "art" }
        })
    );
}

#[tokio::test]
async fn on_the_airing_show_with_art_leads_the_hero() {
    let td = TempDir::new().expect("tempdir");
    let router = test_router(state_with(&td, on_from(Origin::File)));
    let (status, body) = send(&router, "POST", "/api/home/hero", &trending()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["hero"], json!(["4", "1", "2"]));
    assert_eq!(body["row"], json!(["3"]));
    assert_eq!(body["walk"]["airing_first"], json!(true));
    assert_eq!(
        body["walk"]["steps"][2]["verdict"],
        json!({ "kind": "skipped", "because": "not_airing" })
    );
    assert_eq!(body["walk"]["steps"][3]["art"], json!("banner"));
}

#[tokio::test]
async fn before_any_pick_the_walk_is_null_and_the_flag_is_named() {
    let td = TempDir::new().expect("tempdir");
    let router = test_router(state_with(&td, FlagStore::fixed(crate::flags::DECLARED)));
    let (status, body) = send(&router, "GET", "/api/home/hero/walk", "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "flag": { "name": "hero_airing_first", "on": false, "origin": "defaults" },
            "walk": null
        })
    );
}

#[tokio::test]
async fn the_walk_is_the_last_picks_own() {
    let td = TempDir::new().expect("tempdir");
    let router = test_router(state_with(&td, on_from(Origin::Test)));
    let (_, first) = send(
        &router,
        "POST",
        "/api/home/hero",
        r#"{"shows":[{"id":"9"}]}"#,
    )
    .await;
    let (_, last) = send(&router, "POST", "/api/home/hero", &trending()).await;
    assert_ne!(first["walk"], last["walk"]);
    let (status, body) = send(&router, "GET", "/api/home/hero/walk", "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["walk"], last["walk"]);
    assert_eq!(
        body["flag"],
        json!({ "name": "hero_airing_first", "on": true, "origin": "test" })
    );
}

#[tokio::test]
async fn a_body_that_is_not_a_list_of_shows_is_refused_with_the_keyed_envelope() {
    let td = TempDir::new().expect("tempdir");
    let router = test_router(state_with(&td, FlagStore::fixed(crate::flags::DECLARED)));
    for body in [
        "",
        "not json",
        r#"{"shows":"x"}"#,
        r#"{"shows":[{"title":"no id"}]}"#,
    ] {
        let (status, envelope) = send(&router, "POST", "/api/home/hero", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(envelope["kind"], json!("invalid_request_body"), "{body}");
    }
    let (_, walk) = send(&router, "GET", "/api/home/hero/walk", "").await;
    assert_eq!(walk["walk"], Value::Null, "a refused body records no walk");
}

#[tokio::test]
async fn a_list_longer_than_any_trending_list_is_refused() {
    let td = TempDir::new().expect("tempdir");
    let router = test_router(state_with(&td, FlagStore::fixed(crate::flags::DECLARED)));
    let shows: Vec<Value> = (0..=crate::hero::MAX_SHOWS)
        .map(|i| json!({ "id": i.to_string() }))
        .collect();
    let body = json!({ "shows": shows }).to_string();
    let (status, envelope) = send(&router, "POST", "/api/home/hero", &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(envelope["kind"], json!("invalid_request_body"));
    let at_cap: Vec<Value> = shows.into_iter().take(crate::hero::MAX_SHOWS).collect();
    let body = json!({ "shows": at_cap }).to_string();
    let (status, _) = send(&router, "POST", "/api/home/hero", &body).await;
    assert_eq!(status, StatusCode::OK);
}
