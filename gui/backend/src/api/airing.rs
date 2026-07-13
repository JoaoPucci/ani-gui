//! Airing route — `GET /api/kitsu/airing/:kitsu_id` returns the
//! show's [`AiringStatus`] so the detail page can grey out unaired
//! episode tiles. Split from `api/mod.rs` (like `api/account.rs`) to
//! keep that file under the CRAP ratchet ceiling.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use crate::error::AniError;
use crate::meta::anilist_airing::AiringStatus;

/// Mount the airing route. Called from
/// [`crate::api::build_api_router`].
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/kitsu/airing/:kitsu_id", get(get_airing))
}

async fn get_airing(
    State(state): State<Arc<AppState>>,
    Path(kitsu_id): Path<String>,
) -> Result<Json<AiringStatus>, AniError> {
    Ok(Json(
        crate::commands::airing::airing_get(&state, &kitsu_id).await?,
    ))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::cache::meta_cache_put;

    /// Minimal AppState with an in-memory cache; the route test seeds
    /// the airing cache row so no network is involved.
    fn test_state() -> std::sync::Arc<crate::app::AppState> {
        use std::path::PathBuf;
        use std::sync::Arc;
        use tokio::sync::Semaphore;
        Arc::new(crate::app::AppState {
            secret: crate::proxy::AppSecret::random(),
            sessions: crate::proxy::SessionTable::new(),
            proxy_http: reqwest::Client::new(),
            meta_http: reqwest::Client::new(),
            proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
            ani_cli_path: PathBuf::from("/tmp/ani-cli"),
            bash_path: None,
            bundled_bin: None,
            history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
            scraper_slots: Arc::new(Semaphore::new(crate::app::SCRAPER_CONCURRENCY)),
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
        })
    }

    #[tokio::test]
    async fn airing_route_serves_the_cached_status() {
        let state = test_state();
        meta_cache_put(
            &state.cache_pool,
            "airing:v2:50551",
            r#"{"aired":2,"next_episode":3,"next_airing_at":1784215800}"#,
            3600,
        )
        .expect("seed cache");
        let router = crate::api::build_api_router(state);
        let r = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/kitsu/airing/50551")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("oneshot");
        assert_eq!(r.status(), StatusCode::OK);
        let body = axum::body::to_bytes(r.into_body(), 64 * 1024)
            .await
            .expect("body");
        let got: crate::meta::anilist_airing::AiringStatus =
            serde_json::from_slice(&body).expect("parses");
        assert_eq!(got.aired, Some(2));
        assert_eq!(got.next_episode, Some(3));
        assert_eq!(got.next_airing_at, Some(1_784_215_800));
    }
}
