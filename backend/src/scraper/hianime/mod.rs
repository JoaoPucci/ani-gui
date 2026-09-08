//! Native client for hianime — the second stream provider, and the
//! one `ani-cli` moved to when anidb.app went dark in September 2026.
//!
//! The flow: a search page (HTML) → per-entry episode list and
//! per-episode server list (JSON envelopes wrapping HTML, answered
//! only to `X-Requested-With`) → a server's base64 hash decodes to an
//! embed URL on a separate host → the embed page carries a blob that
//! XOR-decodes to the player's JSON, whose `src` is the master
//! playlist and whose `subtitles` are sidecar tracks. The CDN checks
//! the embed host's origin as `Referer` on every playlist fetch.
//!
//! An episode lists several servers, named by slot, and the site
//! moves the slots between embed hosts; only some hosts' pages carry
//! the blob. The client tries the servers it can read first and
//! takes the first page that decodes.

pub mod ajax;
pub mod embed;
pub mod parse;
pub use ajax::{parse_episode_list, parse_servers, servers_for, ServerEmbed};
pub use embed::{decode_embed, embed_origin, EmbedPayload, SubtitleTrack};
pub use parse::{parse_detail_year, parse_search, slug_id};

use crate::error::{AniError, Result};
use crate::scraper::fetch::{Fetch, FetchRequest};
use crate::scraper::provider::{
    encode_query, is_cloudflare_interstitial, BrowseHit, EpisodeRef, Provider, ProviderId,
    StreamSource,
};

/// Provider origin. Overridable at the client level for tests, and
/// worth keeping overridable for real: the site's domain churns and
/// is filtered per ISP.
pub const HIANIME_BASE: &str = "https://hianime.at";

/// The hianime client: search, episode listing, and stream-URL
/// resolution over any [`Fetch`]. The walks reach it through
/// [`Provider`].
pub struct HianimeClient<F> {
    fetch: F,
    base: String,
}

impl<F: Fetch> HianimeClient<F> {
    /// A client against the production origin.
    pub fn new(fetch: F) -> Self {
        Self {
            fetch,
            base: HIANIME_BASE.to_string(),
        }
    }

    /// A client against `base` — a stub origin in tests, a mirror
    /// when the canonical domain is filtered.
    pub fn with_base(fetch: F, base: &str) -> Self {
        Self {
            fetch,
            base: base.to_string(),
        }
    }

    /// The transport this client fetches through.
    pub fn transport(&self) -> &F {
        &self.fetch
    }

    /// The site's AJAX endpoints answer only to a request that says
    /// it is one.
    fn ajax(&self, path: &str) -> FetchRequest {
        FetchRequest::get(format!("{}{path}", self.base))
            .header("X-Requested-With", "XMLHttpRequest")
    }

    /// Perform `req` and hand back content, refusing challenge pages
    /// and non-success statuses as typed upstream errors.
    async fn content(&self, req: &FetchRequest) -> Result<String> {
        let resp = self.fetch.fetch(req).await?;
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

    /// An episode's servers, as the site lists them.
    async fn servers(&self, episode_id: u64) -> Result<Vec<ServerEmbed>> {
        let body = self
            .content(&self.ajax(&format!(
                "/api/theme/episode/servers?episodeId={episode_id}"
            )))
            .await?;
        parse_servers(&body)
    }
}

#[async_trait::async_trait]
impl<F: Fetch> Provider for HianimeClient<F> {
    fn id(&self) -> ProviderId {
        ProviderId::Hianime
    }

    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        let url = format!("{}/search?keyword={}", self.base, encode_query(query));
        let body = self.content(&FetchRequest::get(url)).await?;
        parse_search(&body)
    }

    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        let id = slug_id(slug).ok_or_else(|| AniError::ParseFailed {
            detail: format!("hianime slug without numeric tail: {slug}"),
        })?;
        let body = self
            .content(&self.ajax(&format!("/api/theme/episode/list/{id}")))
            .await?;
        parse_episode_list(&body)
    }

    async fn has_mode(&self, episode_id: u64, mode: &str) -> Result<bool> {
        let servers = self.servers(episode_id).await?;
        Ok(!servers_for(&servers, mode).is_empty())
    }

    async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<StreamSource> {
        let servers = self.servers(episode_id).await?;
        // The first server whose page decodes wins; every other
        // outcome is stepped over and remembered, and surfaces only
        // when no server served a stream — in the order of what it
        // says about the client. A page that carries the payload the
        // key does not open is the site having changed, and the
        // client no longer reading it: a parse failure, ahead of a
        // host that refused or could not be reached. Among those, a
        // block outranks an answered status or a dropped connection
        // ([`weightier`]), so one host's not-found cannot hide the
        // next host's refusal from the breaker. A page without the
        // payload is a host the client does not read at all, and an
        // episode with only those has no stream.
        let mut broken: Option<AniError> = None;
        let mut weather: Option<AniError> = None;
        for server in servers_for(&servers, mode) {
            // The embed host checks that the site sent the viewer.
            let embed = FetchRequest::get(server.embed_url.clone())
                .header("Referer", format!("{}/", self.base));
            let page = match self.content(&embed).await {
                Ok(page) => page,
                Err(e) => {
                    weather = Some(match weather.take() {
                        Some(kept) => weightier(kept, e),
                        None => e,
                    });
                    continue;
                }
            };
            match decode_embed(&page) {
                Ok(payload) => {
                    return Ok(StreamSource {
                        master_url: payload.src,
                        referer: embed_origin(&server.embed_url),
                    })
                }
                Err(AniError::NoResults) => continue,
                Err(e @ AniError::ParseFailed { .. }) => {
                    broken.get_or_insert(e);
                }
                Err(e) => return Err(e),
            }
        }
        Err(broken.or(weather).unwrap_or(AniError::NoResults))
    }

    async fn playlist(&self, url: &str, referer: Option<&str>) -> Result<String> {
        let mut req = FetchRequest::get(url);
        if let Some(r) = referer {
            req = req.header("Referer", r);
        }
        self.content(&req).await
    }

    async fn detail_year(&self, slug: &str) -> Result<Option<u32>> {
        let url = format!("{}/{slug}", self.base);
        match self.content(&FetchRequest::get(url)).await {
            Ok(body) => Ok(parse_detail_year(&body)),
            Err(AniError::Upstream { status })
                if !AniError::Upstream { status }.is_provider_block() =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        self.fetch.last_attempt_at()
    }
}

/// The weather to keep when two of an episode's hosts failed: a
/// provider block — a refusal-shaped status or a rate limit — outranks
/// an answered status or a dropped connection, since the block speaks
/// for the provider and the breaker must hear it; between two of a
/// rank the one seen first stays.
fn weightier(kept: AniError, next: AniError) -> AniError {
    if next.is_provider_block() && !kept.is_provider_block() {
        next
    } else {
        kept
    }
}

#[cfg(test)]
#[path = "hianime_test.rs"]
mod tests;

#[cfg(test)]
#[path = "hianime_prop_test.rs"]
mod prop_tests;
