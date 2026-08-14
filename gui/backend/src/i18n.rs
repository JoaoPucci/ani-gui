//! Stable i18n key constants returned by the backend.
//!
//! The backend never returns localized strings — only these keys. The
//! frontend resolves them via Paraglide using the user's chosen locale.
//!
//! Keys are organized by surface:
//!
//! - `error.scraper.*` — failures from provider resolution
//! - `error.search.*` — search-specific outcomes
//! - `error.network.*` — HTTP / TLS / connectivity
//! - `error.cache.*` — SQLite + on-disk image cache
//! - `error.config.*` — config file (TOML)
//! - `error.metadata.*` — Kitsu / AniList responses
//! - `error.io.*` — generic filesystem
//! - `error.stream.*` — stream proxy + tokens

/// Key constants. Keep alphabetized within each block.
pub mod keys {
    // --- error.cache.* ---
    /// Generic SQLite failure.
    pub const CACHE_GENERIC: &str = "error.cache.generic";

    // --- error.config.* ---
    /// TOML parse or write failure.
    pub const CONFIG_PARSE: &str = "error.config.parse";

    // --- error.download.* ---
    /// A download needs yt-dlp or ffmpeg to turn the HLS stream into
    /// an MP4, and neither is installed. `download_with_tools` refuses
    /// on this before it resolves anything, so the frontend's install
    /// modal is what the user meets rather than a generic failure a
    /// provider walk got to first. A yt-dlp run that cannot repackage
    /// reaches it by the same test: ffmpeg is tried first, and this is
    /// what is left when there is none.
    ///
    /// The name predates yt-dlp becoming an accepted alternative; the
    /// key is what the frontend keys its modal on, so renaming it is a
    /// frontend change too.
    pub const DOWNLOAD_FFMPEG_MISSING: &str = "error.download.ffmpeg_missing";

    // --- error.io.* ---
    /// Generic filesystem error.
    pub const IO_GENERIC: &str = "error.io.generic";

    // --- error.metadata.* ---
    /// Kitsu/AniList returned malformed data.
    pub const METADATA_SOURCE: &str = "error.metadata.source";

    // --- error.network.* ---
    /// Could not reach the upstream host.
    pub const NETWORK_UNREACHABLE: &str = "error.network.unreachable";
    /// Upstream returned a non-success HTTP status.
    pub const NETWORK_UPSTREAM: &str = "error.network.upstream";

    // --- error.scraper.* ---
    /// A provider response could not be parsed.
    pub const SCRAPER_PARSE_FAILED: &str = "error.scraper.parse_failed";
    /// Resolution exceeded its timeout.
    pub const SCRAPER_TIMEOUT: &str = "error.scraper.timeout";

    // --- error.search.* ---
    /// Search returned zero results.
    pub const SEARCH_NO_RESULTS: &str = "error.search.no_results";

    // --- error.stream.* ---
    /// Stream token missing/expired/forged.
    pub const STREAM_INVALID_TOKEN: &str = "error.stream.invalid_token";
}

#[cfg(test)]
mod tests {
    use super::keys::*;

    /// Every constant in this module must be a non-empty string of the form
    /// `error.<scope>.<name>`. A regression here means an i18n key broke
    /// silently — the frontend would look up a missing key and fall through.
    #[test]
    fn every_key_is_well_formed() {
        let all = [
            CACHE_GENERIC,
            CONFIG_PARSE,
            DOWNLOAD_FFMPEG_MISSING,
            IO_GENERIC,
            METADATA_SOURCE,
            NETWORK_UNREACHABLE,
            NETWORK_UPSTREAM,
            SCRAPER_PARSE_FAILED,
            SCRAPER_TIMEOUT,
            SEARCH_NO_RESULTS,
            STREAM_INVALID_TOKEN,
        ];
        for k in all {
            assert!(
                k.starts_with("error."),
                "i18n key '{k}' must start with 'error.'"
            );
            let parts: Vec<_> = k.split('.').collect();
            assert!(
                parts.len() >= 3,
                "i18n key '{k}' should have at least 3 segments"
            );
            assert!(
                parts.iter().all(|p| !p.is_empty()),
                "no empty segments in '{k}'"
            );
        }
    }
}
