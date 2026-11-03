//! The franchise route over a seeded cache: no network.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::cache::meta_cache_put;

fn test_state() -> Arc<crate::app::AppState> {
    Arc::new(crate::app::AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
        ),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

fn entry(id: &str, title: &str, subtype: &str, start: &str, episodes: u32) -> Value {
    json!({ "id": id, "title": title, "subtype": subtype, "start": start,
        "episodes": episodes, "status": "finished",
        "poster": format!("https://media.kitsu.app/anime/poster_images/{id}/medium.jpg"),
        "cover": null })
}

fn rel(role: &str, entry: &Value) -> Value {
    json!({ "role": role, "kind": "anime", "id": entry["id"], "subtype": entry["subtype"], "entry": entry })
}

fn seed(state: &crate::app::AppState, id: &str, relations: Vec<Value>) {
    let listing = json!({ "relations": relations, "complete": true });
    meta_cache_put(
        &state.cache_pool,
        &format!("kitsu:franchise-rel:v1:{id}"),
        &listing.to_string(),
        3600,
    )
    .expect("seed listing");
}

/// The 2000 JoJo OVA's franchise, from cached listings and the cached
/// detail of the OVA itself.
#[tokio::test]
async fn franchise_route_serves_the_walk_from_the_cache() {
    let state = test_state();
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
    let detail = json!({
        "id": "7730", "canonical_title": "JoJo no Kimyou na Bouken: Adventure",
        "titles": {}, "abbreviated_titles": [], "slug": null, "synopsis": null,
        "start_date": "2000-05-25", "end_date": null, "episode_count": 7,
        "average_rating": null, "subtype": "OVA", "status": "finished",
        "age_rating": null, "popularity_rank": null,
        "poster_image": { "tiny": null, "small": null,
            "medium": "https://media.kitsu.app/anime/poster_images/7730/medium.jpg",
            "large": null, "original": null },
        "cover_image": null
    });
    meta_cache_put(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key("7730"),
        &detail.to_string(),
        3600,
    )
    .expect("seed detail");

    let router = crate::api::build_api_router(state);
    let r = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/kitsu/franchise/7730")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(r.status(), StatusCode::OK);
    let body = axum::body::to_bytes(r.into_body(), 256 * 1024)
        .await
        .expect("body");
    let got: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(
        got,
        json!({
            "anchor": "7730",
            "entries": [old, ova, tv],
            "links": [["7158", "7730"], ["7730", "610"]],
            "versions": [sc],
            "films": [],
            "sides": [],
            "recaps": [],
            "parents": {},
            "complete": true
        })
    );
}
