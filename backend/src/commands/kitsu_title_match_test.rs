//! The title-match cache under two providers.

use super::*;
use crate::app::AppState;
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use crate::scraper::provider::ProviderId;
use std::path::PathBuf;
use std::sync::Arc;

fn state() -> AppState {
    AppState {
        anidb_base: None,
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui-history-never-read"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// Two providers naming different shows identically must not read or
/// overwrite each other's mapping.
#[test]
fn the_same_title_maps_per_provider() {
    let s = state();
    title_match_put(&s, ProviderId::Anidb, "Stone Ocean", 1, "id-anidb").expect("put");
    title_match_put(&s, ProviderId::Hianime, "Stone Ocean", 1, "id-hianime").expect("put");
    assert_eq!(
        title_match_get(&s, ProviderId::Anidb, "stone ocean", 1)
            .expect("get")
            .as_deref(),
        Some("id-anidb")
    );
    assert_eq!(
        title_match_get(&s, ProviderId::Hianime, "stone ocean", 1)
            .expect("get")
            .as_deref(),
        Some("id-hianime")
    );
}

#[test]
fn a_row_from_the_provider_blind_schema_is_a_miss() {
    // v2 rows carried no provider; the bump orphans them rather than
    // letting one provider's mapping answer for another.
    let s = state();
    crate::cache::meta_cache_put(
        &s.cache_pool,
        "title-match:v2:stone ocean:c1",
        "id-old",
        3600,
    )
    .expect("put");
    assert_eq!(
        title_match_get(&s, ProviderId::Anidb, "Stone Ocean", 1).expect("get"),
        None
    );
}
