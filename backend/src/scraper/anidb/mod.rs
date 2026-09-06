//! Native client for anidb.app — the provider ani-cli 5.0 scrapes.
//!
//! The flow mirrors the script's, endpoint for endpoint: a browse
//! page searched by title (HTML), an episodes listing and a
//! per-episode languages listing (JSON), and an embed page whose
//! jwplayer setup carries the master-playlist URL. Everything
//! downstream of that URL is already native (`proxy/m3u8`, sessions),
//! so this module is the whole remaining provider surface.
//!
//! Transport is pluggable through [`Fetch`] because the site's
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
//! result list for the same query, or history rows resolve to
//! different shows.

pub mod parse;
pub mod parse_api;
use crate::scraper::fetch::Fetch;
use crate::scraper::provider::{
    encode_query, is_cloudflare_interstitial, BrowseHit, EpisodeRef, Provider, ProviderId,
    StreamSource,
};
pub use parse::{parse_browse, parse_detail_year, slug_search_term};
pub use parse_api::{extract_master_url, parse_episodes, parse_languages, preferred_embed};

use crate::error::{AniError, Result};

/// Provider origin. Kept overridable at the client level for tests.
pub const ANIDB_BASE: &str = "https://anidb.app";

/// One playable embed for an episode, by audio language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageEmbed {
    /// Provider language code: `jpn` for sub, `eng` for dub.
    pub language: String,
    /// The player page the master-playlist URL is extracted from.
    pub embed_url: String,
}

/// The anidb client: search, episode listing, and stream-URL
/// resolution over any [`Fetch`]. The walks reach it through
/// [`Provider`].
pub struct AnidbClient<F> {
    fetch: F,
    base: String,
}

impl<F: Fetch> AnidbClient<F> {
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

#[async_trait::async_trait]
impl<F: Fetch> Provider for AnidbClient<F> {
    fn id(&self) -> ProviderId {
        ProviderId::Anidb
    }

    /// Search the browse page. An interstitial or non-success status
    /// is a typed upstream error; a result-less page is `Ok(vec![])`
    /// only when it shows the browse shape — an unrecognized zero-hit
    /// body is a parse failure, never absence.
    ///
    /// # Errors
    /// [`AniError::Upstream`] when cloudflare or the site refuses,
    /// [`AniError::ParseFailed`] on an unrecognized zero-hit body,
    /// plus the transport errors of [`crate::scraper::fetch::Fetch::get`].
    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        let url = format!("{}/browse?q={}", self.base, encode_query(query));
        let body = self.content(&url).await?;
        parse_browse(&body)
    }

    /// List a show's episodes by slug.
    ///
    /// # Errors
    /// [`AniError::ParseFailed`] on a malformed slug or body, plus
    /// upstream/transport errors as in [`Self::search`].
    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        let id = parse::slug_numeric_id(slug).ok_or_else(|| AniError::ParseFailed {
            detail: format!("anidb slug without numeric tail: {slug}"),
        })?;
        let url = format!("{}/api/frontend/anime/{id}/episodes", self.base);
        let body = self.content(&url).await?;
        parse_episodes(&body)
    }

    /// Whether an episode's languages carry the requested mode's
    /// embed — the availability probes' one-request mode check. Only
    /// the languages row is fetched; no embed page.
    ///
    /// # Errors
    /// Upstream/transport errors as in [`Self::search`],
    /// [`AniError::ParseFailed`] on an unrecognized body.
    async fn has_mode(&self, episode_id: u64, mode: &str) -> Result<bool> {
        let url = format!("{}/api/frontend/episode/{episode_id}/languages", self.base);
        let body = self.content(&url).await?;
        let embeds = parse_languages(&body)?;
        Ok(preferred_embed(&embeds, mode).is_some())
    }

    /// Resolve an episode's master-playlist URL for `sub`/`dub`:
    /// languages → preferred embed → embed page → jwplayer `file:`.
    ///
    /// # Errors
    /// [`AniError::NoResults`] when no embed matches the mode or the
    /// embed page carries no playlist, plus upstream/transport errors.
    async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<StreamSource> {
        let url = format!("{}/api/frontend/episode/{episode_id}/languages", self.base);
        let body = self.content(&url).await?;
        let embeds = parse_languages(&body)?;
        let embed = preferred_embed(&embeds, mode).ok_or(AniError::NoResults)?;
        let embed_body = self.content(&embed.embed_url).await?;
        let master_url = extract_master_url(&embed_body).ok_or(AniError::NoResults)?;
        // anidb's CDN checks no referer; the proxy sends none.
        Ok(StreamSource {
            master_url,
            referer: None,
            subtitles: Vec::new(),
        })
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
    async fn detail_year(&self, slug: &str) -> Result<Option<u32>> {
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

    async fn playlist(&self, url: &str, _referer: Option<&str>) -> Result<String> {
        self.content(url).await
    }

    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        self.fetch.last_attempt_at()
    }
}

#[cfg(test)]
#[path = "anidb_test.rs"]
mod tests;
