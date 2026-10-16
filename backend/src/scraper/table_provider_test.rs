//! A [`Provider`] answered from tables — the stand-in every walk test
//! drives instead of a site-shaped transport stub.
//!
//! The walks (the alias search, the candidate pick, the episode
//! chain, the mode probe) are policy over the [`Provider`] seam, and
//! a test of policy should name what the provider *answers*, not
//! which bytes a particular site would have served to make it answer
//! that. This provider is that: hits per query, a listing per show, a
//! mode and a stream per episode, a playlist per URL, a year per show
//! — each either a value or a failure the test chose — with a log of
//! what was asked, so a test can pin how far a walk went.
//!
//! Defaults for what no table lists follow what a real client does
//! with a page the site does not have: a search answers the clean
//! no-results shape (`Ok(vec![])`), a detail year is the soft `None`,
//! and everything else is a not-found-shaped upstream status.
//!
//! Compiled only under `cfg(test)`; nothing in production reaches it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{AniError, Result};
use crate::scraper::gate::{ScrapePriority, ScraperGate};
use crate::scraper::provider::{
    slug_numeric_id, BrowseHit, EpisodeRef, Provider, ProviderId, StreamSource,
};

/// What a table answers for one key: a value, or a failure made
/// fresh on every ask — [`AniError`] does not clone.
pub(crate) enum Answer<T> {
    Ok(T),
    Err(Arc<dyn Fn() -> AniError + Send + Sync>),
}

impl<T> Answer<T> {
    /// An answer that fails with what `make` builds.
    pub(crate) fn fails(make: impl Fn() -> AniError + Send + Sync + 'static) -> Self {
        Self::Err(Arc::new(make))
    }

    fn get(&self) -> Result<T>
    where
        T: Clone,
    {
        match self {
            Self::Ok(v) => Ok(v.clone()),
            Self::Err(make) => Err(make()),
        }
    }
}

impl<T> From<T> for Answer<T> {
    fn from(value: T) -> Self {
        Self::Ok(value)
    }
}

/// A show as a table keys it: by slug, or by the numeric id a slug
/// carries in its tail, so a listing registered under `77` answers
/// for `the-show-77`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Show {
    Slug(String),
    Id(u64),
}

impl From<&str> for Show {
    fn from(slug: &str) -> Self {
        Self::Slug(slug.to_string())
    }
}

impl From<u64> for Show {
    fn from(id: u64) -> Self {
        Self::Id(id)
    }
}

/// One request a table provider received, in the order received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Asked {
    Search(String),
    Episodes(String),
    Mode(u64, String),
    Master(u64, String),
    Playlist(String),
    Year(String),
}

/// The not-found-shaped status a real client answers for a page the
/// site does not have.
fn not_found() -> AniError {
    AniError::Upstream { status: 404 }
}

/// A listing of `count` episodes numbered `1..=count`, with the ids a
/// site that keys episodes under their show would give them
/// (`id * 1000 + n`), so a test can name an episode by arithmetic.
pub(crate) fn listing_of(id: u64, count: u32) -> Vec<EpisodeRef> {
    (1..=count)
        .map(|n| EpisodeRef {
            id: id * 1000 + u64::from(n),
            number: n,
            number2: None,
            former_tag: None,
        })
        .collect()
}

/// See the module docs.
pub(crate) struct TableProvider {
    id: ProviderId,
    /// `(query substring, answer)` — first match wins, like a
    /// site's search answering by what the query contains.
    searches: Vec<(String, Answer<Vec<BrowseHit>>)>,
    search_default: Answer<Vec<BrowseHit>>,
    listings: HashMap<Show, Answer<Vec<EpisodeRef>>>,
    listings_default: Answer<Vec<EpisodeRef>>,
    modes: HashMap<(u64, String), Answer<bool>>,
    modes_default: Answer<bool>,
    streams: HashMap<(u64, String), Answer<StreamSource>>,
    streams_default: Answer<StreamSource>,
    playlists: HashMap<String, Answer<String>>,
    playlists_default: Answer<String>,
    years: HashMap<Show, Answer<Option<u32>>>,
    years_default: Answer<Option<u32>>,
    log: Mutex<Vec<(Asked, tokio::time::Instant)>>,
    stamp: Option<tokio::time::Instant>,
    stalls: bool,
    delay: Option<std::time::Duration>,
    search_delay: Option<std::time::Duration>,
}

impl Default for TableProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TableProvider {
    /// An empty table: every search is a clean miss, every year is
    /// unknown, everything else is not found.
    pub(crate) fn new() -> Self {
        Self {
            id: crate::provider_set::ProviderSet::TEST_DEFAULT,
            searches: Vec::new(),
            search_default: Answer::Ok(Vec::new()),
            listings: HashMap::new(),
            listings_default: Answer::fails(not_found),
            modes: HashMap::new(),
            modes_default: Answer::fails(not_found),
            streams: HashMap::new(),
            streams_default: Answer::fails(not_found),
            playlists: HashMap::new(),
            playlists_default: Answer::fails(not_found),
            years: HashMap::new(),
            years_default: Answer::Ok(None),
            log: Mutex::new(Vec::new()),
            stamp: None,
            stalls: false,
            delay: None,
            search_delay: None,
        }
    }

    /// A provider whose every request dies on transport.
    pub(crate) fn unreachable() -> Self {
        Self::failing(|| AniError::Network)
    }

    /// A provider whose every request answers `status` — except the
    /// detail year, which a real client reads as a missing page (the
    /// soft `None`) unless the status is a block.
    pub(crate) fn refusing(status: u16) -> Self {
        let mut t = Self::failing(move || AniError::Upstream { status });
        if !(AniError::Upstream { status }).is_provider_block() {
            t.years_default = Answer::Ok(None);
        }
        t
    }

    fn failing(make: impl Fn() -> AniError + Send + Sync + Copy + 'static) -> Self {
        let mut t = Self::new();
        t.search_default = Answer::fails(make);
        t.listings_default = Answer::fails(make);
        t.modes_default = Answer::fails(make);
        t.streams_default = Answer::fails(make);
        t.playlists_default = Answer::fails(make);
        t.years_default = Answer::fails(make);
        t
    }

    /// One show a search for `query` finds — `slug`, `title` — whose
    /// listing is `episodes` as `(id, number)`; each id in `streaming`
    /// plays in sub from `master`.
    pub(crate) fn one_show(
        query: &str,
        (slug, title): (&str, &str),
        episodes: &[(u64, u32)],
        streaming: &[u64],
        master: &str,
    ) -> Self {
        let show = slug_numeric_id(slug).expect("a slug-shaped id");
        let rows = episodes
            .iter()
            .map(|(id, number)| EpisodeRef {
                id: *id,
                number: *number,
                number2: None,
                former_tag: None,
            })
            .collect::<Vec<_>>();
        streaming.iter().fold(
            Self::new()
                .hits(
                    query,
                    vec![BrowseHit {
                        slug: slug.to_string(),
                        title: title.to_string(),
                        kind: None,
                        jname: None,
                    }],
                )
                .listing(show, rows),
            |t, id| t.stream(*id, "sub", master).mode(*id, "sub", true),
        )
    }

    /// Listings of `count` episodes for each `(id, count)`, as
    /// [`listing_of`] numbers them.
    pub(crate) fn counted(shows: &[(u64, u32)]) -> Self {
        shows.iter().fold(Self::new(), |t, (id, count)| {
            t.listing(*id, listing_of(*id, *count))
        })
    }

    /// What fetching the playlist at `url` answers.
    pub(crate) fn playlist(mut self, url: &str, answer: impl Into<Answer<String>>) -> Self {
        self.playlists.insert(url.to_string(), answer.into());
        self
    }

    /// What every unlisted playlist URL answers.
    pub(crate) fn playlists_default(mut self, answer: impl Into<Answer<String>>) -> Self {
        self.playlists_default = answer.into();
        self
    }

    /// [`Self::counted`], each show with the premiere year its detail
    /// page names — `None` for a page without one.
    pub(crate) fn counted_with_years(shows: &[(u64, u32, Option<u32>)]) -> Self {
        shows.iter().fold(Self::new(), |t, (id, count, year)| {
            t.listing(*id, listing_of(*id, *count)).year(*id, *year)
        })
    }

    /// The provider this table answers as — and the slot a
    /// [`crate::provider_set::ProviderSet`] registers it in.
    pub(crate) fn answering_as(mut self, id: ProviderId) -> Self {
        self.id = id;
        self
    }

    /// What a search whose query contains `needle` answers.
    pub(crate) fn hits(mut self, needle: &str, answer: impl Into<Answer<Vec<BrowseHit>>>) -> Self {
        self.searches.push((needle.to_string(), answer.into()));
        self
    }

    /// What every search no `hits` entry matches answers.
    pub(crate) fn search_default(mut self, answer: impl Into<Answer<Vec<BrowseHit>>>) -> Self {
        self.search_default = answer.into();
        self
    }

    /// What `show`'s episode listing answers.
    pub(crate) fn listing(
        mut self,
        show: impl Into<Show>,
        answer: impl Into<Answer<Vec<EpisodeRef>>>,
    ) -> Self {
        self.listings.insert(show.into(), answer.into());
        self
    }

    /// What every unlisted show's episode listing answers.
    pub(crate) fn listings_default(mut self, answer: impl Into<Answer<Vec<EpisodeRef>>>) -> Self {
        self.listings_default = answer.into();
        self
    }

    /// What asking whether `episode` carries `mode` answers.
    pub(crate) fn mode(
        mut self,
        episode: u64,
        mode: &str,
        answer: impl Into<Answer<bool>>,
    ) -> Self {
        self.modes
            .insert((episode, mode.to_string()), answer.into());
        self
    }

    /// What every unlisted `(episode, mode)` answers to the mode
    /// question.
    pub(crate) fn modes_default(mut self, answer: impl Into<Answer<bool>>) -> Self {
        self.modes_default = answer.into();
        self
    }

    /// `episode` streams in `mode` from `master_url`, which answers
    /// a validating playlist until [`Self::playlist`] says otherwise.
    pub(crate) fn stream(self, episode: u64, mode: &str, master_url: &str) -> Self {
        self.stream_from(
            episode,
            mode,
            StreamSource {
                master_url: master_url.to_string(),
                referer: None,
                subtitles: Vec::new(),
            },
        )
    }

    /// What `episode`'s stream in `mode` answers — a source, with a
    /// validating playlist registered for its master, or a failure.
    pub(crate) fn stream_from(
        mut self,
        episode: u64,
        mode: &str,
        answer: impl Into<Answer<StreamSource>>,
    ) -> Self {
        let answer = answer.into();
        if let Answer::Ok(source) = &answer {
            self.playlists
                .entry(source.master_url.clone())
                .or_insert_with(|| Answer::Ok("#EXTM3U\n".to_string()));
        }
        self.streams.insert((episode, mode.to_string()), answer);
        self
    }

    /// What `show`'s detail page names as its premiere year.
    pub(crate) fn year(
        mut self,
        show: impl Into<Show>,
        answer: impl Into<Answer<Option<u32>>>,
    ) -> Self {
        self.years.insert(show.into(), answer.into());
        self
    }

    /// What every unlisted show's detail page answers.
    pub(crate) fn years_default(mut self, answer: impl Into<Answer<Option<u32>>>) -> Self {
        self.years_default = answer.into();
        self
    }

    /// Report `stamp` as the start of the latest attempt, the way a
    /// gated transport reports its post-admission start.
    pub(crate) fn stamped(mut self, stamp: tokio::time::Instant) -> Self {
        self.stamp = Some(stamp);
        self
    }

    /// Every request hangs forever — the provider accepting
    /// connections and never answering. Still logged.
    pub(crate) fn stalling(mut self) -> Self {
        self.stalls = true;
        self
    }

    /// Every request answers only after `delay` — a provider
    /// answering near its own per-request timeout.
    pub(crate) fn delaying(mut self, delay: std::time::Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// Searches answer only after `delay`; everything else at once —
    /// a resolve held on its first request.
    pub(crate) fn delaying_searches(mut self, delay: std::time::Duration) -> Self {
        self.search_delay = Some(delay);
        self
    }

    /// Everything asked so far, in order.
    pub(crate) fn asked(&self) -> Vec<Asked> {
        self.log
            .lock()
            .expect("asked")
            .iter()
            .map(|(a, _)| a.clone())
            .collect()
    }

    /// When each request so far was received, in order.
    pub(crate) fn asked_at(&self) -> Vec<(Asked, tokio::time::Instant)> {
        self.log.lock().expect("asked").clone()
    }

    /// The queries searched so far, in order.
    pub(crate) fn searched(&self) -> Vec<String> {
        self.asked()
            .into_iter()
            .filter_map(|a| match a {
                Asked::Search(q) => Some(q),
                _ => None,
            })
            .collect()
    }

    /// How many of the requests so far `pred` picks out.
    pub(crate) fn count(&self, pred: impl Fn(&Asked) -> bool) -> usize {
        self.asked().iter().filter(|a| pred(a)).count()
    }

    async fn note(&self, asked: Asked) {
        self.log
            .lock()
            .expect("asked")
            .push((asked, tokio::time::Instant::now()));
        if self.stalls {
            std::future::pending::<()>().await;
        }
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
    }

    fn by_show<T: Clone>(
        table: &HashMap<Show, Answer<T>>,
        default: &Answer<T>,
        slug: &str,
    ) -> Result<T> {
        if let Some(answer) = table.get(&Show::Slug(slug.to_string())) {
            return answer.get();
        }
        if let Some(answer) = slug_numeric_id(slug).and_then(|id| table.get(&Show::Id(id))) {
            return answer.get();
        }
        default.get()
    }
}

#[async_trait::async_trait]
impl Provider for TableProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        self.note(Asked::Search(query.to_string())).await;
        if let Some(delay) = self.search_delay {
            tokio::time::sleep(delay).await;
        }
        self.searches
            .iter()
            .find(|(needle, _)| query.contains(needle.as_str()))
            .map_or(&self.search_default, |(_, answer)| answer)
            .get()
    }

    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        self.note(Asked::Episodes(slug.to_string())).await;
        Self::by_show(&self.listings, &self.listings_default, slug)
    }

    async fn has_mode(&self, episode_id: u64, mode: &str) -> Result<bool> {
        self.note(Asked::Mode(episode_id, mode.to_string())).await;
        self.modes
            .get(&(episode_id, mode.to_string()))
            .unwrap_or(&self.modes_default)
            .get()
    }

    async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<StreamSource> {
        self.note(Asked::Master(episode_id, mode.to_string())).await;
        self.streams
            .get(&(episode_id, mode.to_string()))
            .unwrap_or(&self.streams_default)
            .get()
    }

    async fn playlist(&self, url: &str, _referer: Option<&str>) -> Result<String> {
        self.note(Asked::Playlist(url.to_string())).await;
        self.playlists
            .get(url)
            .unwrap_or(&self.playlists_default)
            .get()
    }

    async fn detail_year(&self, slug: &str) -> Result<Option<u32>> {
        self.note(Asked::Year(slug.to_string())).await;
        Self::by_show(&self.years, &self.years_default, slug)
    }

    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        self.stamp
    }
}

/// A gate-admitting decorator for a [`TableProvider`] — what
/// [`crate::scraper::gated::GatedFetch`] is for a transport, at the
/// provider seam: every request admits through the gate at
/// `priority` before the inner provider answers, a chain's first
/// admit may take the half-open trial and the rest ride it, and the
/// post-admission start of the latest request is what the breaker
/// outcome gets stamped with.
///
/// Only the six request methods are gated. `stream_for`,
/// `playlist_at` and the quality step keep the trait's defaults, which
/// call back into the gated methods, so every request a table answers
/// is admitted. A real client that overrides them (hianime overrides
/// `stream_for`) would bypass the gate through this wrapper, which is
/// why production clients are gated at their transport instead.
pub(crate) struct Gated<'g, P> {
    inner: P,
    gate: Option<&'g ScraperGate>,
    priority: ScrapePriority,
    sanction: Mutex<Option<tokio::time::Instant>>,
    last_attempt_at: Mutex<Option<tokio::time::Instant>>,
}

impl<'g, P> Gated<'g, P> {
    pub(crate) fn new(inner: P, gate: Option<&'g ScraperGate>, priority: ScrapePriority) -> Self {
        Self {
            inner,
            gate,
            priority,
            sanction: Mutex::new(None),
            last_attempt_at: Mutex::new(None),
        }
    }

    async fn admit(&self) -> Result<()> {
        if let Some(gate) = self.gate {
            let held = *self.sanction.lock().expect("sanction");
            let granted = gate
                .admit_chained(self.priority, held)
                .await
                .map_err(|_| AniError::GateRefused)?;
            *self.sanction.lock().expect("sanction") = granted;
        }
        *self.last_attempt_at.lock().expect("stamp") = Some(tokio::time::Instant::now());
        Ok(())
    }
}

#[async_trait::async_trait]
impl<P: Provider> Provider for Gated<'_, P> {
    fn id(&self) -> ProviderId {
        self.inner.id()
    }

    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        self.admit().await?;
        self.inner.search(query).await
    }

    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        self.admit().await?;
        self.inner.episodes(slug).await
    }

    async fn has_mode(&self, episode_id: u64, mode: &str) -> Result<bool> {
        self.admit().await?;
        self.inner.has_mode(episode_id, mode).await
    }

    async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<StreamSource> {
        self.admit().await?;
        self.inner.master_playlist_url(episode_id, mode).await
    }

    async fn playlist(&self, url: &str, referer: Option<&str>) -> Result<String> {
        self.admit().await?;
        self.inner.playlist(url, referer).await
    }

    async fn detail_year(&self, slug: &str) -> Result<Option<u32>> {
        self.admit().await?;
        self.inner.detail_year(slug).await
    }

    async fn detail_page(&self, slug: &str) -> Result<crate::scraper::provider::EntryPage> {
        self.admit().await?;
        self.inner.detail_page(slug).await
    }

    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        *self.last_attempt_at.lock().expect("stamp")
    }
}

#[async_trait::async_trait]
impl<P: Provider + ?Sized> Provider for &P {
    fn id(&self) -> ProviderId {
        (**self).id()
    }

    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        (**self).search(query).await
    }

    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        (**self).episodes(slug).await
    }

    async fn has_mode(&self, episode_id: u64, mode: &str) -> Result<bool> {
        (**self).has_mode(episode_id, mode).await
    }

    async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<StreamSource> {
        (**self).master_playlist_url(episode_id, mode).await
    }

    async fn playlist(&self, url: &str, referer: Option<&str>) -> Result<String> {
        (**self).playlist(url, referer).await
    }

    async fn playlist_at(&self, url: &str, referer: Option<&str>) -> Result<(String, String)> {
        (**self).playlist_at(url, referer).await
    }

    async fn quality_stream_url(&self, source: &StreamSource, quality: &str) -> Result<String> {
        (**self).quality_stream_url(source, quality).await
    }

    async fn stream_for(
        &self,
        episode_id: u64,
        mode: &str,
        quality: &str,
    ) -> Result<crate::scraper::provider::ResolvedStream> {
        (**self).stream_for(episode_id, mode, quality).await
    }

    async fn detail_year(&self, slug: &str) -> Result<Option<u32>> {
        (**self).detail_year(slug).await
    }

    async fn detail_page(&self, slug: &str) -> Result<crate::scraper::provider::EntryPage> {
        (**self).detail_page(slug).await
    }

    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        (**self).last_attempt_at()
    }

    fn bound_attempt(&self, deadline: Option<tokio::time::Instant>) {
        (**self).bound_attempt(deadline);
    }
}

#[async_trait::async_trait]
impl<P: Provider + ?Sized> Provider for Arc<P> {
    fn id(&self) -> ProviderId {
        (**self).id()
    }

    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>> {
        (**self).search(query).await
    }

    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>> {
        (**self).episodes(slug).await
    }

    async fn has_mode(&self, episode_id: u64, mode: &str) -> Result<bool> {
        (**self).has_mode(episode_id, mode).await
    }

    async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<StreamSource> {
        (**self).master_playlist_url(episode_id, mode).await
    }

    async fn playlist(&self, url: &str, referer: Option<&str>) -> Result<String> {
        (**self).playlist(url, referer).await
    }

    async fn playlist_at(&self, url: &str, referer: Option<&str>) -> Result<(String, String)> {
        (**self).playlist_at(url, referer).await
    }

    async fn quality_stream_url(&self, source: &StreamSource, quality: &str) -> Result<String> {
        (**self).quality_stream_url(source, quality).await
    }

    async fn stream_for(
        &self,
        episode_id: u64,
        mode: &str,
        quality: &str,
    ) -> Result<crate::scraper::provider::ResolvedStream> {
        (**self).stream_for(episode_id, mode, quality).await
    }

    async fn detail_year(&self, slug: &str) -> Result<Option<u32>> {
        (**self).detail_year(slug).await
    }

    async fn detail_page(&self, slug: &str) -> Result<crate::scraper::provider::EntryPage> {
        (**self).detail_page(slug).await
    }

    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        (**self).last_attempt_at()
    }

    fn bound_attempt(&self, deadline: Option<tokio::time::Instant>) {
        (**self).bound_attempt(deadline);
    }
}
