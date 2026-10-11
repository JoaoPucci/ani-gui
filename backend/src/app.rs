//! `AppState` — the one state value the HTTP API handlers share.
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
//! Built once by [`AppState::build`] at backend startup and shared with
//! the API router as `Arc<AppState>`; the proxy router gets the derived
//! [`ProxyState`] instead.

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

/// The one state container the HTTP API handlers share.
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
    /// Path of the app's watch-history file.
    pub history_path: PathBuf,
    /// Test override for the anidb provider origin the native play
    /// resolution scrapes. `None` in production (the real site).
    pub anidb_base: Option<String>,
    /// Admission gate for anidb traffic: paces background probes
    /// and breaks the circuit on consecutive failures so cold caches
    /// can't rate-limit the IP out from under a user's click.
    pub anidb_gate: Arc<crate::scraper::gate::ScraperGate>,
    /// Test override for the hianime origin. `None` in production.
    pub hianime_base: Option<String>,
    /// hianime's own admission gate. One breaker per provider: an
    /// outage on one must not pace or refuse traffic to the other.
    pub hianime_gate: Arc<crate::scraper::gate::ScraperGate>,
    /// The providers a walk runs against, in order — the next is
    /// asked only when the one before it was unreachable, refusing
    /// or broken (see `commands::providers`). Production lists them
    /// all; a test lists the ones it stubs, so a stubbed outage
    /// cannot fall through to a real site.
    pub provider_order: Vec<crate::scraper::provider::ProviderId>,
    /// On-disk image-cache directory served by the `/api/image` route.
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
            hianime_base: None,
            hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
            provider_order: vec![
                crate::scraper::provider::ProviderId::Anidb,
                crate::scraper::provider::ProviderId::Hianime,
            ],
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

    /// Convert into the [`ProxyState`] the proxy router is built with.
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
#[path = "app_test.rs"]
mod tests;
