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
pub mod detail;
pub mod embed;
pub mod megaplay;
pub mod parse;
pub use ajax::{
    last_readable_index, parse_episode_list, parse_server_listing, parse_servers, servers_for,
    ServerEmbed, ServerListing,
};
pub use detail::parse_detail_year;
pub use embed::{decode_embed, embed_origin, EmbedPayload};
pub use megaplay::{lang_of_track, media_id, parse_sources, sources_url};
pub use parse::{parse_search, slug_id};

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

/// How long one server's chain — its page, its sources answer, its
/// master and the chosen rendition — may take before the walk steps
/// to the next server. Sized against the walk's own budget: a
/// provider's whole attempt has twenty seconds, of which the search,
/// the entry and the listings take about a second and a half when
/// the site is healthy, and a host that holds a connection open
/// costs the transport its full ten-second wait. Six seconds a
/// server lets three servers be tried inside the attempt, where an
/// unbounded server's single stalled master would spend the whole
/// attempt with the site's other servers unasked — which is the
/// outage of 2026-09-12 as the walk would have met it.
///
/// The bound holds time back for the servers still to come, so it
/// applies to every server but the last. The last server — a lone
/// server included — runs on whatever the attempt has left: the
/// walk above the client cancels the attempt at its own deadline,
/// and the transport below it gives up on any one request at its
/// own wait, so an unanswered last server ends the attempt on one
/// of those two bounds rather than on this one. Holding the last
/// server to this bound would cut off a chain that is merely slower
/// than six seconds, with nobody left to give the time to.
pub const SERVER_ATTEMPT_BUDGET: std::time::Duration = std::time::Duration::from_secs(6);

/// The hianime client: search, episode listing, and stream-URL
/// resolution over any [`Fetch`]. The walks reach it through
/// [`Provider`].
pub struct HianimeClient<F> {
    fetch: F,
    base: String,
    server_budget: std::time::Duration,
    /// The instant of the attempt that produced the failure the
    /// server walk last kept, with the transport's stamp as it stood
    /// when the walk ended: the kept failure may come from an earlier
    /// server than the walk's last fetch, and the gate must be told
    /// when that failure was observed, not when the walk finished. A
    /// fetch after the walk moves the transport's stamp, and the
    /// kept instant no longer applies.
    kept_attempt_at: std::sync::Mutex<Option<KeptAttempt>>,
}

/// See [`HianimeClient::kept_attempt_at`].
#[derive(Clone, Copy)]
struct KeptAttempt {
    at: tokio::time::Instant,
    transport_then: Option<tokio::time::Instant>,
}

impl<F: Fetch> HianimeClient<F> {
    /// A client against the production origin.
    pub fn new(fetch: F) -> Self {
        Self {
            fetch,
            base: HIANIME_BASE.to_string(),
            server_budget: SERVER_ATTEMPT_BUDGET,
            kept_attempt_at: std::sync::Mutex::new(None),
        }
    }

    /// A client against `base` — a stub origin in tests, a mirror
    /// when the canonical domain is filtered.
    pub fn with_base(fetch: F, base: &str) -> Self {
        Self {
            fetch,
            base: base.to_string(),
            server_budget: SERVER_ATTEMPT_BUDGET,
            kept_attempt_at: std::sync::Mutex::new(None),
        }
    }

    /// Replace the per-server budget — the seam the stalled-host
    /// tests drive; production keeps [`SERVER_ATTEMPT_BUDGET`].
    #[cfg(test)]
    pub(crate) fn with_server_budget(mut self, budget: std::time::Duration) -> Self {
        self.server_budget = budget;
        self
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
        // The listing's attempt, read before any host's fetch moves
        // the transport's stamp: the doubt below is this attempt's
        // finding, and the gate is told when it was observed.
        let listing_at = self.fetch.last_attempt_at();
        // The mode's rows the client could not read are a parse
        // failure before any embed page is asked for.
        listing.mode_readable(mode)?;
        // A row of the mode the client could not read, or a row typed
        // with a mode it does not know, stays a doubt through the
        // walk: should no server serve a stream, the verdict is the
        // mode's uncertainty, never the answered absence — that row
        // may have been the server. The doubt rides with the listing
        // attempt's instant, as a host's failure rides with its own.
        let doubt_at = listing.uncertain_for(mode).then_some(listing_at);
        let servers = listing.servers;
        // The first server whose page yields a stream that serves the
        // play wins ([`Self::read_server`], [`Self::resolved`]);
        // every other outcome is stepped over and remembered, and
        // the loudest surfaces when no server served a stream
        // ([`weightier`]): a rate limit above everything, since it
        // alone opens the breaker's advertised pause at once; then a
        // host that refused or failed, which speaks for the provider
        // and is what the shared walk stops on; then a page, a
        // sources answer or a master the client cannot use — the key
        // does not open it, the source is nothing the transport
        // fetches, the sources came back encrypted, the master is not
        // a playlist — which is the site having changed and the
        // client no longer reading it, transient to the shared walk;
        // then a dropped connection; then an answered status. A page
        // of neither shape says what its host does
        // ([`payload_missing_verdict`]): from a host the client reads
        // it is the site having changed shape, a parse failure like a
        // blob the key no longer opens; from a host the client never
        // read it says nothing and is stepped over, and an episode
        // with only those has no stream. A fetch the gate refuses is
        // not the host's weather at all but the gate speaking — a
        // breaker opened, or a pause began, between two fetches of a
        // background walk — and ends the walk as it is: no later
        // server is asked, and the shared walk stops on it rather
        // than recording a dead end.
        //
        // Each server's chain but the last has its own bound
        // ([`SERVER_ATTEMPT_BUDGET`]): a host that holds a connection
        // open without answering would otherwise spend, on one server,
        // the time the walk's attempt had left for the rest, and the
        // attempt would time out with a healthy server unasked. A
        // server cut off at its bound is stepped over like one whose
        // connection dropped; the transport's child is killed with
        // the future it ran under. The last server the walk can use
        // — the last on a host the client reads
        // ([`last_readable_index`]), since the hosts the listing
        // trails behind it are stepped over unread — has no rest to
        // hold time back for and runs on the attempt's remainder,
        // bounded by the walk around this client — the attempt's
        // deadline above, the transport's per-request wait below — so
        // a chain slower than the bound is still served when it is
        // the last chain the walk can read.
        let mut kept: Option<(AniError, Option<tokio::time::Instant>)> = None;
        let ordered = servers_for(&servers, mode);
        let unbounded = last_readable_index(&ordered);
        for (i, server) in ordered.into_iter().enumerate() {
            let chain = async {
                let payload = self.read_server(server).await?;
                self.resolved(server, payload, quality).await
            };
            let outcome = if unbounded == Some(i) {
                chain.await
            } else {
                match tokio::time::timeout(self.server_budget, chain).await {
                    Ok(outcome) => outcome,
                    Err(_elapsed) => Err(AniError::Timeout),
                }
            };
            // The attempt that produced this outcome, read before the
            // next server's fetch moves the transport's stamp.
            let at = self.fetch.last_attempt_at();
            let weather = match outcome {
                Ok(stream) => return Ok(stream),
                Err(AniError::NoResults) => match payload_missing_verdict(&server.embed_url) {
                    Some(e) => e,
                    None => continue,
                },
                Err(AniError::GateRefused) => return Err(AniError::GateRefused),
                Err(e) => e,
            };
            kept = Some(match kept.take() {
                Some(so_far) => weightier(so_far, (weather, at)),
                None => (weather, at),
            });
        }
        let (verdict, at) = final_verdict(kept, doubt_at, mode);
        if let Some(at) = at {
            *self.kept_attempt_at.lock().expect("kept attempt lock") = Some(KeptAttempt {
                at,
                transport_then: self.fetch.last_attempt_at(),
            });
        }
        Err(verdict)
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
        let transport = self.fetch.last_attempt_at();
        match *self.kept_attempt_at.lock().expect("kept attempt lock") {
            // The walk's kept failure, while no fetch has moved the
            // transport's stamp since the walk ended.
            Some(kept) if kept.transport_then == transport => Some(kept.at),
            _ => transport,
        }
    }
}

/// The failure to keep when two of an episode's hosts failed, by
/// what the walk and the breaker make of it: a rate limit outranks
/// everything, since it alone opens the advertised pause at once; a
/// block — a refusal-shaped status or a server error — outranks a
/// page the client could not read, since the block speaks for the
/// provider, the breaker must hear it, and the shared walk stops on
/// it, while a parse failure is transient to that walk; a parse
/// failure outranks a dropped connection or a timeout, since it says
/// the client no longer reads the site; and those outrank an
/// answered status, since a server never heard from may carry the
/// stream and the transport failure is what moves the walk on,
/// while an answered status is that host's own dead end; between
/// two of a rank the one seen first stays.
fn weightier<T>(kept: (AniError, T), next: (AniError, T)) -> (AniError, T) {
    if weather_rank(&next.0) > weather_rank(&kept.0) {
        next
    } else {
        kept
    }
}

/// What a page without the payload marker says about its host: a
/// host the client reads ([`ajax::readable`]) has changed shape under
/// the client, which is a parse failure naming the host, ranked like
/// any other; a host the client never read says nothing, and the
/// walk steps over it.
fn payload_missing_verdict(embed_url: &str) -> Option<AniError> {
    if !ajax::readable(embed_url) {
        return None;
    }
    let host = url::Url::parse(embed_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| embed_url.to_string());
    Some(AniError::ParseFailed {
        detail: format!("hianime embed page on {host}: the payload marker is missing"),
    })
}

/// The verdict when no server served a stream: the loudest failure
/// kept across the hosts, lifted to a parse failure when the mode had
/// a row the client could not read — that row may have been the
/// playable server, so the walk's end is the site having changed
/// shape, not the episode having no stream — and the answered
/// absence only when every page merely lacked the payload and every
/// row was read. The lift ranks like any parse failure, so a
/// provider block still outranks it. Whatever rides with a failure —
/// the instant of the attempt that produced it — rides with the
/// verdict: the kept failure's, or the listing attempt's when the
/// doubt stands (`doubt` carries it when the mode had such a row),
/// and nothing when the absence is the verdict.
fn final_verdict<T: Default>(
    kept: Option<(AniError, T)>,
    doubt: Option<T>,
    mode: &str,
) -> (AniError, T) {
    let doubt = doubt.map(|at| {
        (
            AniError::ParseFailed {
                detail: format!("hianime {mode} servers: a row the client could not read"),
            },
            at,
        )
    });
    match (kept, doubt) {
        (Some(kept), Some(doubt)) => weightier(kept, doubt),
        (Some(kept), None) => kept,
        (None, Some(doubt)) => doubt,
        (None, None) => (AniError::NoResults, T::default()),
    }
}

/// How loudly a host's failure speaks: a rate limit above all, any
/// other provider block above a parse failure — the block is what
/// the shared walk stops on, the parse failure is transient to it —
/// a parse failure above a transport failure, a transport failure
/// above the rest.
fn weather_rank(weather: &AniError) -> u8 {
    match weather {
        AniError::RateLimited { .. } | AniError::Upstream { status: 429 } => 4,
        w if w.is_provider_block() => 3,
        AniError::ParseFailed { .. } => 2,
        AniError::Network | AniError::Timeout => 1,
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
