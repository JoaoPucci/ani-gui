//! A tracker's 429 reaches the editor's seed read as a rate limit with
//! its wait. Extracted via `#[path]` beside `account_edit_test.rs` so
//! the fixtures stay off the command file's CRAP budget.
//!
//! The renderer retries a failed read; what it does next depends on
//! the envelope. `Upstream { status: 429 }` answers as HTTP 429 under
//! `error.network.upstream` with no wait, and the renderer's schedule
//! runs — five attempts in fifteen seconds against a budget that
//! resets per minute. `RateLimited { retry_after_secs }` answers under
//! `error.network.rate_limited` with the tracker's own wait.

use std::path::PathBuf;
use std::sync::Arc;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::get_entry_via;
use crate::account::provider::{ProviderKind, Tokens};
use crate::app::AppState;
use crate::error::AniError;
use crate::meta::anilist_user::AniListProvider;
use crate::meta::mal_user::{MalProvider, MalRefreshState};

fn state_with_kitsu(kitsu_uri: &str) -> Arc<AppState> {
    Arc::new(AppState {
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
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

fn tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

/// Kitsu `/anime/12` mapped to MAL, so a MAL account needs no AniList
/// translation and the one request is the tracker's own.
const KITSU_MAL: &str = r#"{
    "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "One Piece" } },
    "included": [
        { "id": "1", "type": "mappings",
          "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" } }
    ]
}"#;

/// Kitsu `/anime/12` mapped to AniList alone. An AniList account
/// resolves a MAL mapping through AniList's public API first — a
/// request to graphql.anilist.co, which this case does not stand up —
/// so the show carries only the direct mapping, and the one request is
/// again the tracker's own.
const KITSU_ANILIST: &str = r#"{
    "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "One Piece" } },
    "included": [
        { "id": "2", "type": "mappings",
          "attributes": { "externalSite": "anilist/anime", "externalId": "21" } }
    ]
}"#;

async fn kitsu(body: &'static str) -> MockServer {
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&kitsu)
        .await;
    kitsu
}

/// A tracker answering everything with 429 and a sixty-second wait —
/// what AniList sends once the minute's budget is spent.
async fn rate_limiting_tracker() -> MockServer {
    let tracker = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "60"))
        .expect(1)
        .mount(&tracker)
        .await;
    tracker
}

fn assert_rate_limited_for(err: &AniError, wait: u64) {
    assert!(
        matches!(err, AniError::RateLimited { retry_after_secs: Some(w), .. } if *w == wait),
        "expected RateLimited {{ retry_after_secs: Some({wait}) }}, got {err:?}"
    );
}

#[tokio::test]
async fn a_mal_429_reaches_the_seed_read_as_a_rate_limit_with_its_wait() {
    let kitsu = kitsu(KITSU_MAL).await;
    let mal = rate_limiting_tracker().await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = MalProvider::with_bases(
        reqwest::Client::new(),
        mal.uri(),
        "http://unused-token".to_string(),
        MalRefreshState::new(),
    );

    let err = get_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &provider,
        &tokens(),
        "12",
    )
    .await
    .expect_err("the tracker refused");
    assert_rate_limited_for(&err, 60);
    // `.expect(1)`: nothing in the backend retries the tracker.
}

#[tokio::test]
async fn an_anilist_429_reaches_the_seed_read_as_a_rate_limit_with_its_wait() {
    let kitsu = kitsu(KITSU_ANILIST).await;
    let anilist = rate_limiting_tracker().await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = AniListProvider::with_bases(
        reqwest::Client::new(),
        anilist.uri(),
        "http://unused-token".to_string(),
    );

    let err = get_entry_via(&state, ProviderKind::AniList, &provider, &tokens(), "12")
        .await
        .expect_err("the tracker refused");
    assert_rate_limited_for(&err, 60);
}
