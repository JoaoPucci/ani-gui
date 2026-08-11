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

pub mod fetch;
pub mod gated;
pub mod parse;
pub mod parse_api;
pub use fetch::{AnidbFetch, CurlImpersonateFetch, FetchResponse};
pub use gated::GatedFetch;
pub use parse::{encode_query, is_cloudflare_interstitial, parse_browse, parse_detail_year};
pub use parse_api::{
    extract_master_url, parse_episodes, parse_languages, parse_master_variants, preferred_embed,
    select_variant, MasterVariant,
};

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
    /// The card's format badge (`TV`, `Movie`, `OVA`, ...) when the
    /// markup carries one. `None` reads as unknown — a soft signal,
    /// like an unparseable year.
    pub kind: Option<String>,
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
    /// The provider's display tag when it differs from `number` —
    /// recaps and specials stream under decimal tags ("1061.5"),
    /// and a decimal play request matches this field verbatim.
    pub number2: Option<String>,
}

/// One playable embed for an episode, by audio language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageEmbed {
    /// Provider language code: `jpn` for sub, `eng` for dub.
    pub language: String,
    /// The player page the master-playlist URL is extracted from.
    pub embed_url: String,
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

    /// The transport this client fetches through. The orchestrator
    /// reads the gated transport's per-attempt stamp off it when
    /// recording breaker outcomes ([`GatedFetch::last_attempt_at`]).
    pub fn transport(&self) -> &F {
        &self.fetch
    }

    /// Search the browse page. An interstitial or non-success status
    /// is a typed upstream error; a result-less page is `Ok(vec![])`
    /// only when it shows the browse shape — an unrecognized zero-hit
    /// body is a parse failure, never absence.
    ///
    /// # Errors
    /// [`AniError::Upstream`] when cloudflare or the site refuses,
    /// [`AniError::ParseFailed`] on an unrecognized zero-hit body,
    /// plus the transport errors of [`AnidbFetch::get`].
    pub async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        let url = format!("{}/browse?q={}", self.base, encode_query(query));
        let body = self.content(&url).await?;
        parse_browse(&body)
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

    /// The stream URL a quality setting selects from a master
    /// playlist, mirroring the script's `select_quality`. `best`
    /// keeps the adaptive master URL (hls.js picks levels itself)
    /// after one validating fetch; any other setting parses
    /// its variants and returns the matching height's URI resolved
    /// against the master's URL. Soft only on a SERVED playlist that
    /// misses — an unserved height, an unparseable body or variant
    /// URI — where the master URL comes back and playback stays
    /// adaptive.
    ///
    /// # Errors
    /// The fetch's own failure: returning the master URL that just
    /// failed would report success upstream — stamping availability,
    /// caching a session the player cannot load — and a swallowed
    /// 429 would record breaker health instead of the rate-limit
    /// pause.
    pub async fn quality_stream_url(&self, master_url: &str, quality: &str) -> Result<String> {
        // One validating fetch on EVERY path, best included: the
        // extracted URL is only a claim until the playlist answers,
        // and an unvalidated claim rides into breaker success,
        // availability, history, and a cached session the proxy
        // cannot load.
        let body = self.content(master_url).await?;
        if quality == "best" {
            return Ok(master_url.to_string());
        }
        let variants = parse_master_variants(&body);
        let Some(variant) = select_variant(&variants, quality) else {
            tracing::debug!(
                quality,
                "anidb: quality not served, keeping adaptive master"
            );
            return Ok(master_url.to_string());
        };
        let rendition = match url::Url::parse(master_url).and_then(|base| base.join(&variant.url)) {
            Ok(joined) => joined.to_string(),
            Err(_) => return Ok(master_url.to_string()),
        };
        // The rendition gets its own validating fetch: a dead
        // rendition behind a healthy master must not report success
        // — and an ANSWERED miss must not fail a play the served
        // adaptive master can carry, so that one falls back soft. A
        // refusal, rate limit, or transport failure is not a miss:
        // hls.js would request renditions through the same blocked
        // upstream, and masking it records breaker success and
        // stamps availability, history, and a cached session on a
        // blocked play.
        match self.content(&rendition).await {
            Ok(_) => Ok(rendition),
            Err(AniError::Upstream { status })
                if !AniError::Upstream { status }.is_provider_block() =>
            {
                tracing::debug!(
                    quality,
                    status,
                    "anidb: rendition not served, keeping adaptive master"
                );
                Ok(master_url.to_string())
            }
            Err(e) => Err(e),
        }
    }

    /// The premiere year the slug's detail page names, when it names
    /// one. A missing page (not-found-shaped status) or a page
    /// without a season link is the soft `Ok(None)` — the year is an
    /// identity hint, and resolution must not die on a missing hint.
    /// A refusal, rate limit, or transport failure is NOT a missing
    /// hint: it is the provider blocking this client, and swallowing
    /// it would let the picker keep probing detail pages and select
    /// year-blind through the block.
    ///
    /// # Errors
    /// [`AniError::RateLimited`], refusal-shaped [`AniError::Upstream`]
    /// statuses, and transport errors, verbatim from the fetch.
    pub async fn detail_year(&self, slug: &str) -> Result<Option<u32>> {
        let url = format!("{}/anime/{slug}", self.base);
        match self.content(&url).await {
            Ok(body) => Ok(parse_detail_year(&body)),
            Err(AniError::Upstream { status })
                if !AniError::Upstream { status }.is_provider_block() =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
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
#[path = "anidb_test.rs"]
mod tests;
