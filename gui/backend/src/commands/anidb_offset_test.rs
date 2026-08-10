use super::*;

fn make_state() -> crate::app::AppState {
    crate::app::AppState {
        allanime_base: None,
        anidb_base: None,
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 0),
        ani_cli_path: std::path::PathBuf::from("/x/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        botan_shim_bin: None,
        history_path: std::path::PathBuf::from("/tmp/ani-gui-test-hsts"),
        scraper_gate: std::sync::Arc::new(crate::scraper::gate::ScraperGate::new()),
        image_cache_dir: std::path::PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
        config_path: std::path::PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: std::path::PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

#[test]
fn offsets_survive_the_metadata_cache_clear() {
    // History rows live indefinitely, while meta_cache rows expire
    // and the diagnostics clear wipes them all at once. An offset
    // that dies with either leaves a provider-numbered history row
    // reading as "episode 41" on the home rail — and Resume
    // targeting a nonexistent episode — until the show happens to be
    // resolved again. The translation is history metadata, not a
    // cache.
    let state = make_state();
    put(&state, "the-sequel-88", 40);
    crate::cache::meta_cache_clear(&state.cache_pool).expect("clear");
    assert_eq!(get(&state, "the-sequel-88"), 40);
}

#[test]
fn put_and_get_round_trip_including_updates() {
    let state = make_state();
    assert_eq!(get(&state, "unknown-1"), 0, "no stamp reads as no shift");
    put(&state, "show-1", 12);
    assert_eq!(get(&state, "show-1"), 12);
    // A re-resolve can correct a stamp; last write wins.
    put(&state, "show-1", 0);
    assert_eq!(get(&state, "show-1"), 0);
}

#[test]
fn translation_is_identity_at_offset_zero() {
    assert_eq!(provider_ep_no("5", 0), "5");
    assert_eq!(kitsu_ep_no("5", 0), "5");
}

#[test]
fn non_numeric_episodes_pass_through_both_ways() {
    assert_eq!(provider_ep_no("finale", 40), "finale");
    assert_eq!(kitsu_ep_no("finale", 40), "finale");
}

#[test]
fn numbers_at_or_below_the_offset_are_not_collapsed() {
    // A stored "3" against a stamp of 40 means the stamp doesn't
    // describe this row (stale stamp, foreign writer): serving raw
    // beats serving zero or wrapping.
    assert_eq!(kitsu_ep_no("3", 40), "3");
    assert_eq!(kitsu_ep_no("40", 40), "40");
}

proptest::proptest! {
    // The write and read boundaries are exact inverses over every
    // number a play can produce, whatever the stamped offset.
    #[test]
    fn provider_and_kitsu_translation_round_trip(
        episode in 1u32..=100_000,
        offset in 0u32..=50_000,
    ) {
        let provider = provider_ep_no(&episode.to_string(), offset);
        proptest::prop_assert_eq!(
            kitsu_ep_no(&provider, offset),
            episode.to_string()
        );
    }
}
