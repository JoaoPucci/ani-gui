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

pub mod parse;
pub use parse::{
    encode_query, extract_master_url, is_cloudflare_interstitial, parse_browse, parse_episodes,
    parse_languages, preferred_embed,
};

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
        parse::slug_numeric_id(&self.slug)
    }
}

/// One episode row: the db id the languages endpoint is keyed on and
/// the 1-based episode number shown to users.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeRef {
    /// The provider's episode db id — what the languages endpoint
    /// takes.
    pub id: u64,
    /// 1-based episode number as shown to users.
    pub number: u32,
}

/// One playable embed for an episode, by audio language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageEmbed {
    /// Provider language code: `jpn` for sub, `eng` for dub.
    pub language: String,
    /// The player page the master-playlist URL is extracted from.
    pub embed_url: String,
}

/// A fetched response: enough for the client to tell content from a
/// challenge page without transport details leaking upward.
#[derive(Debug, Clone)]
pub struct FetchResponse {
    /// HTTP status of the final response after redirects.
    pub status: u16,
    /// Response body, lossily decoded.
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
        for name in CURL_FAILOVER {
            if let Some(dir) = extra_dir {
                let candidate = dir.join(name);
                if is_executable(&candidate) {
                    return Some(Self { exe: candidate });
                }
            }
            for dir in std::env::split_paths(path_env) {
                let candidate = dir.join(name);
                if is_executable(&candidate) {
                    return Some(Self { exe: candidate });
                }
            }
        }
        None
    }

    /// The resolved executable, for logging and diagnostics.
    pub fn exe(&self) -> &Path {
        &self.exe
    }
}

/// Whether `path` names an executable regular file.
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Per-request deadline for the subprocess; slightly above the
/// script's own `--max-time 10` so curl reports its timeout first.
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

#[async_trait::async_trait]
impl AnidbFetch for CurlImpersonateFetch {
    async fn get(&self, url: &str) -> Result<FetchResponse> {
        // `-w` appends the status after the body; the last line is
        // split back off. Mirrors the script's anidb_curl flags.
        let mut cmd = tokio::process::Command::new(&self.exe);
        cmd.arg("-sL")
            .arg("-A")
            .arg(IMPERSONATE_AGENT)
            .arg("--max-time")
            .arg("10")
            .arg("-w")
            .arg("\n%{http_code}")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let output = tokio::time::timeout(FETCH_TIMEOUT, cmd.output())
            .await
            .map_err(|_| AniError::Timeout)?
            .map_err(|_| AniError::Network)?;
        let text = String::from_utf8_lossy(&output.stdout);
        let (body, status_line) = text.rsplit_once('\n').unwrap_or(("", &text));
        let status: u16 = status_line.trim().parse().map_err(|_| AniError::Network)?;
        if status == 0 {
            // curl writes 000 when the transfer itself failed.
            return Err(AniError::Network);
        }
        Ok(FetchResponse {
            status,
            body: body.to_string(),
        })
    }
}

/// The provider client: search, episode listing, and stream-URL
/// resolution over any [`AnidbFetch`].
pub struct AnidbClient<F> {
    fetch: F,
    base: String,
}

impl<F: AnidbFetch> AnidbClient<F> {
    /// A client against the production origin.
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
        let url = format!("{}/browse?q={}", self.base, encode_query(query));
        let body = self.content(&url).await?;
        Ok(parse_browse(&body))
    }

    /// List a show's episodes by slug.
    ///
    /// # Errors
    /// [`AniError::ParseFailed`] on a malformed slug or body, plus
    /// upstream/transport errors as in [`Self::search`].
    pub async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        let id = parse::slug_numeric_id(slug).ok_or_else(|| AniError::ParseFailed {
            detail: format!("anidb slug without numeric tail: {slug}"),
        })?;
        let url = format!("{}/api/frontend/anime/{id}/episodes", self.base);
        let body = self.content(&url).await?;
        parse_episodes(&body)
    }

    /// Resolve an episode's master-playlist URL for `sub`/`dub`:
    /// languages → preferred embed → embed page → jwplayer `file:`.
    ///
    /// # Errors
    /// [`AniError::NoResults`] when no embed matches the mode or the
    /// embed page carries no playlist, plus upstream/transport errors.
    pub async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<String> {
        let url = format!("{}/api/frontend/episode/{episode_id}/languages", self.base);
        let body = self.content(&url).await?;
        let embeds = parse_languages(&body)?;
        let embed = preferred_embed(&embeds, mode).ok_or(AniError::NoResults)?;
        let embed_body = self.content(&embed.embed_url).await?;
        extract_master_url(&embed_body).ok_or(AniError::NoResults)
    }

    /// Fetch `url` and hand back content, refusing challenge pages
    /// and non-success statuses as typed upstream errors.
    async fn content(&self, url: &str) -> Result<String> {
        let resp = self.fetch.get(url).await?;
        if is_cloudflare_interstitial(&resp.body) {
            let status = if resp.status >= 400 { resp.status } else { 403 };
            return Err(AniError::Upstream { status });
        }
        if !(200..300).contains(&resp.status) {
            return Err(AniError::Upstream {
                status: resp.status,
            });
        }
        Ok(resp.body)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
