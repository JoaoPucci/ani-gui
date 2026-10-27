//! The boot sweep over a history and cache on disk. The tests walk
//! `test-a` alone, so a `hianime:` key is as retired here as a bare
//! one: the rule names no provider.

use super::*;
use crate::app::AppState;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::history::{read_all, write_atomic, HistoryEntry};
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::path::{Path, PathBuf};

const DAY: u64 = 86_400;

fn state_in(dir: &Path) -> AppState {
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: dir.join("history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
        config_path: dir.join("config.toml"),
        state_dir: dir.to_path_buf(),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

fn entry(ep: &str, id: &str, title: &str, at: Option<i64>, kitsu: Option<&str>) -> HistoryEntry {
    HistoryEntry {
        ep_no: ep.into(),
        id: id.into(),
        title: title.into(),
        watched_at: at,
        kitsu_id: kitsu.map(str::to_string),
        own_number: false,
    }
}

fn put(state: &AppState, key: &str, body: &str) {
    meta_cache_put(&state.cache_pool, key, body, 30 * DAY).expect("put");
}

/// Store a row as long expired: written a year ago for a day.
fn put_expired(state: &AppState, key: &str, body: &str) {
    put(state, key, body);
    let conn = state.cache_pool.get().expect("conn");
    conn.execute(
        "UPDATE meta_cache SET fetched_at = fetched_at - 31536000, ttl_seconds = 86400 WHERE key = ?1",
        [key],
    )
    .expect("age");
}

fn present(state: &AppState, key: &str) -> bool {
    let conn = state.cache_pool.get().expect("conn");
    conn.query_row(
        "SELECT count(*) FROM meta_cache WHERE key = ?1",
        [key],
        |r| r.get::<_, i64>(0),
    )
    .expect("count")
        == 1
}

fn offsets(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("ani-gui-offsets")).unwrap_or_default()
}

fn title_match_key(provider: Option<ProviderId>, title: &str) -> String {
    format!(
        "{}1",
        crate::commands::kitsu::title_match_prefix(provider, title)
    )
}

/// A retired row superseded by a walked one, a retired row with no
/// other row for its show (a continuation the retired provider listed
/// at slot 21 with an offset of 13), and the walked row.
fn seed_history(state: &AppState, dir: &Path) {
    write_atomic(
        &state.history_path,
        &[
            entry(
                "388",
                "naruto-shippuden-3687",
                "Naruto Shippuden",
                None,
                None,
            ),
            entry("21", "the-separation-677", "The Separation", None, None),
            entry(
                "425",
                "test-a:naruto-shippuden-600",
                "Naruto: Shippuden",
                Some(1_792_499_281_363),
                Some("1555"),
            ),
        ],
    )
    .expect("history");
    std::fs::write(
        dir.join("ani-gui-offsets"),
        "naruto-shippuden-3687\t0\nthe-separation-677\t13\nfrieren-1663\t0\ntest-a:one-piece-1\t0\n",
    )
    .expect("offsets");
    put(state, &title_match_key(None, "Naruto Shippuden"), "1555");
    put(state, &title_match_key(None, "The Separation"), "46903");
    put_expired(
        state,
        "watched-at:v1:naruto-shippuden-3687",
        "1788436545407",
    );
    put_expired(state, "allmanga2kitsu:v3:naruto-shippuden-3687", "1555");
}

#[test]
fn a_superseded_retired_row_goes_with_what_is_kept_about_it() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_in(dir.path());
    seed_history(&state, dir.path());

    let report = sweep(&state);

    let ids: Vec<String> = read_all(&state.history_path)
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, ["the-separation-677", "test-a:naruto-shippuden-600"]);
    assert_eq!(report.superseded, ["Naruto Shippuden"]);
    assert!(!present(&state, "watched-at:v1:naruto-shippuden-3687"));
    assert!(!present(&state, "allmanga2kitsu:v3:naruto-shippuden-3687"));
    assert!(!offsets(dir.path()).contains("naruto-shippuden-3687"));
}

#[test]
fn a_kept_retired_row_reads_the_same_episode_in_the_apps_own_numbering() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_in(dir.path());
    seed_history(&state, dir.path());
    let shown = |state: &AppState| {
        crate::commands::history::history_list(state)
            .unwrap()
            .into_iter()
            .find(|e| e.id == "the-separation-677")
            .map(|e| e.ep_no)
    };
    assert_eq!(shown(&state).as_deref(), Some("8"));

    let report = sweep(&state);

    assert_eq!(shown(&state).as_deref(), Some("8"));
    let row = read_all(&state.history_path)
        .unwrap()
        .into_iter()
        .find(|e| e.id == "the-separation-677")
        .expect("kept");
    assert!(row.own_number);
    assert_eq!(
        row.kitsu_id, None,
        "a title match is a guess, never the row's id"
    );
    assert_eq!(report.renumbered, ["The Separation"]);
    assert!(!offsets(dir.path()).contains("the-separation-677"));
}

#[test]
fn numbering_of_a_retired_key_with_no_row_goes_and_a_walked_keys_stays() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_in(dir.path());
    seed_history(&state, dir.path());

    let report = sweep(&state);

    let store = offsets(dir.path());
    assert!(!store.contains("frieren-1663"), "{store}");
    assert!(store.contains("test-a:one-piece-1"), "{store}");
    assert_eq!(report.numbering_rows, 3);
}

#[test]
fn cache_rows_only_a_retired_provider_could_be_read_by_go() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_in(dir.path());
    seed_history(&state, dir.path());
    let retired_play = "play:v12:Naruto: Shippuuden:sub:best:366:2007:500:TV";
    let walked_play = "play:v25:Naruto: Shippuuden:sub:best:425:2007:500:TV";
    put_expired(
        &state,
        retired_play,
        r#"{"upstream_url":"https://hls.example/m.m3u8","show_id":"naruto-shippuden-3687"}"#,
    );
    put(
        &state,
        walked_play,
        r#"{"upstream_url":"https://cdn.example/m.m3u8","show_id":"test-a:naruto-shippuden-600"}"#,
    );
    put(
        &state,
        "availability:v24:1:sub",
        r#"{"available":true,"provider":"hianime"}"#,
    );
    put(&state, "availability:v24:2:sub", r#"{"available":true}"#);
    put(
        &state,
        "availability:v24:3:sub",
        r#"{"available":true,"provider":"testa"}"#,
    );
    let retired_title = title_match_key(Some(ProviderId::Hianime), "Naruto: Shippuden");
    let walked_title = title_match_key(Some(ProviderId::TestA), "Naruto: Shippuden");
    put(&state, &retired_title, "1555");
    put(&state, &walked_title, "1555");
    put(
        &state,
        "title-match:v3:anidb.app:naruto shippuden:c1",
        "1555",
    );
    put(&state, "watched-at:v1:hianime:gone-1", "1");
    put(&state, "allmanga2kitsu:v8:hianime:gone-1", "7");
    put(&state, "allmanga2kitsu:played:v1:hianime:gone-1", "7");

    let report = sweep(&state);

    for gone in [
        retired_play,
        "availability:v24:1:sub",
        "availability:v24:2:sub",
        retired_title.as_str(),
        "title-match:v3:anidb.app:naruto shippuden:c1",
        "watched-at:v1:hianime:gone-1",
        "allmanga2kitsu:v8:hianime:gone-1",
        "allmanga2kitsu:played:v1:hianime:gone-1",
    ] {
        assert!(!present(&state, gone), "{gone} stayed");
    }
    for kept in [walked_play, "availability:v24:3:sub", walked_title.as_str()] {
        assert!(present(&state, kept), "{kept} went");
    }
    // The kept retired row's title match is what Continue Watching
    // shows it by.
    assert_eq!(
        meta_cache_get(&state.cache_pool, &title_match_key(None, "The Separation")).unwrap(),
        Some("46903".to_string())
    );
    assert!(report.cache_rows >= 8, "{report:?}");
}

#[test]
fn a_second_sweep_finds_nothing_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_in(dir.path());
    seed_history(&state, dir.path());
    let _ = sweep(&state);
    let history = std::fs::read(&state.history_path).unwrap();
    let store = offsets(dir.path());

    let report = sweep(&state);

    assert_eq!(report, RetiredSweepReport::default());
    assert_eq!(std::fs::read(&state.history_path).unwrap(), history);
    assert_eq!(offsets(dir.path()), store);
}

#[test]
fn a_history_with_nothing_retired_is_left_as_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let state = state_in(dir.path());
    write_atomic(
        &state.history_path,
        &[entry(
            "3",
            "test-a:show-1",
            "Show",
            Some(1_792_499_281_363),
            Some("9"),
        )],
    )
    .unwrap();

    assert_eq!(sweep(&state), RetiredSweepReport::default());
    assert!(!dir.path().join("ani-gui-offsets").exists());
}
