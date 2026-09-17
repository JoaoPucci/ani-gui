//! Native client for hianime — the second stream provider, and the
//! one `ani-cli` moved to when anidb.app went dark in September 2026.
//!
//! The flow: a search page (HTML) → per-entry episode list and
//! per-episode server list (JSON envelopes wrapping HTML, answered
//! only to `X-Requested-With`) → a server's base64 hash decodes to an
//! embed URL on a separate host → the embed page carries a blob that
//! XOR-decodes to the player's JSON, whose `src` is the master
//! playlist and whose `subtitles` are sidecar tracks. The CDN checks
//! the origin of the page the payload came from as `Referer` on every
//! playlist fetch — the host that served it, which a redirect can
//! make something other than the host the listing named.
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
    chain_reserve, parse_episode_list, parse_server_listing, parse_servers, remainder_index,
    servers_for, ServerCaps, ServerEmbed, ServerListing, CHAIN_REQUESTS,
};
pub use detail::parse_detail_year;
pub use embed::{decode_embed, embed_origin, EmbedPayload};
pub use megaplay::{lang_of_track, media_id, parse_sources, served_cdn, sources_url};
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
/// than six seconds, with nobody left to give the time to. And the
/// bound is the most a bounded server gets, not the least: told the
/// attempt's deadline, the walk gives each bounded server its share
/// of what the attempt has left once a whole chain's worth is held
/// back for the last ([`ajax::ServerCaps`],
/// [`ajax::chain_reserve`]), so stalled servers cannot spend the
/// remainder that last server runs on. A chain's worth and not this
/// bound: the chain is four requests deep, and a bound spread over
/// four of them is less than a loaded host takes to answer. What is
/// held back is a share of the attempt and never the whole of what
/// is left of it: a bounded server keeps at least the share it would
/// have if what remained were the chain's worth exactly, that worth
/// split among the servers ahead and the last together. When the
/// attempt cannot fund both that share and the chain's worth behind
/// it, the chain's worth is what gives way, so no bounded server is
/// skipped — or cut off a moment into its chain — for want of a
/// window while the attempt still runs. That floor under the shares
/// is settled at the walk's first server and holds for the walk: a
/// floor worked out again for each server climbs as the servers
/// ahead run out, and the last of them would then be handed a wider
/// window than the first had out of less.
pub const SERVER_ATTEMPT_BUDGET: std::time::Duration = std::time::Duration::from_secs(6);

/// The hianime client: search, episode listing, and stream-URL
/// resolution over any [`Fetch`]. The walks reach it through
/// [`Provider`].
pub struct HianimeClient<F> {
    fetch: F,
    base: String,
    server_budget: std::time::Duration,
    /// The deadline of the walk's attempt this client runs under,
    /// when the walk has told it one ([`Provider::bound_attempt`]).
    attempt_deadline: std::sync::Mutex<Option<tokio::time::Instant>>,
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

/// A page the client fetched and the URL it came from. The two
/// travel together because a redirect makes them differ: the embed
/// URLs the listing hands out move hosts, and the page that arrives
/// is the one whose host answers for it.
struct Page {
    /// The page's content.
    body: String,
    /// The URL the transport ended on, the request's own when
    /// nothing redirected.
    url: String,
}

impl<F: Fetch> HianimeClient<F> {
    /// A client against the production origin.
    pub fn new(fetch: F) -> Self {
        Self {
            fetch,
            base: HIANIME_BASE.to_string(),
            server_budget: SERVER_ATTEMPT_BUDGET,
            attempt_deadline: std::sync::Mutex::new(None),
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
            attempt_deadline: std::sync::Mutex::new(None),
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

    /// Perform `req` and hand back the page with the URL it came
    /// from, refusing challenge pages and non-success statuses as
    /// typed upstream errors.
    async fn page(&self, req: &FetchRequest) -> Result<Page> {
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
        Ok(Page {
            body: resp.body,
            url: resp.url,
        })
    }

    /// [`Self::page`]'s body alone — the site's own endpoints, whose
    /// every caller parses the content and cares about no host but
    /// the one it asked.
    async fn content(&self, req: &FetchRequest) -> Result<String> {
        self.page(req).await.map(|page| page.body)
    }

    /// A server's stream, read from its embed page by the page's
    /// shape ([`Self::read_page`]), beside the URL the page came
    /// from: the transport follows redirects, so the host that served
    /// the page is what the walk judges the outcome by, what the
    /// reading of the page is keyed on, and it is not always the host
    /// the listing named. When nothing was served the listing's URL
    /// is all the failure has to name.
    ///
    /// The outcome's errors are [`Self::read_page`]'s and the embed
    /// fetch's own refusals and transport failures.
    async fn read_server(&self, server: &ServerEmbed) -> (Result<EmbedPayload>, String) {
        // The embed host checks that the site sent the viewer.
        let embed = FetchRequest::get(server.embed_url.clone())
            .header("Referer", format!("{}/", self.base));
        let page = match self.page(&embed).await {
            Ok(page) => page,
            Err(e) => return (Err(e), server.embed_url.clone()),
        };
        (self.read_page(&page.url, &page.body).await, page.url)
    }

    /// The stream a fetched embed page yields, by the page's shape: a
    /// page carrying the payload decodes in place; a page without it
    /// that names its media — megaplay's — has the site asked for the
    /// sources, with the page's own origin as the referer and the
    /// header its player sends; a page with neither shape is a host
    /// the client does not read, an answered "nothing here".
    ///
    /// `served_by` is the URL the page came from, which is where the
    /// sources are asked for, and whose CDN family decides whether
    /// they are asked for at all ([`ajax::readable`]): megaplay's
    /// endpoint sits under the
    /// origin of the page whose player asks it, the site serves that
    /// player from its own host and from numbered mirrors, and a
    /// redirect between them leaves the listing's URL naming a host
    /// that served no page and answers for no media of it. It is the
    /// URL the walk judges the page by ([`Self::read_server`]) and
    /// the one the stream's referer is taken from
    /// ([`Self::resolved`]), so the whole of what a page yields keys
    /// on the host that served it; the listing's URL keys what is
    /// decided before the page exists — which servers are tried, in
    /// what order, and what is asked for.
    ///
    /// # Errors
    /// [`AniError::NoResults`] for a page of neither shape; the
    /// sources fetch's own refusals and transport failures; a parse
    /// failure for a payload or a sources answer the client cannot
    /// use.
    async fn read_page(&self, served_by: &str, page: &str) -> Result<EmbedPayload> {
        match decode_embed(page) {
            Err(AniError::NoResults) => {}
            decoded => return decoded,
        }
        let Some(id) = media_id(page) else {
            return Err(AniError::NoResults);
        };
        // A page served for a CDN family whose segments the proxy
        // cannot pass through is a server this client gets no stream
        // from, so the site is never asked what that family streams:
        // the answer would name playlists that validate and segments
        // the player cannot decode, and taking it would end the walk
        // on a stream that plays nothing. Answered like a host the
        // client does not read, which the walk steps over
        // ([`ajax::readable`], [`payload_missing_verdict`]).
        if !served_cdn(served_by) {
            return Err(AniError::NoResults);
        }
        let url = sources_url(served_by, id).ok_or_else(|| AniError::ParseFailed {
            detail: "megaplay embed page from a URL without an origin".into(),
        })?;
        let sources = FetchRequest::get(url)
            .header("Referer", embed_origin(served_by).unwrap_or_default())
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
        served_by: &str,
        payload: EmbedPayload,
        quality: &str,
    ) -> Result<ResolvedStream> {
        // The referer is the origin of the page the stream was read
        // from, the host that served it, as the walk judges the page.
        let source = StreamSource {
            master_url: payload.src,
            referer: embed_origin(served_by),
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
        // Each server's chain but one has its own bound: a host that
        // holds a connection open without answering would otherwise
        // spend, on one server, the time the walk's attempt had left
        // for the rest, and the attempt would time out with a healthy
        // server unasked. A server cut off at its bound is stepped
        // over like one whose connection dropped; the transport's
        // child is killed with the future it ran under. One server has
        // no rest to hold time back for and runs on the attempt's
        // remainder, bounded by the walk around this client — the
        // attempt's deadline above, the transport's per-request wait
        // below — so a chain slower than the bound is still served
        // when it is that one ([`remainder_index`]): the last on a
        // host the client names as one it reads, since the hosts the
        // listing trails behind it are stepped over unread; or, when
        // no listed host is named, the listing's last server, since a
        // page is read by its shape from any host and which one reads
        // is not known before the fetch. The limit that accepts: a
        // server on an unnamed host ahead of that position keeps the
        // bound even when its page would read, so that a trailing host
        // the client never read takes no time from a named one.
        //
        // The bound is [`SERVER_ATTEMPT_BUDGET`] at most, and less
        // when the attempt's deadline says so ([`ServerCaps`]): the
        // attempt above this client bounds the search, the candidate,
        // the listings and the servers together, and part of it is
        // spent before the first server is asked, so a fixed bound per
        // server would let a few stalled servers spend what the
        // remainder's server needed. Told the deadline
        // ([`Provider::bound_attempt`]), the walk gives each bounded
        // server its share of what remains once the reserve is held
        // back for the remainder's server, and a stalled server is
        // cut off the sooner for it. A client run outside an attempt
        // knows no deadline and keeps the fixed bound.
        //
        // The reserve is a whole chain's worth ([`chain_reserve`]),
        // not one bound. A chain is four requests deep — the embed
        // page, the sources answer, the master and the rendition —
        // and each waits on the one before it, so a bound held back
        // is a bound spread over four. A CDN under load answers each
        // of them in a second or two, well inside the transport's
        // own wait and past a quarter of the bound, and a reserve of
        // one bound would leave that chain cancelled with the
        // attempt on the last server the walk had, every request it
        // made having been answered.
        //
        // The reserve gives way before a share does, and by as much
        // as it has to. What the walk has left is the attempt less
        // the search, the candidate and the listings, and a slow
        // site can leave it the reserve and nothing over — or the
        // reserve and a millisecond. Held back whole in either case,
        // the reserve is the whole remainder, or all but that
        // millisecond of it, and a server ahead of the remainder's
        // is capped at nothing or at the millisecond: stepped over
        // unasked, or cut off a request into a chain of four, while
        // the attempt still has time to spend — a healthy server not
        // tried, and the episode lost when the remainder's server is
        // the dead one. The second is the worse of the two for being
        // the narrower window: more of the attempt left, and less of
        // it given to the same healthy server.
        //
        // So a bounded server keeps at least the share it would have
        // if what remained were the reserve exactly — the reserve
        // split evenly among the servers ahead and the remainder's
        // server alike. Below the reserve that split is the whole
        // rule. Just above it the split still governs and the
        // reserve shrinks by what the servers ahead are owed, no
        // more. Once what is over the reserve has grown back to that
        // share, the reserve is held back whole again and the share
        // is what is over, as it was ([`ServerCaps`]).
        //
        // That floor is worked out once, here, out of what the
        // attempt has left before the first server is asked and how
        // many servers run ahead of the remainder's. What remains
        // and what is still ahead are read afresh for each server,
        // so a server that answered early leaves what it did not
        // spend to the ones behind it; the floor under those shares
        // is not, because it climbs when it is. The reserve split
        // with one server ahead and the remainder's is half of it
        // where, with two ahead and the remainder's, it was a third,
        // so the second of two stalled servers would be handed a
        // wider window than the first had out of an attempt with
        // less left in it — and the pair would spend between them a
        // reserve this walk had set aside and could afford, leaving
        // a chain that was answering to be cancelled with the
        // attempt.
        let mut kept: Option<(AniError, Option<tokio::time::Instant>)> = None;
        let ordered = servers_for(&servers, mode);
        let unbounded = remainder_index(&ordered);
        let deadline = *self.attempt_deadline.lock().expect("attempt deadline");
        let remaining =
            || deadline.map(|at| at.saturating_duration_since(tokio::time::Instant::now()));
        let caps = ServerCaps::for_walk(
            self.server_budget,
            chain_reserve(self.server_budget),
            remaining(),
            unbounded.unwrap_or(0),
        );
        for (i, server) in ordered.into_iter().enumerate() {
            // A page is judged by the host that served it, which is
            // not always the host the listing named: the transport
            // follows redirects, and an embed URL that moves hosts
            // ends on a page whose own origin is the one the CDN
            // checks and whose own host decides whether this client
            // reads pages of that shape at all ([`Self::read_server`]
            // hands that host back beside the outcome). The listing's
            // URL keys what is decided before the page exists — which
            // servers are tried and in what order ([`servers_for`]),
            // and what the request asks for.
            let chain = async {
                let (outcome, served_by) = self.read_server(server).await;
                let outcome = match outcome {
                    Ok(payload) => self.resolved(&served_by, payload, quality).await,
                    Err(e) => Err(e),
                };
                (outcome, served_by)
            };
            let (outcome, served_by) = if unbounded == Some(i) {
                chain.await
            } else {
                let ahead = unbounded.map_or(0, |u| u.saturating_sub(i));
                let cap = caps.cap(remaining(), ahead);
                match tokio::time::timeout(cap, chain).await {
                    Ok(outcome) => outcome,
                    // Cut off before any page answered for it, so the
                    // listing's URL is all the failure has to name.
                    Err(_elapsed) => (Err(AniError::Timeout), server.embed_url.clone()),
                }
            };
            // The attempt that produced this outcome, read before the
            // next server's fetch moves the transport's stamp.
            let at = self.fetch.last_attempt_at();
            let weather = match outcome {
                Ok(stream) => return Ok(stream),
                Err(AniError::NoResults) => match payload_missing_verdict(&served_by) {
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
        self.playlist_at(url, referer).await.map(|(body, _)| body)
    }

    /// The playlist beside the URL that served it: the transport
    /// follows redirects and reports where it ended, so a master the
    /// CDN has moved is read from where it landed.
    async fn playlist_at(&self, url: &str, referer: Option<&str>) -> Result<(String, String)> {
        let mut req = FetchRequest::get(url);
        if let Some(r) = referer {
            req = req.header("Referer", r);
        }
        self.page(&req).await.map(|page| (page.body, page.url))
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

    fn bound_attempt(&self, deadline: Option<tokio::time::Instant>) {
        *self.attempt_deadline.lock().expect("attempt deadline") = deadline;
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
///
/// `served_by` is the URL the page came from, not the one the listing
/// named: a listing URL that redirects lands on another host, and it
/// is the host that answered — the one whose markup is in hand — the
/// client either reads or does not.
fn payload_missing_verdict(served_by: &str) -> Option<AniError> {
    if !ajax::readable(served_by) {
        return None;
    }
    let host = url::Url::parse(served_by)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| served_by.to_string());
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
