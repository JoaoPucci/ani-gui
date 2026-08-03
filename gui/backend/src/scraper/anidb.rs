//! Native client for anidb.app — the provider ani-cli 5.0 scrapes.
//!
//! The flow mirrors the script's, endpoint for endpoint: a browse
//! page searched by title (HTML), an episodes listing and a
//! per-episode languages listing (JSON), and an embed page whose
//! jwplayer setup carries the master-playlist URL. Everything
//! downstream of that URL is already native (`proxy/m3u8`, sessions),
//! so this module is the whole remaining provider surface.
//!
//! Transport is pluggable through [`AnidbFetch`] because the site's
//! cloudflare front rejects ordinary HTTP clients by TLS fingerprint —
//! plain curl and reqwest both get the "Just a moment" interstitial.
//! The production implementation shells out to a curl-impersonate
//! binary, the same dependency ani-cli 5.0 itself prefers, resolved
//! through the same failover order. A Rust-native impersonation layer
//! (wreq) was spiked and rejected: its BoringSSL build drags a clang
//! toolchain into every contributor and packaging environment.
//!
//! Query encoding matches the script byte-for-byte (spaces become `+`,
//! nothing else is touched): the CLI and the GUI must see the same
//! result list for the same query, or shared history rows resolve to
//! different shows.

use std::path::{Path, PathBuf};

use crate::error::{AniError, Result};

/// Provider origin. Kept overridable at the client level for tests.
pub const ANIDB_BASE: &str = "https://anidb.app";

/// The user agent ani-cli 5.0 sends; the interstitial keys on TLS
/// fingerprint first but the agent rides along for parity.
pub const IMPERSONATE_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// curl binaries in preference order — ani-cli 5.0's failover list.
/// Impersonate builds come first: the plain-curl tail exists so the
/// resolver can still name an executable on hosts without them, where
/// the interstitial then surfaces as a typed upstream error rather
/// than a missing-binary one.
pub const CURL_FAILOVER: &[&str] = &[
    "curl_firefox135",
    "curl_chrome136",
    "curl_chrome116",
    "curl_ff117",
    "curl",
];

/// One row of the browse page: the slug the whole provider API is
/// keyed on, and the display title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseHit {
    /// e.g. `one-piece-69`. The numeric tail is the internal id.
    pub slug: String,
    /// Entity-decoded display title from the cover's alt text.
    pub title: String,
}

impl BrowseHit {
    /// The provider's internal id: the digits after the slug's last
    /// hyphen.
    pub fn numeric_id(&self) -> Option<u64> {
        todo!()
    }
}

/// One episode row: the db id the languages endpoint is keyed on and
/// the 1-based episode number shown to users.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeRef {
    pub id: u64,
    pub number: u32,
}

/// One playable embed for an episode, by audio language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageEmbed {
    /// Provider language code: `jpn` for sub, `eng` for dub.
    pub language: String,
    pub embed_url: String,
}

/// Whether a response body is cloudflare's challenge interstitial
/// rather than provider content.
pub fn is_cloudflare_interstitial(body: &str) -> bool {
    let _ = body;
    todo!()
}

/// Space→`+` and nothing else, mirroring the script's `sed 's| |+|g'`.
pub fn encode_query(query: &str) -> String {
    let _ = query;
    todo!()
}

/// Extract browse hits from the search page HTML. Titles are
/// entity-decoded (`&#039;`, `&quot;`, `&amp;`). A page without
/// matching anchors yields an empty list — "no results" is the
/// caller's verdict, not a parse failure.
pub fn parse_browse(html: &str) -> Vec<BrowseHit> {
    let _ = html;
    todo!()
}

/// Parse the episodes endpoint's JSON array into id/number pairs,
/// preserving order.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body isn't the expected array.
pub fn parse_episodes(json: &str) -> Result<Vec<EpisodeRef>> {
    let _ = json;
    todo!()
}

/// Parse the languages endpoint's JSON array into per-language embeds.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body isn't the expected array.
pub fn parse_languages(json: &str) -> Result<Vec<LanguageEmbed>> {
    let _ = json;
    todo!()
}

/// The embed the given mode plays: `jpn` for sub, `eng` for dub —
/// first match wins, as in the script.
pub fn preferred_embed<'a>(embeds: &'a [LanguageEmbed], mode: &str) -> Option<&'a LanguageEmbed> {
    let _ = (embeds, mode);
    todo!()
}

/// Pull the master-playlist URL out of an embed page's jwplayer
/// setup (`file: '…'`, first occurrence).
pub fn extract_master_url(embed_html: &str) -> Option<String> {
    let _ = embed_html;
    todo!()
}

/// A fetched response: enough for the client to tell content from a
/// challenge page without transport details leaking upward.
#[derive(Debug, Clone)]
pub struct FetchResponse {
    pub status: u16,
    pub body: String,
}

/// Transport seam. Implemented by the curl-impersonate subprocess in
/// production and by fixture-backed fakes in tests.
#[async_trait::async_trait]
pub trait AnidbFetch: Send + Sync {
    /// GET `url` and return status + body.
    ///
    /// # Errors
    /// [`AniError::Network`] on spawn/transport failure,
    /// [`AniError::Timeout`] when the request exceeds its deadline.
    async fn get(&self, url: &str) -> Result<FetchResponse>;
}

/// Production transport: a curl-impersonate binary spawned per
/// request with the script's own flags.
#[derive(Debug, Clone)]
pub struct CurlImpersonateFetch {
    exe: PathBuf,
}

impl CurlImpersonateFetch {
    /// Walk [`CURL_FAILOVER`] across `extra_dir` (the bundled-binary
    /// directory, when packaging ships one) and then the given PATH
    /// string, returning the first executable found — the same
    /// preference order as the script's `dep_ch_failover`.
    pub fn resolve(extra_dir: Option<&Path>, path_env: &str) -> Option<Self> {
        let _ = (extra_dir, path_env);
        todo!()
    }

    /// The resolved executable, for logging and diagnostics.
    pub fn exe(&self) -> &Path {
        &self.exe
    }
}

#[async_trait::async_trait]
impl AnidbFetch for CurlImpersonateFetch {
    async fn get(&self, url: &str) -> Result<FetchResponse> {
        let _ = url;
        todo!()
    }
}

/// The provider client: search, episode listing, and stream-URL
/// resolution over any [`AnidbFetch`].
pub struct AnidbClient<F> {
    fetch: F,
    base: String,
}

impl<F: AnidbFetch> AnidbClient<F> {
    pub fn new(fetch: F) -> Self {
        Self {
            fetch,
            base: ANIDB_BASE.to_string(),
        }
    }

    /// Test seam: point the client at a stub origin.
    pub fn with_base(fetch: F, base: &str) -> Self {
        Self {
            fetch,
            base: base.to_string(),
        }
    }

    /// Search the browse page. An interstitial or non-success status
    /// is a typed upstream error; a result-less page is `Ok(vec![])`.
    ///
    /// # Errors
    /// [`AniError::Upstream`] when cloudflare or the site refuses,
    /// plus the transport errors of [`AnidbFetch::get`].
    pub async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        let _ = query;
        todo!()
    }

    /// List a show's episodes by slug.
    ///
    /// # Errors
    /// [`AniError::ParseFailed`] on a malformed slug or body, plus
    /// upstream/transport errors as in [`Self::search`].
    pub async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        let _ = slug;
        todo!()
    }

    /// Resolve an episode's master-playlist URL for `sub`/`dub`:
    /// languages → preferred embed → embed page → jwplayer `file:`.
    ///
    /// # Errors
    /// [`AniError::NoResults`] when no embed matches the mode or the
    /// embed page carries no playlist, plus upstream/transport errors.
    pub async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<String> {
        let _ = (episode_id, mode);
        todo!()
    }
}

#[cfg(test)]
#[path = "anidb_test.rs"]
mod tests;
