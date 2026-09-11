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
//! moves the slots between embed hosts, and the hosts differ in how
//! their pages expose the stream: zokoanime's carry the blob;
//! megaplay's carry no payload, only the media id its player asks
//! the site's sources endpoint for. The client reads a page by its
//! shape, not the host's name, tries the servers on hosts it can
//! read first, and takes the first that yields a stream.

pub mod ajax;
pub mod embed;
pub mod megaplay;
pub mod parse;
pub use ajax::{
    parse_episode_list, parse_server_listing, parse_servers, servers_for, ServerEmbed,
    ServerListing,
};
pub use embed::{decode_embed, embed_origin, EmbedPayload};
pub use megaplay::{lang_of_track, media_id, parse_sources, sources_url};
pub use parse::{parse_detail_year, parse_search, slug_id};

use crate::error::{AniError, Result};
use crate::scraper::fetch::{Fetch, FetchRequest};
use crate::scraper::provider::{
    encode_query, is_cloudflare_interstitial, BrowseHit, EpisodeRef, Provider, ProviderId,
    ResolvedStream, StreamSource,
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

    /// A server's stream, read from its embed page by the page's
    /// shape: a page carrying the payload decodes in place; a page
    /// without it that names its media — megaplay's — has the site
    /// asked for the sources, with the page's own origin as the
    /// referer and the header its player sends; a page with neither
    /// shape is a host the client does not read, an answered
    /// "nothing here".
    ///
    /// # Errors
    /// [`AniError::NoResults`] for a page of neither shape; the
    /// fetches' own refusals and transport failures; a parse failure
    /// for a payload or a sources answer the client cannot use.
    async fn read_server(&self, server: &ServerEmbed) -> Result<EmbedPayload> {
        // The embed host checks that the site sent the viewer.
        let embed = FetchRequest::get(server.embed_url.clone())
            .header("Referer", format!("{}/", self.base));
        let page = self.content(&embed).await?;
        match decode_embed(&page) {
            Err(AniError::NoResults) => {}
            decoded => return decoded,
        }
        let Some(id) = media_id(&page) else {
            return Err(AniError::NoResults);
        };
        let url = sources_url(&server.embed_url, id).ok_or_else(|| AniError::ParseFailed {
            detail: "megaplay embed URL without an origin".into(),
        })?;
        let sources = FetchRequest::get(url)
            .header(
                "Referer",
                embed_origin(&server.embed_url).unwrap_or_default(),
            )
            .header("X-Requested-With", "XMLHttpRequest");
        parse_sources(&self.content(&sources).await?)
    }

    /// A server's stream, validated the way the episode step needs
    /// it: the master fetched with the embed host's origin as the
    /// referer and required to be a playlist, the quality selected
    /// from it, and the chosen rendition fetched and required to be
    /// one too — the shared selection every provider's episode step
    /// runs ([`crate::scraper::hls::stream_url`]), run here so a
    /// server that does not serve the play is one the walk steps
    /// over. A payload names a host that may be down — on 2026-09-12
    /// zokoanime's playlist host was, for every episode, while
    /// megaplay's served — and a master that answers can still front
    /// a rendition that refuses; either taken unasked ends the walk
    /// on a stream the episode step then fails on, sending the
    /// resolver to the next alias and never the next server.
    ///
    /// # Errors
    /// The fetches' own refusals and transport failures; a parse
    /// failure for a master that is not a playlist, as the episode
    /// step would report it.
    async fn resolved(
        &self,
        server: &ServerEmbed,
        payload: EmbedPayload,
        quality: &str,
    ) -> Result<ResolvedStream> {
        let source = StreamSource {
            master_url: payload.src,
            referer: embed_origin(&server.embed_url),
            subtitles: payload.subtitles,
        };
        let url = crate::scraper::hls::stream_url(self, &source, quality).await?;
        Ok(ResolvedStream {
            url,
            referer: source.referer,
            subtitles: source.subtitles,
        })
    }

    /// An episode's servers, as the site lists them, with the
    /// listing's uncertainty per mode.
    async fn servers(&self, episode_id: u64) -> Result<ServerListing> {
        let body = self
            .content(&self.ajax(&format!(
                "/api/theme/episode/servers?episodeId={episode_id}"
            )))
            .await?;
        parse_server_listing(&body)
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
        // A mode with rows the client could not read is a parse
        // failure, never the absence the mode probe would persist.
        self.servers(episode_id).await?.mode_readable(mode)
    }

    async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<StreamSource> {
        // The walk of the servers at the adaptive quality: the master
        // that answered, from the first server that serves.
        let stream = self.stream_for(episode_id, mode, "best").await?;
        Ok(StreamSource {
            master_url: stream.url,
            referer: stream.referer,
            subtitles: stream.subtitles,
        })
    }

    async fn stream_for(
        &self,
        episode_id: u64,
        mode: &str,
        quality: &str,
    ) -> Result<ResolvedStream> {
        let listing = self.servers(episode_id).await?;
        // The mode's rows the client could not read are a parse
        // failure before any embed page is asked for.
        listing.mode_readable(mode)?;
        let servers = listing.servers;
        // The first server whose page yields a stream that serves the
        // play wins ([`Self::read_server`], [`Self::resolved`]);
        // every other outcome is stepped over and remembered, and
        // the loudest surfaces when no server served a stream
        // ([`weightier`]): a rate limit above everything, since it
        // alone opens the breaker's advertised pause at once; then a
        // page, a sources answer or a master the client cannot use —
        // the key does not open it, the source is nothing the
        // transport fetches, the sources came back encrypted, the
        // master is not a playlist — which is the site having
        // changed and the client no longer reading it; then a host
        // that refused or failed, which speaks for the provider; then
        // an answered status or a dropped connection. A page of
        // neither shape is a host the client does not read at all,
        // and an episode with only those has no stream.
        let mut kept: Option<AniError> = None;
        for server in servers_for(&servers, mode) {
            let outcome = match self.read_server(server).await {
                Ok(payload) => self.resolved(server, payload, quality).await,
                Err(e) => Err(e),
            };
            match outcome {
                Ok(stream) => return Ok(stream),
                Err(AniError::NoResults) => {}
                Err(e) => {
                    kept = Some(match kept.take() {
                        Some(so_far) => weightier(so_far, e),
                        None => e,
                    });
                }
            }
        }
        Err(kept.unwrap_or(AniError::NoResults))
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

/// The failure to keep when two of an episode's hosts failed, by
/// what the walk and the breaker make of it: a rate limit outranks
/// everything, since it alone opens the advertised pause at once; a
/// page the client could not read — a parse failure — outranks any
/// other block, since it says the client no longer reads the site;
/// a block — a refusal-shaped status or a server error — outranks an
/// answered status or a dropped connection, since the block speaks
/// for the provider and the breaker must hear it; between two of a
/// rank the one seen first stays.
fn weightier(kept: AniError, next: AniError) -> AniError {
    if weather_rank(&next) > weather_rank(&kept) {
        next
    } else {
        kept
    }
}

/// How loudly a host's failure speaks: a rate limit above all, a
/// parse failure above any other block, a block above the rest.
fn weather_rank(weather: &AniError) -> u8 {
    match weather {
        AniError::RateLimited { .. } | AniError::Upstream { status: 429 } => 3,
        AniError::ParseFailed { .. } => 2,
        w if w.is_provider_block() => 1,
        _ => 0,
    }
}

#[cfg(test)]
#[path = "hianime_test.rs"]
mod tests;

#[cfg(test)]
#[path = "hianime_prop_test.rs"]
mod prop_tests;

#[cfg(test)]
#[path = "megaplay_test.rs"]
mod megaplay_tests;

#[cfg(test)]
#[path = "megaplay_prop_test.rs"]
mod megaplay_prop_tests;
