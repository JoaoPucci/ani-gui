//! Tests for `crate::commands::account`. Extracted via `#[path]` so
//! the dispatcher + helper complexity doesn't pile onto `account.rs`'s
//! CCN budget — per `project_crap_inline_test_gotcha`.

use super::*;
use crate::account::pkce::PkceMethod;

/// Build an `AppState` whose Kitsu client points at `kitsu_uri` (a
/// wiremock server) so id-resolution tests can mock the mappings
/// endpoint. Everything else is throwaway.
#[cfg(test)]
fn state_with_kitsu(kitsu_uri: &str) -> std::sync::Arc<crate::app::AppState> {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    Arc::new(crate::app::AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(crate::app::SCRAPER_CONCURRENCY)),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
    })
}

/// Kitsu `/anime/:id?include=mappings` body carrying a MAL mapping.
#[cfg(test)]
const KITSU_MAL_MAPPING_BODY: &str = r#"{
    "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "One Piece" } },
    "included": [{
        "id": "1175",
        "type": "mappings",
        "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" }
    }]
}"#;

#[tokio::test]
async fn resolve_native_media_id_mal_is_the_mapped_mal_id() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::MyAnimeList, "12", None)
        .await
        .expect("resolve ok");
    assert_eq!(got, Some(ProviderMediaId(21)));
}

#[tokio::test]
async fn resolve_native_media_id_anilist_bridges_mal_to_media_id() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Media":{"id":154587}}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::AniList, "12", Some(&anilist.uri()))
        .await
        .expect("resolve ok");
    assert_eq!(got, Some(ProviderMediaId(154587)));
}

#[tokio::test]
async fn resolve_native_media_id_none_when_kitsu_has_no_mal_mapping() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/999"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"999","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::MyAnimeList, "999", None)
        .await
        .expect("resolve ok");
    assert_eq!(got, None);
}

#[tokio::test]
async fn resolve_native_media_id_none_when_anilist_unmapped() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::AniList, "12", Some(&anilist.uri()))
        .await
        .expect("resolve ok");
    assert_eq!(got, None);
}

#[tokio::test]
async fn push_progress_skips_unmappable_show_without_writing() {
    // A show Kitsu can't map to MAL → resolve yields None → push_progress
    // returns Ok(None) and never reaches update_entry. The provider
    // built by the dispatcher hits the real MAL host, so a stray write
    // attempt would surface as Network/Upstream, not Ok(None) — proving
    // the short-circuit fires before any upstream call.
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/999"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"999","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let tokens = Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let got = push_progress(
        &state,
        ProviderKind::MyAnimeList,
        &tokens,
        "999",
        crate::account::provider::EntryUpdate {
            progress_episodes: Some(5),
            ..Default::default()
        },
    )
    .await
    .expect("push ok");
    assert!(got.is_none(), "unmappable show must short-circuit to None");
}

#[test]
fn build_entry_update_rejects_empty_and_unknown_status() {
    // Codex P2 #3381617932: an all-absent update, or a status typo
    // that silently parses to None, would still call update_entry —
    // and since both providers upsert, that creates a list row with
    // upstream defaults. Reject both so a malformed fan-out request
    // is a no-op error, not a phantom "watching" entry.
    assert!(
        build_entry_update(None, None, None).is_err(),
        "all-absent update must be rejected"
    );
    assert!(
        build_entry_update(Some("not_a_status"), Some(5), None).is_err(),
        "unrecognized status must be rejected, not dropped to None"
    );
    let ok = build_entry_update(Some("watching"), Some(5), None).expect("valid update");
    assert_eq!(ok.status, Some(ListStatus::Watching));
    assert_eq!(ok.progress_episodes, Some(5));
    // Progress-only (no status) is a legitimate update.
    let progress_only = build_entry_update(None, Some(7), None).expect("progress-only ok");
    assert!(progress_only.status.is_none());
    assert_eq!(progress_only.progress_episodes, Some(7));
}

#[test]
fn provider_for_kind_dispatches_anilist_and_mal_but_not_inhouse() {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    let state = Arc::new(crate::app::AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(crate::app::SCRAPER_CONCURRENCY)),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
    });
    assert!(provider_for_kind(&state, ProviderKind::AniList).is_some());
    assert!(provider_for_kind(&state, ProviderKind::MyAnimeList).is_some());
    assert!(provider_for_kind(&state, ProviderKind::InHouse).is_none());
}

#[test]
fn pkce_for_kind_picks_method_per_provider() {
    assert_eq!(
        pkce_for_kind(ProviderKind::AniList).method,
        PkceMethod::S256
    );
    assert_eq!(
        pkce_for_kind(ProviderKind::MyAnimeList).method,
        PkceMethod::Plain
    );
    assert_eq!(
        pkce_for_kind(ProviderKind::InHouse).method,
        PkceMethod::S256
    );
}

#[test]
fn status_snake_round_trips_every_variant() {
    for s in [
        ListStatus::Planning,
        ListStatus::Watching,
        ListStatus::Completed,
        ListStatus::Paused,
        ListStatus::Dropped,
        ListStatus::Rewatching,
    ] {
        assert_eq!(status_from_snake(status_to_snake(s)), Some(s));
    }
}

#[test]
fn status_from_snake_returns_none_for_unknown() {
    assert_eq!(status_from_snake(""), None);
    assert_eq!(status_from_snake("Planning"), None);
    assert_eq!(status_from_snake("plan_to_watch"), None);
}

#[test]
fn tokens_from_bearer_drops_expiry_and_refresh() {
    let t = tokens_from_bearer("xyz");
    assert_eq!(t.access_token, "xyz");
    assert!(t.refresh_token.is_none());
    assert_eq!(t.expires_at_epoch_s, 0);
}

#[test]
fn watch_later_bridge_max_ids_is_a_sane_ceiling() {
    // Codex P1 #3373789621: the route gates on this constant.
    // 500 covers the largest plausible Plan-to-Watch (a heavy
    // listmaker with curated picks tops out around 200-300) and
    // bounds per-request fan-out cost. Pinned here so a future
    // bump is intentional, not a typo.
    assert_eq!(WATCH_LATER_BRIDGE_MAX_IDS, 500);
}
