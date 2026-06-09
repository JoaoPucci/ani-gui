//! Tests for `crate::commands::account`. Extracted via `#[path]` so
//! the dispatcher + helper complexity doesn't pile onto `account.rs`'s
//! CCN budget — per `project_crap_inline_test_gotcha`.

use super::*;
use crate::account::pkce::PkceMethod;

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
