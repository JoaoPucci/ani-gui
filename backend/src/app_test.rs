//! Tests for `crate::app`. Extracted via `#[path]` so the module's
//! complexity stays out of `app.rs`'s CRAP count.

use super::*;

/// Boot the real `build` path against a staged environment: a
/// tempdir plays HOME + every XDG root, and the resource dir
/// carries the `bin/` the packages stage. Unix-only, because the
/// coverage gate this protects runs on Linux.
#[cfg(unix)]
#[tokio::test]
async fn build_assembles_state_from_a_staged_environment() {
    let _guard = crate::config::paths::TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let td = tempfile::tempdir().expect("tempdir");
    let resource = td.path().join("resources");
    std::fs::create_dir_all(resource.join("bin")).expect("mkdir resources/bin");

    let saved: Vec<(String, Option<String>)> = [
        "HOME",
        "XDG_CACHE_HOME",
        "XDG_CONFIG_HOME",
        "XDG_STATE_HOME",
        "XDG_DATA_HOME",
    ]
    .into_iter()
    .map(|k| (k.to_string(), std::env::var(k).ok()))
    .collect();
    std::env::set_var("HOME", td.path());
    std::env::set_var("XDG_CACHE_HOME", td.path().join("cache"));
    std::env::set_var("XDG_CONFIG_HOME", td.path().join("config"));
    std::env::set_var("XDG_STATE_HOME", td.path().join("state"));
    std::env::set_var("XDG_DATA_HOME", td.path().join("data"));

    let built = AppState::build(
        reqwest::Client::new(),
        ProxyOrigin::new("127.0.0.1", 1),
        Some(resource),
    );

    // Restore before asserting so a failure can't leak the fake
    // env into whichever env-locked test runs next.
    for (k, v) in saved {
        match v {
            Some(v) => std::env::set_var(&k, v),
            None => std::env::remove_var(&k),
        }
    }

    let state = built.expect("build succeeds against the staged env");
    let root = td.path().to_path_buf();
    assert!(state.history_path.starts_with(&root));
    assert!(state.image_cache_dir.starts_with(&root));
    assert!(state.config_path.starts_with(&root));
    assert!(state.state_dir.starts_with(&root));
    assert!(state.image_cache_dir.is_dir(), "image cache dir created");
    assert!(state.bundled_bin.is_some(), "resource bin dir picked up");

    // Boot sweeps the script copy an earlier version would have
    // left in the cache root. This staged env never had one, so
    // the report is empty rather than absent.
    assert!(
        state.legacy_sweep.removed.is_empty(),
        "nothing to sweep in a freshly staged cache"
    );
}

fn fake_state() -> AppState {
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui/history"),
        anidb_base: None,
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::new(reqwest::Client::new()),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: MalRefreshState::new(),
        account_write_locks: AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

#[test]
fn proxy_state_view_shares_session_table_with_app_state() {
    let app = fake_state();
    let proxy = app.proxy_state();
    // Inserting via one view is visible from the other (same Arc<DashMap>).
    let id = proxy.sessions.insert(crate::proxy::StreamSession::new(
        url::Url::parse("https://example.com/m.m3u8").unwrap(),
        "https://allmanga.to",
    ));
    assert!(app.sessions.get(&id).is_some());
}

#[cfg(not(windows))]
#[test]
fn resolve_bundled_bin_returns_none_when_resource_dir_is_none() {
    // No resource dir handed in (cargo run from source on Linux,
    // dev Windows without fetch:win-deps) → no bundled dir to
    // resolve. PATH falls through unchanged at every spawn site.
    assert!(resolve_bundled_bin(None).is_none());
}

#[test]
fn resolve_bundled_bin_returns_none_when_bin_subdir_missing() {
    // Resource dir exists but the bundled deps haven't been
    // staged into <resource_dir>/bin (Linux packaging, or a
    // Windows dev build that didn't run fetch:win-deps yet).
    // Tempdir gets cleaned up at scope exit.
    let td = tempfile::tempdir().expect("tempdir");
    // td.path() exists; td.path()/bin does not.
    assert!(resolve_bundled_bin(Some(td.path())).is_none());
}

#[test]
fn resolve_bundled_bin_returns_some_when_bin_subdir_exists() {
    // Production shape: <resource_dir>/bin holds the bundled
    // tools. Helper returns Some(<resource_dir>/bin) so the
    // spawn paths can put it ahead of the caller's PATH.
    let td = tempfile::tempdir().expect("tempdir");
    let bin = td.path().join("bin");
    std::fs::create_dir(&bin).expect("mkdir bin");
    let got = resolve_bundled_bin(Some(td.path()));
    assert_eq!(got.as_deref(), Some(bin.as_path()));
}

#[cfg(all(windows, target_env = "msvc"))]
#[test]
fn the_windows_build_carries_its_own_c_runtime() {
    // A dynamically linked CRT imports vcruntime140.dll, which
    // Windows does not ship and neither does anything in the
    // package — Electron's own binaries carry no copy. On a
    // machine where no other installer has left the VC++
    // redistributable, the backend is then the one piece of the
    // install that cannot start, and the app opens to a shell
    // with nothing behind it.
    //
    // MSVC only, matching the cargo config it verifies: a GNU
    // toolchain links against msvcrt.dll, which Windows ships,
    // and keeps its default linkage.
    assert!(
        cfg!(target_feature = "crt-static"),
        "the CRT is linked dynamically; a clean Windows has no vcruntime140.dll"
    );
}
