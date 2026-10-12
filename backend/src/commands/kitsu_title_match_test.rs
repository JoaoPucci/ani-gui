//! The title-match cache under two providers.

use super::*;
use crate::app::AppState;
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use crate::scraper::provider::ProviderId;
use std::path::PathBuf;

fn state() -> AppState {
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui-history-never-read"),
        providers: crate::provider_set::ProviderSet::for_tests(),
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
    title_match_put(&s, Some(ProviderId::TestA), "Stone Ocean", 1, "id-a").expect("put");
    title_match_put(
        &s,
        Some(ProviderId::Hianime),
        "Stone Ocean",
        1,
        "id-hianime",
    )
    .expect("put");
    assert_eq!(
        title_match_get(&s, Some(ProviderId::TestA), "stone ocean", 1)
            .expect("get")
            .as_deref(),
        Some("id-a")
    );
    assert_eq!(
        title_match_get(&s, Some(ProviderId::Hianime), "stone ocean", 1)
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
        title_match_get(&s, Some(ProviderId::TestA), "Stone Ocean", 1).expect("get"),
        None
    );
}

/// A history row whose key names no provider — anidb.app's bare slug —
/// keeps its own title matches: stored and read under the legacy
/// segment, apart from any provider's, and only while a row searches
/// the title. Continue Watching asks for such a row without a
/// provider.
#[test]
fn a_bare_rows_title_match_lives_under_the_legacy_segment() {
    let td = tempfile::tempdir().expect("td");
    let mut s = state();
    s.history_path = td.path().join("history");
    crate::history::write_atomic(
        &s.history_path,
        &[crate::history::HistoryEntry {
            ep_no: "3".into(),
            id: "stone-ocean-41".into(),
            title: "Stone Ocean".into(),
            watched_at: None,
            kitsu_id: None,
        }],
    )
    .expect("history");
    crate::commands::title_match_store::store_title_match(&s, None, "Stone Ocean", 1, "12")
        .expect("store");
    assert_eq!(
        title_match_get(&s, None, "stone ocean", 1)
            .expect("get")
            .as_deref(),
        Some("12"),
        "the row with no provider searches the title, so its match is kept"
    );
    assert!(
        crate::cache::meta_cache_get(&s.cache_pool, "title-match:v3:legacy:stone ocean:c1")
            .expect("read")
            .is_some()
    );
    assert_eq!(
        title_match_get(&s, Some(ProviderId::Hianime), "stone ocean", 1).expect("get"),
        None,
        "and no provider's title reads it"
    );
}
