use super::*;

fn make_state_at(history_path: std::path::PathBuf) -> crate::app::AppState {
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
        history_path,
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

fn make_state() -> crate::app::AppState {
    let tmp = tempfile::tempdir().expect("tmp");
    // Leak the tempdir so the per-test history path stays alive; the
    // OS reclaims it after the test process exits.
    let dir = Box::leak(Box::new(tmp));
    make_state_at(dir.path().join("ani-hsts"))
}

#[test]
fn offsets_are_shared_across_profiles_like_the_history_they_translate() {
    // config/paths.rs gives debug and packaged builds separate cache
    // databases while ani-hsts stays shared. An offset persisted in
    // the profile-local cache is invisible to the other profile: the
    // packaged app writes provider episode 41 with offset 40, the
    // source-built GUI reads the same shared history row, finds no
    // stamp, and shows episode 41 — and deleting the XDG cache has
    // the same effect. The translation lives beside the history file
    // it makes readable.
    let tmp = tempfile::tempdir().expect("tmp");
    let history = tmp.path().join("ani-hsts");
    let packaged = make_state_at(history.clone());
    let dev = make_state_at(history);
    put(&packaged, "the-sequel-88", 40);
    assert_eq!(get(&dev, "the-sequel-88"), 40);
}

#[test]
fn puts_create_the_store_directory_on_a_fresh_profile() {
    // On a fresh profile $XDG_STATE_HOME/ani-cli does not exist yet:
    // the first continuation-cour resolve reaches the offset write
    // BEFORE the history writer creates the directory, so a put that
    // assumes the parent silently loses the stamp — and the cached
    // resolution keeps the loss alive for the row's whole TTL.
    let tmp = tempfile::tempdir().expect("tmp");
    let state = make_state_at(tmp.path().join("ani-cli").join("ani-hsts"));
    put(&state, "the-sequel-88", 40);
    assert_eq!(get(&state, "the-sequel-88"), 40);
}

#[test]
fn concurrent_puts_keep_every_stamp() {
    // Page prefetches resolve several shows at once; unserialized
    // read-merge-write lets one put overwrite another's row (or
    // consume its temp file), and the lost show's history starts
    // exposing provider numbering. The whole sequence holds a lock.
    let tmp = tempfile::tempdir().expect("tmp");
    let history = tmp.path().join("ani-hsts");
    let state = std::sync::Arc::new(make_state_at(history));
    let mut handles = Vec::new();
    for t in 0..16u32 {
        let state = state.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..4u32 {
                put(&state, &format!("show-{t}-{i}"), t * 10 + i);
            }
        }));
    }
    for h in handles {
        h.join().expect("writer thread");
    }
    for t in 0..16u32 {
        for i in 0..4u32 {
            assert_eq!(
                get(&state, &format!("show-{t}-{i}")),
                t * 10 + i,
                "a concurrent put lost show-{t}-{i}"
            );
        }
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
