//! `AniError` — the single error type that crosses every Tauri command
//! boundary.
//!
//! Every variant maps to a stable i18n key returned to the frontend.
//! Localized strings are resolved by the frontend (Paraglide), never by the
//! backend. See [`crate::i18n`] for the canonical key list.

use serde::Serialize;
use thiserror::Error;

/// Result alias for backend operations.
pub type Result<T, E = AniError> = std::result::Result<T, E>;

/// Any failure that may occur in the backend. Variants serialize to the
/// frontend with a `kind` discriminator and an i18n `key` so the UI can
/// localize without parsing the message.
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AniError {
    /// A download tool exited non-zero. The only thing that raises
    /// this is `spawn_download_tool` when yt-dlp or ffmpeg fails in a
    /// way that is not a missing tool and not a timeout — provider
    /// resolution has its own variants and constructs none of these.
    /// The name is older than that narrowing.
    #[error("scraper error")]
    Scraper {
        /// i18n key under `error.scraper.*`.
        key: &'static str,
    },

    /// Resolution didn't finish within its timeout.
    #[error("scraper timed out")]
    Timeout,

    /// Search returned zero results.
    #[error("no results")]
    NoResults,

    /// The provider answered HTTP 200 with an in-band "Too many
    /// requests" error — an application-level rate limit. Distinct
    /// from [`AniError::Upstream`] with 429: the status line says
    /// nothing, only the payload does.
    #[error("rate limited by upstream")]
    RateLimited {
        /// Seconds the upstream asked us to wait ("please try again
        /// in N seconds"); `None` when the message carried no
        /// parseable number.
        retry_after_secs: Option<u64>,
    },

    /// Something did not have the shape the code required. Mostly
    /// upstream bodies — Kitsu, AniList, MAL, the provider's HTML and
    /// JSON, the proxy's manifests — but also input the app validates
    /// on its own, like a session's upstream URL or an `image://` URI.
    /// The shared parse-or-validate variant, not a provider one.
    #[error("parse failed: {detail}")]
    ParseFailed {
        /// Free-text detail for logs only — not surfaced to the user.
        detail: String,
    },

    /// A download has no tool to run: neither yt-dlp nor ffmpeg is on
    /// PATH or in the bundled-bin directory. Raised on the way in,
    /// before any provider request, so the frontend's install modal is
    /// what the user meets rather than a network error from a walk
    /// that could not have been used.
    ///
    /// The other way in is a yt-dlp run that reports MPEG-TS left
    /// under the .mp4 name: the mislabeled file is deleted, and the
    /// stream is retried through ffmpeg when one exists — so this
    /// error still means what it says, that no tool is left to try.
    ///
    /// The name predates yt-dlp becoming an accepted alternative. It
    /// is the key the frontend renders its modal on, so renaming it is
    /// a frontend change too.
    #[error("ffmpeg required for downloads")]
    FfmpegMissing,

    /// An upstream HTTP request returned a non-success status.
    #[error("upstream {status}")]
    Upstream {
        /// HTTP status code from the upstream server.
        status: u16,
    },

    /// A network-layer failure (connection refused, DNS, TLS).
    #[error("network error")]
    Network,

    /// The scraper gate refused the request (breaker open, background
    /// priority). The gate's own answer, not the provider's: outcome
    /// recording skips it entirely — recording it as failure would
    /// let background warmups keep extending the open breaker's
    /// cooldown without a single provider request. Callers surface it
    /// like a transient network failure.
    #[error("scraper gate refused")]
    GateRefused,

    /// External-player binary couldn't be spawned (not on PATH, or
    /// the configured path doesn't point at an executable). The
    /// `binary` field carries the configured player name so the UI
    /// can name the failed command in the error toast.
    #[error("player spawn failed: {binary}")]
    PlayerSpawnFailed {
        /// The player command the user configured (e.g. `"vlc"`,
        /// `"/usr/bin/mpv"`). Surfaced verbatim in the localized
        /// error message via the `{binary}` placeholder.
        binary: String,
    },

    /// Syncplay binary couldn't be spawned. Distinct from
    /// `PlayerSpawnFailed` because the recovery action is different:
    /// the frontend's ErrorOverlay links the user to syncplay.pl
    /// (Syncplay is a separate install) rather than telling them to
    /// fix their external-player setting.
    #[error("syncplay spawn failed: {binary}")]
    SyncplaySpawnFailed {
        /// The Syncplay binary path the user configured (default
        /// `"syncplay"` on Linux/Win, the macOS .app inner path on
        /// macOS). Surfaced verbatim via the localized message's
        /// `{binary}` placeholder.
        binary: String,
    },

    /// Cache (SQLite) operation failed.
    #[error("cache error")]
    Cache,

    /// Filesystem I/O failure.
    #[error("io error")]
    Io,

    /// Configuration file (TOML) parse or write failure.
    #[error("config error")]
    Config,

    /// Metadata source (Kitsu or AniList) returned malformed data.
    #[error("metadata source")]
    Metadata,

    /// Request supplied a PKCE configuration the provider doesn't accept
    /// (today: MAL rejects `S256`). Distinct from `Metadata` so the
    /// route handler can return 400 instead of 500 — the client / renderer
    /// sent the bad value, not the server.
    #[error("unsupported pkce method for this provider")]
    UnsupportedPkce,

    /// Stream session token was missing, expired, or signature-invalid.
    #[error("invalid stream token")]
    InvalidToken,
}

impl AniError {
    /// Stable i18n key used by the frontend to look up a localized message.
    /// Variants without their own key fall back to a top-level key by
    /// variant name.
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Scraper { key } => key,
            Self::Timeout => "error.scraper.timeout",
            Self::NoResults => "error.search.no_results",
            Self::ParseFailed { .. } => "error.scraper.parse_failed",
            Self::FfmpegMissing => crate::i18n::keys::DOWNLOAD_FFMPEG_MISSING,
            Self::PlayerSpawnFailed { .. } => "error.player.spawn_failed",
            Self::SyncplaySpawnFailed { .. } => "error.syncplay.spawn_failed",
            Self::Upstream { .. } => "error.network.upstream",
            Self::RateLimited { .. } => "error.network.rate_limited",
            Self::Network => "error.network.unreachable",
            // The user-facing story is the same transient
            // couldn't-reach; the variant exists for outcome
            // recording, not for copy.
            Self::GateRefused => "error.network.unreachable",
            Self::Cache => "error.cache.generic",
            Self::Io => "error.io.generic",
            Self::Config => "error.config.parse",
            Self::Metadata => "error.metadata.source",
            Self::UnsupportedPkce => "error.account.unsupported_pkce",
            Self::InvalidToken => "error.stream.invalid_token",
        }
    }

    /// Whether this error is the provider blocking THIS CLIENT rather
    /// than answering about one resource: a rate limit, or a
    /// refusal-shaped upstream status (403 interstitial, 429, 5xx).
    /// Probe loops stop on these — one block turns every further
    /// request into hole-deepening — while not-found-shaped statuses
    /// and transport failures speak only about the single request.
    #[must_use]
    pub fn is_provider_block(&self) -> bool {
        match self {
            Self::RateLimited { .. } => true,
            Self::Upstream { status } => *status == 403 || *status == 429 || *status >= 500,
            _ => false,
        }
    }

    /// HTTP status code the route layer surfaces for this variant.
    /// Lives here (next to the variant declarations) instead of on the
    /// `IntoResponse` impl in `api/mod.rs` because that file is already
    /// the largest match in the codebase — every new variant otherwise
    /// nudges its CRAP score, even when the variant has nothing to do
    /// with API routing.
    #[must_use]
    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::NoResults => 404,
            Self::InvalidToken => 401,
            // A rate-limit passes through verbatim so the frontend can tell it
            // apart from a generic bad gateway and tell the user to retry.
            Self::Upstream { status: 429 } => 429,
            Self::RateLimited { .. } => 429,
            Self::Upstream { .. } => 502,
            Self::Network => 503,
            Self::GateRefused => 503,
            Self::Timeout => 504,
            Self::UnsupportedPkce => 400,
            Self::ParseFailed { .. }
            | Self::FfmpegMissing
            | Self::PlayerSpawnFailed { .. }
            | Self::SyncplaySpawnFailed { .. }
            | Self::Cache
            | Self::Io
            | Self::Config
            | Self::Metadata
            | Self::Scraper { .. } => 500,
        }
    }
}

impl From<reqwest::Error> for AniError {
    fn from(_: reqwest::Error) -> Self {
        AniError::Network
    }
}

impl From<rusqlite::Error> for AniError {
    fn from(_: rusqlite::Error) -> Self {
        AniError::Cache
    }
}

impl From<std::io::Error> for AniError {
    fn from(_: std::io::Error) -> Self {
        AniError::Io
    }
}

impl From<serde_json::Error> for AniError {
    fn from(e: serde_json::Error) -> Self {
        AniError::ParseFailed {
            detail: e.to_string(),
        }
    }
}

impl From<toml::de::Error> for AniError {
    fn from(_: toml::de::Error) -> Self {
        AniError::Config
    }
}

#[cfg(test)]
#[path = "error_test.rs"]
mod tests;
