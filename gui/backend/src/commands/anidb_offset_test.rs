use super::*;

fn make_state_at(history_path: std::path::PathBuf) -> crate::app::AppState {
    crate::app::AppState {
        anidb_base: None,
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path,
        anidb_gate: std::sync::Arc::new(crate::scraper::gate::ScraperGate::new()),
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
    make_state_at(dir.path().join("history"))
}

#[test]
fn display_stamps_round_trip_and_survive_offset_puts() {
    // The stamp maps the CLI-visible slot to the row's display tag.
    // An offset-only re-stamp (every fresh resolve writes one) must
    // not erase it — the last fractional watch stays translatable.
    let state = make_state();
    put_display(&state, "the-show-77", 0, 4, "3.5");
    assert_eq!(get(&state, "the-show-77"), 0);
    assert_eq!(
        display_stamp(&state, "the-show-77"),
        Some((4, "3.5".to_string()))
    );
    put(&state, "the-show-77", 0);
    assert_eq!(
        display_stamp(&state, "the-show-77"),
        Some((4, "3.5".to_string())),
        "an offset put must preserve the display stamp"
    );
    assert_eq!(display_stamp(&state, "unstamped"), None);
}

#[test]
fn writes_map_a_stamped_display_back_to_its_slot() {
    // The mark-watched and cache-hit writers have no listing at
    // hand: when the requested episode names the stamped display
    // tag — by numeric identity, since the frontend normalizes
    // "3.50" to "3.5" — the shared file gets the slot the CLI can
    // grep. Everything else keeps the offset translation.
    let state = make_state();
    put_display(&state, "the-show-77", 0, 4, "3.50");
    assert_eq!(write_ep_no(&state, "the-show-77", "3.5", 0), "4");
    assert_eq!(write_ep_no(&state, "the-show-77", "2", 0), "2");
    let cont = make_state();
    put_display(&cont, "the-sequel-88", 40, 2, "41.5");
    assert_eq!(write_ep_no(&cont, "the-sequel-88", "1.5", 40), "2");
    assert_eq!(write_ep_no(&cont, "the-sequel-88", "1", 40), "41");
}

#[test]
fn reads_present_the_stamped_display_per_entry() {
    // The read boundary turns the CLI-visible slot back into the
    // per-entry display the GUI counts in: "4" with stamp (4, "3.5")
    // reads as 3.5; a tagged continuation's slot 2 under (2, "41.5")
    // at offset 40 reads as 1.5; rows not matching the stamp keep
    // the plain offset translation.
    let state = make_state();
    put_display(&state, "the-show-77", 0, 4, "3.5");
    assert_eq!(read_ep_no(&state, "the-show-77", "4"), "3.5");
    assert_eq!(read_ep_no(&state, "the-show-77", "2"), "2");
    let cont = make_state();
    put_display(&cont, "the-sequel-88", 40, 2, "41.5");
    assert_eq!(read_ep_no(&cont, "the-sequel-88", "2"), "1.5");
    assert_eq!(read_ep_no(&cont, "the-sequel-88", "41"), "1");
}

#[test]
fn offsets_follow_the_history_file_they_translate() {
    // Why the store is a file beside the history rather than a cache
    // row: it has to reach whoever reads that history, and last as
    // long as the rows do. An offset kept in the cache database is
    // scoped to the cache instead — a different profile's database, or
    // a cleared one — and a row it cannot reach shows the provider's
    // episode 41 where the user expects 1.
    //
    // Two states are given the same history path here, which is the
    // whole property: the store is keyed to that file and nothing
    // else. Not a claim about profiles — `paths::app_name` puts a
    // debug build under `ani-gui-dev`, so a source build and a
    // packaged one have different history files and share nothing by
    // default. What makes them meet is being pointed at one file,
    // which `ANI_GUI_DEV` does across builds and two instances of one
    // build do by existing.
    let tmp = tempfile::tempdir().expect("tmp");
    let history = tmp.path().join("history");
    let packaged = make_state_at(history.clone());
    let dev = make_state_at(history);
    put(&packaged, "the-sequel-88", 40);
    assert_eq!(get(&dev, "the-sequel-88"), 40);
}

#[test]
fn puts_create_the_store_directory_on_a_fresh_profile() {
    // On a fresh profile the state dir does not exist yet:
    // the first continuation-cour resolve reaches the offset write
    // BEFORE the history writer creates the directory, so a put that
    // assumes the parent silently loses the stamp — and the cached
    // resolution keeps the loss alive for the row's whole TTL.
    let tmp = tempfile::tempdir().expect("tmp");
    let state = make_state_at(tmp.path().join("ani-gui").join("history"));
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
    let history = tmp.path().join("history");
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
fn puts_exclude_each_other_across_processes_via_the_file_lock() {
    // The store is shared by the packaged and dev profiles by design
    // (the cross-profile test above) — two app INSTANCES, not two
    // threads, can resolve different shows at once. The in-process
    // mutex cannot serialize those: both processes read the same old
    // rows, write through the same temp path, and one instance's
    // stamp vanishes in the other's rename. The writer therefore
    // holds an OS file lock — released by the kernel even if its
    // holder crashes — across the whole read-merge-rename. This test
    // plays the second process by taking that lock on its own
    // handle: the put must wait for it.
    let tmp = tempfile::tempdir().expect("tmp");
    let history = tmp.path().join("history");
    let state = make_state_at(history.clone());
    let lock_path = history.with_file_name("ani-gui-offsets.lock");
    let foreign = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .expect("lock file");
    fs4::FileExt::lock(&foreign).expect("foreign lock");
    let (tx, rx) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        put(&state, "the-sequel-88", 40);
        tx.send(()).ok();
    });
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(300))
            .is_err(),
        "put finished while the offsets file lock was held elsewhere"
    );
    fs4::FileExt::unlock(&foreign).expect("unlock");
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .expect("put never finished after the lock was released");
    writer.join().expect("writer thread");
    assert_eq!(get(&make_state_at(history), "the-sequel-88"), 40);
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
fn fractional_episodes_translate_like_integers() {
    // The native resolve translates a clicked per-entry "1.5" to
    // provider tag "41.5"; the history writer must speak the same
    // provider numbering or the script's process_hist_entry, which
    // searches the provider list for the stored tag exactly, no
    // longer recognizes the shared row. The read boundary maps it
    // back for the GUI.
    assert_eq!(provider_ep_no("1.5", 40), "41.5");
    assert_eq!(kitsu_ep_no("41.5", 40), "1.5");
    // At or below the offset the stamp doesn't describe this row —
    // same pass-through rule as integers.
    assert_eq!(kitsu_ep_no("3.5", 40), "3.5");
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

    /// Fractional tags round-trip the same way the integers do.
    #[test]
    fn fractional_translation_round_trips(
        episode in 1u32..=100_000,
        frac in 0u32..10,
        offset in 0u32..=50_000,
    ) {
        let per_entry = format!("{episode}.{frac}");
        let provider = provider_ep_no(&per_entry, offset);
        proptest::prop_assert_eq!(kitsu_ep_no(&provider, offset), per_entry);
    }
}
