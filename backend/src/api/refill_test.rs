//! The refill routes: a read carries its pending refills' tickets in a
//! header only when there are any, and a wait answers for each ended
//! ticket.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use super::{with_refills, REFILL_HEADER};
use crate::app::AppState;
use crate::cache::meta_cache_put;

fn test_state() -> Arc<AppState> {
    Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui-refill/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-refill/images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
        ),
        config_path: PathBuf::from("/tmp/ani-gui-refill/config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-refill/state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

async fn send(
    state: Arc<AppState>,
    method: &str,
    uri: &str,
    body: &str,
) -> axum::response::Response {
    crate::api::build_api_router(state)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("oneshot")
}

#[test]
fn a_response_carries_its_tickets_comma_separated() {
    let resp = with_refills("x", &[3, 4]);
    assert_eq!(
        resp.headers()
            .get(REFILL_HEADER)
            .and_then(|v| v.to_str().ok()),
        Some("3,4")
    );
}

#[test]
fn a_response_with_no_tickets_carries_no_header() {
    let resp = with_refills("x", &[]);
    assert!(resp.headers().get(REFILL_HEADER).is_none());
}

#[tokio::test]
async fn a_fully_served_episode_page_carries_no_refill() {
    let state = test_state();
    let eps = serde_json::json!([{
        "id": "e1", "canonical_title": "One", "season_number": 1, "number": 1,
        "relative_number": 1, "length": null, "synopsis": null, "airdate": null,
        "thumbnail": { "original": "https://kitsu.cdn/1.jpg" }
    }]);
    meta_cache_put(
        &state.cache_pool,
        "kitsu:episodes:12:p1",
        &eps.to_string(),
        3600,
    )
    .expect("seed");
    let resp = send(state, "GET", "/api/kitsu/episodes/12?page=1", "").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get(REFILL_HEADER).is_none());
}

#[tokio::test]
async fn a_wait_on_a_ticket_never_issued_answers_it_ended_unlanded() {
    let resp = send(
        test_state(),
        "POST",
        "/api/refills/wait",
        r#"{"tickets":[18446744073709551615]}"#,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("body");
    let got: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(
        got,
        serde_json::json!([{ "ticket": 18_446_744_073_709_551_615_u64, "landed": false }])
    );
}
