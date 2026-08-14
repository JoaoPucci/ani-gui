//! `AppState` — the single state value Tauri hands to every command.
//!
//! Wires together everything the frontend can reach:
//!
//! - the streaming proxy (its session table, app secret, http client,
//!   origin, and the kernel-assigned base URL once the listener is up)
//! - the directory of bundled binaries native resolution spawns
//! - the path of the GUI's own watch-history file
//! - an admission gate for provider traffic so background probes
//!   never hammer the upstream
//!
//! Built once during `tauri::Builder::setup` and stored as managed state.

use std::path::PathBuf;
use std::sync::Arc;

use crate::account::InternalSecret;
use crate::cache::SqlitePool;
use crate::commands::account::AccountWriteLocks;
use crate::config::paths;
use crate::error::{AniError, Result};
use crate::meta::kitsu::KitsuClient;
use crate::meta::mal_user::MalRefreshState;
use crate::proxy::{AppSecret, ProxyOrigin, ProxyState, SessionTable};

/// Single state container Tauri hands to every command.
#[derive(Clone)]
pub struct AppState {
    /// HMAC secret for stream tokens.
    pub secret: AppSecret,
    /// Live session table (shared with the proxy server).
    pub sessions: SessionTable,
    /// Outbound http client used by the proxy.
    pub proxy_http: reqwest::Client,
    /// Outbound HTTP client for metadata calls (Kitsu, AniList,
    /// images, GitHub release polls). Separate from
    /// `proxy_http` so these calls carry tight timeouts: the proxy
    /// client's 120s ceiling is sized for streaming bodies, and a
    /// stalled metadata connection could hold a probe handler for two
    /// minutes. Same User-Agent as the proxy client — CDN HEAD probes
    /// (`upstream_head_ok`) rely on the client default.
    pub meta_http: reqwest::Client,
    /// Public base URL the frontend uses to reach the proxy
    /// (`http://127.0.0.1:<port>`). Set after the listener binds.
    pub proxy_origin: ProxyOrigin,
    /// Directory the packages stage next to the backend binary,
    /// holding the impersonating transport and the download tools.
    /// Computed once in `build()` from the resource dir; searched
    /// ahead of PATH so a bundled binary beats a system install.
    /// `None` on a dev run that hasn't staged the directory.
    pub bundled_bin: Option<PathBuf>,
    /// What the boot-time sweep removed of the script copy earlier
    /// versions maintained. Empty on every launch after the first, and
    /// on installs that never ran one of those versions. Surfaced by
    /// the diagnostics page so the removal is visible rather than
    /// silent.
    pub legacy_sweep: crate::legacy_script::SweepReport,
    /// Path of the shared history file.
    pub history_path: PathBuf,
    /// Test override for the anidb provider origin the native play
    /// resolution scrapes. `None` in production (the real site).
    pub anidb_base: Option<String>,
    /// Admission gate for provider traffic: paces background probes
    /// and breaks the circuit on consecutive failures so cold caches
    /// can't rate-limit the IP out from under a user's click.
    pub anidb_gate: Arc<crate::scraper::gate::ScraperGate>,
    /// On-disk image-cache directory served by the `image://` protocol.
    pub image_cache_dir: PathBuf,
    /// Connection pool for the SQLite metadata cache.
    pub cache_pool: SqlitePool,
    /// Kitsu metadata client (shares the same reqwest pool as the proxy).
    pub kitsu: KitsuClient,
    /// Path to the user's TOML settings file (`config.toml`).
    pub config_path: PathBuf,
    /// `$XDG_STATE_HOME/ani-gui/` — backing store for the watch
    /// history and the account tokens.
    pub state_dir: PathBuf,
    /// Per-process random secret renderer-only paths require as the
    /// `x-ani-gui-internal-secret` header. Currently used to gate the
    /// disconnect-after-expiry cache wipe (Codex P2 #3370011855) so a
    /// cross-origin tab under the permissive CORS layer can't poison
    /// another user's local cache.
    pub internal_secret: InternalSecret,
    /// Shared refresh-coalesce state for MAL. One slot per process —
    /// every `MalProvider` the `provider_for_kind` dispatcher
    /// constructs clones the cheap `Arc` so concurrent refresh
    /// handlers serialize on the same mutex and reuse the same
    /// rotation cache (Codex P2 #3379969316).
    pub mal_refresh: MalRefreshState,
    /// Per-(provider, show) write serialization for tracker write-back.
    /// The un-awaited fan-out can fire overlapping writes for the same
    /// show; this makes the read-then-upsert monotonic guard atomic so a
    /// later-landing lower write can't regress progress (Codex P2
    /// #3387237642). Process-wide; cloned `Arc` is cheap.
    pub account_write_locks: AccountWriteLocks,
    /// Per-(kitsu_id, mode) write ordering for the availability cache.
    /// A user's cache-bypassing re-ask and the page-load lookup can be
    /// in flight together, and the row is INSERT OR REPLACE — so
    /// without this the one that finishes last wins, and the ordinary
    /// lookup landing second reinstates the exact count the re-ask was
    /// sent to replace for the row's whole TTL. Process-wide; cloned
    /// `Arc` is cheap.
    pub availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes,
}

impl AppState {
    /// Build state from the resolved proxy origin and the shared http
    /// client. `resource_dir` is the packaged resources directory; its
    /// `bin/` holds the binaries the native resolver and the downloader
    /// spawn.
    ///
    /// # Errors
    /// - [`AniError::Io`] if the history file's parent directory can't be
    ///   resolved (e.g., XDG paths fail on an exotic platform).
    pub fn build(
        proxy_http: reqwest::Client,
        proxy_origin: ProxyOrigin,
        resource_dir: Option<PathBuf>,
    ) -> Result<Self> {
        // Bundled-deps dir lives at `<resource_dir>/bin`, holding the
        // impersonating transport the native resolver spawns plus the
        // downloader's tools. electron-builder stages it through
        // `extraResources`; cargo dev runs get the same path from the
        // `fetch:*-deps` scripts, so playback works without polluting
        // global PATH.
        let bundled_bin = resolve_bundled_bin(resource_dir.as_deref());
        let cache_root = paths::cache_dir().ok_or(AniError::Io)?;
        let state_root = paths::state_dir().ok_or(AniError::Io)?;
        // Earlier versions kept their own copy of the shell script in
        // the cache root and logged every update attempt under the
        // state dir. Nothing reads either now, so both are removed
        // rather than left behind — reported, not silent, through the
        // diagnostics page.
        let legacy_sweep = crate::legacy_script::sweep_legacy_files(&cache_root, &state_root);
        for path in &legacy_sweep.removed {
            tracing::info!(
                target: "legacy_script",
                path = %path.display(),
                "removed the script copy an earlier version maintained"
            );
        }
        let history_path = paths::gui_history().ok_or(AniError::Io)?;
        let image_cache_dir = paths::image_cache_dir().ok_or(AniError::Io)?;
        std::fs::create_dir_all(&image_cache_dir).map_err(|_| AniError::Io)?;
        let metadata_db = paths::metadata_db().ok_or(AniError::Io)?;
        if let Some(parent) = metadata_db.parent() {
            std::fs::create_dir_all(parent).map_err(|_| AniError::Io)?;
        }
        let cache_pool = crate::cache::open_pool(&metadata_db)?;
        let meta_http = crate::proxy::upstream::build_meta_client();
        let kitsu = KitsuClient::new(meta_http.clone());
        let config_path = paths::config_file().ok_or(AniError::Io)?;
        let state_dir = state_root;
        Ok(Self {
            secret: AppSecret::random(),
            sessions: SessionTable::new(),
            proxy_http,
            meta_http,
            proxy_origin,
            bundled_bin,
            legacy_sweep,
            history_path,
            anidb_base: None,
            anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
            image_cache_dir,
            cache_pool,
            kitsu,
            config_path,
            state_dir,
            internal_secret: InternalSecret::random(),
            mal_refresh: MalRefreshState::new(),
            account_write_locks: AccountWriteLocks::new(),
            availability_refreshes:
                crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        })
    }

    /// Configured image-cache size cap, in bytes. Reads from the
    /// user's settings TOML on each call (cheap; sub-millisecond)
    /// so a settings change applies immediately without restarting.
    /// Falls back to the documented default if the file is missing
    /// or unreadable.
    #[must_use]
    pub fn image_cache_cap_bytes(&self) -> u64 {
        let cfg = crate::config::read_config(&self.config_path).unwrap_or_default();
        cfg.image_cache_cap_mb.saturating_mul(1024 * 1024)
    }

    /// Convert into a [`ProxyState`] suitable for the axum router.
    #[must_use]
    pub fn proxy_state(&self) -> ProxyState {
        ProxyState {
            sessions: self.sessions.clone(),
            secret: self.secret.clone(),
            client: self.proxy_http.clone(),
            origin: self.proxy_origin.clone(),
        }
    }
}

/// Resolve the bundled-deps directory next to the backend binary.
/// `<resource_dir>/bin` holds what native resolution spawns and the
/// host may not have: the impersonating transport the provider
/// requires, plus the download tools. Both packaged platforms stage
/// it (`fetch:linux-deps` / `fetch:win-deps`). Returns `Some` only
/// when the dir actually exists, so a dev run that never staged it
/// comes out `None` and the spawn path falls through to PATH.
fn resolve_bundled_bin(resource_dir: Option<&std::path::Path>) -> Option<PathBuf> {
    resource_dir.map(|d| d.join("bin")).filter(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
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
            image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
            cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
            kitsu: KitsuClient::new(reqwest::Client::new()),
            config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
            state_dir: PathBuf::from("/tmp/ani-gui-state"),
            internal_secret: crate::account::InternalSecret::random(),
            mal_refresh: MalRefreshState::new(),
            account_write_locks: AccountWriteLocks::new(),
            availability_refreshes:
                crate::commands::availability_refresh::AvailabilityRefreshes::new(),
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
        // POSIX deps. Helper returns Some(<resource_dir>/bin) so
        // compose_anicli_path can prepend it.
        let td = tempfile::tempdir().expect("tempdir");
        let bin = td.path().join("bin");
        std::fs::create_dir(&bin).expect("mkdir bin");
        let got = resolve_bundled_bin(Some(td.path()));
        assert_eq!(got.as_deref(), Some(bin.as_path()));
    }
}
