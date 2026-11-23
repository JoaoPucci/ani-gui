//! The watch-order routes over a seeded cache and a scratch history: no
//! network. The JoJo franchise from `api/franchise_test.rs`'s seed.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::app::AppState;
use crate::cache::meta_cache_put;
use crate::history::HistoryEntry;

fn entry(id: &str, title: &str, subtype: &str, start: &str, episodes: u32) -> Value {
    json!({ "id": id, "title": title, "subtype": subtype, "start": start,
        "episodes": episodes, "status": "finished", "poster": null, "cover": null })
}

fn rel(role: &str, entry: &Value) -> Value {
    json!({ "role": role, "kind": "anime", "id": entry["id"], "subtype": entry["subtype"], "entry": entry })
}

fn seed(state: &AppState, id: &str, relations: Vec<Value>) {
    let listing = json!({ "relations": relations, "complete": true });
    meta_cache_put(
        &state.cache_pool,
        &format!("kitsu:franchise-rel:v1:{id}"),
        &listing.to_string(),
        3600,
    )
    .expect("seed listing");
}

fn seed_detail(state: &AppState, id: &str, title: &str, subtype: &str, start: &str, episodes: u32) {
    let detail = json!({
        "id": id, "canonical_title": title, "titles": {}, "abbreviated_titles": [], "slug": null,
        "synopsis": null, "start_date": start, "end_date": null, "episode_count": episodes,
        "average_rating": null, "subtype": subtype, "status": "finished", "age_rating": null,
        "popularity_rank": null, "poster_image": null, "cover_image": null
    });
    meta_cache_put(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key(id),
        &detail.to_string(),
        3600,
    )
    .expect("seed detail");
}

/// JoJo's TV run (7158 → 8063), the 1993 OVA (610) and the 2000 OVA
/// (7730), every listing and detail the walks from 7158, 7730 and 8063 read.
fn jojo(td: &TempDir) -> AppState {
    let state = crate::api::tests::test_app_state(td);
    let tv = entry(
        "7158",
        "JoJo no Kimyou na Bouken (TV)",
        "TV",
        "2012-10-06",
        26,
    );
    let sc = entry(
        "8063",
        "JoJo no Kimyou na Bouken: Stardust Crusaders",
        "TV",
        "2014-04-05",
        24,
    );
    let ova = entry(
        "7730",
        "JoJo no Kimyou na Bouken: Adventure",
        "OVA",
        "2000-05-25",
        7,
    );
    let old = entry("610", "JoJo no Kimyou na Bouken", "OVA", "1993-11-19", 6);
    seed(
        &state,
        "7730",
        vec![
            rel("sequel", &old),
            rel("alternative_version", &sc),
            rel("prequel", &tv),
        ],
    );
    seed(&state, "610", vec![rel("prequel", &ova)]);
    seed(&state, "7158", vec![rel("sequel", &sc)]);
    seed(
        &state,
        "8063",
        vec![rel("prequel", &tv), rel("alternative_version", &ova)],
    );
    seed_detail(
        &state,
        "7730",
        "JoJo no Kimyou na Bouken: Adventure",
        "OVA",
        "2000-05-25",
        7,
    );
    seed_detail(
        &state,
        "7158",
        "JoJo no Kimyou na Bouken (TV)",
        "TV",
        "2012-10-06",
        26,
    );
    state
}

fn row(id: &str, kitsu_id: &str, ep_no: &str) -> HistoryEntry {
    HistoryEntry {
        ep_no: ep_no.into(),
        id: id.into(),
        title: id.into(),
        watched_at: None,
        kitsu_id: Some(kitsu_id.into()),
        own_number: true,
    }
}

fn router(state: AppState) -> Router {
    crate::api::test_router(std::sync::Arc::new(state))
}

async fn call(router: &Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let r = router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(if method == "GET" {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = r.status();
    let bytes = axum::body::to_bytes(r.into_body(), 1024 * 1024)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

const TODAY: &str = "2026-11-02";

/// The walk the franchise route serves for `id`, as the watch order reads it.
async fn walk(router: &Router, id: &str) -> crate::watch_order::Franchise {
    let (status, body) = call(
        router,
        "GET",
        &format!("/api/kitsu/franchise/{id}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_value(body).expect("a franchise")
}

fn rows(list: &[(&str, &str)]) -> Vec<crate::watch_order::HistoryRow> {
    list.iter()
        .map(|(k, ep)| crate::watch_order::HistoryRow {
            kitsu_id: Some((*k).into()),
            ep_no: (*ep).into(),
        })
        .collect()
}

#[tokio::test]
async fn the_show_route_draws_the_strip_from_the_walk_and_the_rows_its_entries_resume_from() {
    let td = tempfile::tempdir().expect("td");
    let state = jojo(&td);
    crate::history::write_atomic(&state.history_path, &[row("p:sc", "8063", "4")])
        .expect("history");
    let router = router(state);
    let (status, got) = call(
        &router,
        "POST",
        "/api/watch-order/7158/show",
        json!({ "today": TODAY }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{got}");
    let want = crate::watch_order::wire::show_view(
        &walk(&router, "7158").await,
        &rows(&[("8063", "4")]),
        "7158",
        crate::watch_order::WatchOrder::Linked,
        TODAY,
    );
    assert_eq!(got, serde_json::to_value(&want).expect("json"));
    assert_eq!(got["order"], "linked");
    assert_eq!(got["strip"]["after"][0]["id"], "8063");
    assert_eq!(
        got["strip"]["after"][0]["progress"],
        json!({ "kind": "partway", "episode": 4.0, "total": 24 })
    );
}

#[tokio::test]
async fn the_routes_read_the_order_setting() {
    let td = tempfile::tempdir().expect("td");
    let state = jojo(&td);
    std::fs::write(&state.config_path, "watch_order = \"released\"\n").expect("config");
    let router = router(state);
    let (_, show) = call(
        &router,
        "POST",
        "/api/watch-order/7158/show",
        json!({ "today": TODAY }),
    )
    .await;
    assert_eq!(show["order"], "released");
    let (_, page) = call(
        &router,
        "POST",
        "/api/watch-order/7158/franchise",
        json!({ "today": TODAY }),
    )
    .await;
    assert_eq!(page["order"], "released");
}

#[tokio::test]
async fn history_that_cannot_be_read_draws_the_strip_without_progress() {
    let td = tempfile::tempdir().expect("td");
    let state = jojo(&td);
    // A directory where the history file should be: every read fails.
    std::fs::create_dir_all(&state.history_path).expect("a directory");
    let router = router(state);
    let (status, got) = call(
        &router,
        "POST",
        "/api/watch-order/7158/show",
        json!({ "today": TODAY }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        got["strip"]["after"][0]["progress"],
        json!({ "kind": "none" })
    );
}

#[tokio::test]
async fn a_franchise_that_cannot_be_read_fails_every_walking_route() {
    let td = tempfile::tempdir().expect("td");
    // Nothing seeded, and Kitsu unreachable.
    let router = router(crate::api::tests::test_app_state(&td));
    for route in ["show", "franchise", "handoff"] {
        let (status, _) = call(
            &router,
            "POST",
            &format!("/api/watch-order/9/{route}"),
            json!({ "today": TODAY }),
        )
        .await;
        assert!(!status.is_success(), "{route}: {status}");
    }
    let (status, _) = call(
        &router,
        "POST",
        "/api/watch-order/9/up-next",
        json!({ "today": TODAY, "episode": 1 }),
    )
    .await;
    assert!(!status.is_success());
}

#[tokio::test]
async fn the_routes_refuse_an_id_that_is_not_a_kitsu_id() {
    let td = tempfile::tempdir().expect("td");
    let router = router(jojo(&td));
    let (status, _) = call(
        &router,
        "POST",
        "/api/watch-order/abc/show",
        json!({ "today": TODAY }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_franchise_route_lays_out_the_franchise_page() {
    let td = tempfile::tempdir().expect("td");
    let router = router(jojo(&td));
    let (status, got) = call(
        &router,
        "POST",
        "/api/watch-order/7158/franchise",
        json!({ "today": TODAY }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let want = crate::watch_order::wire::franchise_view(
        &walk(&router, "7158").await,
        &[],
        "7158",
        crate::watch_order::WatchOrder::Linked,
        TODAY,
    );
    assert_eq!(got, serde_json::to_value(&want).expect("json"));
    assert_eq!(got["page"]["opened_from"]["id"], "7158");
}

#[tokio::test]
async fn the_up_next_route_names_what_follows_the_finale_and_nothing_before_it() {
    let td = tempfile::tempdir().expect("td");
    let router = router(jojo(&td));
    let (_, at_finale) = call(
        &router,
        "POST",
        "/api/watch-order/7158/up-next",
        json!({ "today": TODAY, "episode": 26 }),
    )
    .await;
    assert_eq!(at_finale["card"]["kind"], "entry");
    assert_eq!(at_finale["card"]["card"]["kind"], "one");
    assert_eq!(at_finale["card"]["card"]["entry"]["id"], "8063");
    let (_, before) = call(
        &router,
        "POST",
        "/api/watch-order/7158/up-next",
        json!({ "today": TODAY, "episode": 25 }),
    )
    .await;
    assert_eq!(before, json!({ "card": null }));
}

#[tokio::test]
async fn the_handoff_route_moves_on_unless_a_row_resumes_the_next_entry() {
    let td = tempfile::tempdir().expect("td");
    let state = jojo(&td);
    let history = state.history_path.clone();
    let router = router(state);
    let (status, got) = call(
        &router,
        "POST",
        "/api/watch-order/7158/handoff",
        json!({ "today": TODAY }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(got["handoff"]["finished"]["id"], "7158");
    assert_eq!(got["handoff"]["next"]["id"], "8063");
    crate::history::write_atomic(&history, &[row("p:sc", "8063", "2")]).expect("history");
    let (_, claimed) = call(
        &router,
        "POST",
        "/api/watch-order/7158/handoff",
        json!({ "today": TODAY }),
    )
    .await;
    assert_eq!(claimed, json!({ "handoff": null }));
}

#[tokio::test]
async fn the_rail_route_is_the_rail_over_what_home_sends() {
    let td = tempfile::tempdir().expect("td");
    let router = router(jojo(&td));
    let next = entry("8063", "SC", "TV", "2014-04-05", 24);
    let done = entry("7158", "TV", "TV", "2012-10-06", 26);
    let body = json!({
        "rows": [
            { "row_id": "a", "matched": { "id": "7158", "status": "finished", "episode_count": 26 }, "ep_no": "26" },
            { "row_id": "b", "matched": { "id": "7158", "status": "finished", "episode_count": 26 }, "ep_no": "12" }
        ],
        "history": [{ "id": "a" }, { "id": "b" }],
        "matches": { "a": "7158", "b": "7158" },
        "series": { "7158": { "finished": done, "next": next } }
    });
    let (status, got) = call(&router, "POST", "/api/watch-order/rail", body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(got["asks"], json!({ "a": "7158", "b": null }));
    assert_eq!(got["handoffs"]["a"]["next"]["id"], "8063");
    assert!(got["handoffs"].get("b").is_none());
}
