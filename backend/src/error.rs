//! `AniError` — the single error type that crosses every command and
//! HTTP API boundary.
//!
//! Every variant maps to a stable i18n key returned to the frontend.
//! Localized strings are resolved by the frontend (Paraglide), never by the
//! backend. [`crate::i18n`] holds the key constants; `backend/error-keys.json`
//! publishes the key of every variant for the renderer's catalogues, and
//! `error_test.rs` holds that file to the variant list in
//! `error_variants_test.rs`.

use serde::Serialize;
use thiserror::Error;

/// Result alias for backend operations.
pub type Result<T, E = AniError> = std::result::Result<T, E>;

/// Any failure that may occur in the backend. A variant serializes with
/// a `kind` discriminator and its fields; the error envelope every HTTP
/// error and SSE error event carries (`api::ani_error_payload`) adds
/// its stable i18n `key` ([`AniError::key`]), so the UI can localize
/// without parsing the message.
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AniError {
    /// A download tool failed: yt-dlp or ffmpeg exited non-zero, or was
    /// found but could not be started. The only thing that raises this
    /// is `spawn_download_tool`, in ways that are not a missing tool and
    /// not a timeout — provider resolution has its own variants and
    /// constructs none of these. The name is older than that narrowing.
    #[error("scraper error")]
    Scraper {
        /// Which way the tool failed; each has its own key. Not
        /// serialized: the envelope's `key` already says which.
        #[serde(skip)]
        failure: DownloadToolFailure,
    },

    /// A deadline elapsed. The shared one: a bounded resolve and an
    /// availability probe both talk to the provider and both end here,
    /// and so does a download whose transfer ran past its hour, which
    /// is not the provider's doing at all.
    #[error("deadline exceeded")]
    Timeout,

    /// The streaming source has no match for the title: raised while
    /// resolving a play or a download, not by the catalogue search.
    #[error("no results")]
    NoResults,

    /// The show was found and the episode was not — not listed, or
    /// with no stream in the requested mode. The episode's verdict,
    /// distinct from a title the catalogue lacks, so the page can
    /// say which; an answer, like the miss, never a block.
    #[error("episode unavailable")]
    EpisodeUnavailable,

    /// The show was found and its stream server did not serve the
    /// episode: a host the episode's chain asked — the provider's own
    /// listing of the episode's servers, a server's embed host, the
    /// playlist's CDN — did not answer, timed out, or answered a
    /// page, payload or playlist the client could not play. The
    /// chain's verdict for every failure that is not an answer. A
    /// transport failure there is not the user's connection, which
    /// the searches before it got through on, and a parse failure
    /// there is not the app failing to read the provider, which it
    /// read all the way to that host — so neither surfaces under its
    /// own name from the chain. It therefore also covers the client
    /// no longer reading a host — the site having changed shape under
    /// it — which from the chain is told from a host serving nothing
    /// only by the cause the log carries; the copy's advice, later or
    /// another episode, is the best the renderer can give for either.
    /// Distinct from [`Self::EpisodeUnavailable`], a host answering
    /// that the episode is not there. Transient to every walk — the
    /// next alias or provider may serve — and distress to the breaker,
    /// like the failures it stands for.
    ///
    /// Both fields are for the log: the renderer shows one sentence
    /// whatever they say, and never the host.
    #[error("stream server failed: stage={stage} host={}", .host.as_deref().unwrap_or("unnamed"))]
    StreamServerFailed {
        /// Where in the chain the host failed: `servers` (the
        /// provider's listing of the episode's servers), `embed` (a
        /// server's player page and what that page has the client ask
        /// its host for), `playlist` (the master or the rendition on
        /// the CDN), or `stream` when the provider named no stage and
        /// the episode step named the chain as a whole.
        stage: &'static str,
        /// The host asked at that stage, when the provider named it.
        host: Option<String>,
    },

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
    /// on its own, like a session's upstream URL.
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
    /// The other way in is a yt-dlp run reporting it could not
    /// repackage. With an ffmpeg to retry through, that is not this
    /// error; without one it is, and the run ends here after the
    /// scratch guard removes what the warning condemned. The output
    /// is not examined, because it no longer needs to be: the tool
    /// wrote to this run's own uuid-named scratch, never to the
    /// user's file, so everything under that prefix is this run's to
    /// discard and nothing else is touched.
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

    /// An account request the backend could not carry out: an unknown
    /// or unconfigured list provider, a PKCE request it cannot convert,
    /// AniList's absent refresh flow or a delete it reported as not
    /// done, a Watch Later bridge asking for too many ids. The name is
    /// older than that use; malformed bodies are `ParseFailed`.
    #[error("metadata source")]
    Metadata,

    /// Request supplied a PKCE configuration the provider doesn't accept
    /// (today: MAL rejects `S256`). Distinct from `Metadata` so the
    /// route handler can return 400 instead of 500 — the client / renderer
    /// sent the bad value, not the server.
    #[error("unsupported pkce method for this provider")]
    UnsupportedPkce,

    /// The renderer named a Kitsu id that is not one: anything but
    /// ASCII digits once surrounding whitespace is trimmed. Refused
    /// at the route boundary ([`crate::kitsu_id`]) before the value
    /// reaches a cache key, a row, or an outbound URL. 400, like
    /// [`Self::UnsupportedPkce`]: the caller sent the bad value.
    #[error("invalid kitsu id")]
    InvalidKitsuId,

    /// The renderer asked for a search page that is not one: an offset
    /// off a page boundary or past the deepest page a search walks to.
    /// 400, like [`Self::InvalidKitsuId`]: the caller sent the bad value.
    #[error("invalid search offset")]
    InvalidSearchOffset,

    /// A request body that is not the shape its route takes: not JSON,
    /// or missing a field it requires, other than the fields with a key
    /// of their own (a search page's offset answers
    /// [`Self::InvalidSearchOffset`]). 400: the caller sent it. Routes
    /// that read their body themselves answer this instead of the
    /// framework's plain-text rejection, so the renderer gets the same
    /// keyed envelope as for every other error.
    #[error("invalid request body")]
    InvalidRequestBody,

    /// A token or secret the request carried is missing, expired or
    /// forged: a stream session's token at the proxy, an account's
    /// bearer at the account routes, AniList or MAL answering 401, the
    /// internal secret the Watch Later bridge checks. The `stream` in
    /// its key is older than the account uses.
    #[error("invalid stream token")]
    InvalidToken,
}

/// How a download tool failed, for [`AniError::Scraper`]. A closed set,
/// so the keys the backend can answer with are too: each value has one
/// key, chosen in [`AniError::key`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadToolFailure {
    /// The tool ran and exited non-zero.
    Exited,
    /// The tool was found but could not be started: gone since it was
    /// found, not executable, or refused by the OS.
    NotStarted,
}

impl AniError {
    /// Stable i18n key used by the frontend to look up a localized message.
    /// Variants without their own key fall back to a top-level key by
    /// variant name.
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Scraper {
                failure: DownloadToolFailure::Exited,
            } => crate::i18n::keys::DOWNLOAD_TOOL_FAILED,
            Self::Scraper {
                failure: DownloadToolFailure::NotStarted,
            } => crate::i18n::keys::DOWNLOAD_TOOL_SPAWN_FAILED,
            Self::Timeout => "error.scraper.timeout",
            Self::NoResults => "error.search.no_results",
            Self::EpisodeUnavailable => crate::i18n::keys::PLAY_EPISODE_UNAVAILABLE,
            Self::StreamServerFailed { .. } => crate::i18n::keys::PLAY_STREAM_SERVER_FAILED,
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
            Self::InvalidKitsuId => crate::i18n::keys::REQUEST_INVALID_KITSU_ID,
            Self::InvalidSearchOffset => crate::i18n::keys::REQUEST_INVALID_SEARCH_OFFSET,
            Self::InvalidRequestBody => crate::i18n::keys::REQUEST_INVALID_BODY,
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

    /// Whether this error is the upstream answering that the resource
    /// asked for is absent — a 404 or 410 — as opposed to refusing
    /// this client ([`Self::is_provider_block`]) or rejecting this
    /// request, which a 400 or 401 does. Only absence is an answer
    /// about the one thing asked for; the episode step reads it as
    /// the episode's own verdict, and nothing else as one.
    #[must_use]
    pub fn is_not_found_shaped(&self) -> bool {
        matches!(self, Self::Upstream { status } if *status == 404 || *status == 410)
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
            Self::NoResults | Self::EpisodeUnavailable => 404,
            Self::InvalidToken => 401,
            // A rate-limit passes through verbatim so the frontend can tell it
            // apart from a generic bad gateway and tell the user to retry.
            Self::Upstream { status: 429 } => 429,
            Self::RateLimited { .. } => 429,
            // The stream server is the gateway's upstream too: it was
            // asked and did not serve.
            Self::Upstream { .. } | Self::StreamServerFailed { .. } => 502,
            Self::Network => 503,
            Self::GateRefused => 503,
            Self::Timeout => 504,
            Self::UnsupportedPkce
            | Self::InvalidKitsuId
            | Self::InvalidSearchOffset
            | Self::InvalidRequestBody => 400,
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

#[cfg(test)]
#[path = "error_variants_test.rs"]
pub(crate) mod variants;
