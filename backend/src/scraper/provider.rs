//! The seam every stream provider sits behind.
//!
//! The resolver's walks — search the aliases, pick the candidate by
//! episode count and year, chase the episode to a playable URL — are
//! policy, and they read the same whichever site answers. What
//! differs per site is how a query becomes hits, how a slug becomes
//! an episode list, and how an episode becomes a playlist: that is
//! the [`Provider`] trait, and a client implements it over the shared
//! transport. The walks take `&P where P: Provider`, so a second
//! provider slots in without the walks learning its name.

use crate::error::Result;

/// Which provider answered. Stamped onto everything provider output
/// reaches — progress lines, breaker outcomes, cache rows — so a
/// failover between providers can attribute each attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    /// anidb.app — the provider ani-cli 5.0 scrapes.
    Anidb,
}

impl ProviderId {
    /// The label the renderer interpolates into its progress copy
    /// ("Searching {provider}…").
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Anidb => "anidb.app",
        }
    }
}

#[cfg(test)]
#[path = "provider_prop_test.rs"]
mod prop_tests;

/// One search hit: the slug the provider's API is keyed on, and the
/// display title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseHit {
    /// The provider's show id, as the provider spells it.
    pub slug: String,
    /// Entity-decoded display title.
    pub title: String,
    /// The card's format badge (`TV`, `Movie`, `OVA`, ...) when the
    /// markup carries one. `None` reads as unknown — a soft signal,
    /// like an unparseable year.
    pub kind: Option<String>,
}

/// One episode row: the id the per-episode endpoints are keyed on
/// and the 1-based episode number shown to users.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeRef {
    /// The provider's episode id — what the mode and embed lookups
    /// take.
    pub id: u64,
    /// 1-based episode number as shown to users.
    pub number: u32,
    /// The provider's display tag when it differs from `number` —
    /// recaps and specials stream under decimal tags ("1061.5"),
    /// and a decimal play request matches this field verbatim.
    pub number2: Option<String>,
}

/// What an episode resolved to: the master-playlist URL and the
/// referer the provider's CDN wants on every fetch of it and of what
/// it lists. `None` when the CDN wants none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSource {
    /// The master-playlist URL the provider's embed carried.
    pub master_url: String,
    /// The `Referer` to send on playlist and segment fetches, when
    /// the CDN checks one — the embed host's origin, typically.
    pub referer: Option<String>,
}

/// A stream provider: search, episode listing, per-episode audio
/// mode, and stream-URL resolution. Every method's errors follow the
/// walk's vocabulary — a typed upstream refusal, a parse failure, the
/// transport's own failures — because the walk classifies them.
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    /// Which provider this is.
    fn id(&self) -> ProviderId;

    /// The label progress lines carry.
    fn label(&self) -> &'static str {
        self.id().label()
    }

    /// Search by title. An empty list is the provider answering
    /// absence; a body that does not show the search shape is a
    /// parse failure, never absence.
    ///
    /// # Errors
    /// Upstream refusals, parse failures, and transport errors.
    async fn search(&self, query: &str) -> Result<Vec<BrowseHit>>;

    /// A show's episodes by slug.
    ///
    /// # Errors
    /// As [`Provider::search`].
    async fn episodes(&self, slug: &str) -> Result<Vec<EpisodeRef>>;

    /// Whether an episode carries the requested audio mode
    /// (`sub`/`dub`), fetching only what answers that.
    ///
    /// # Errors
    /// As [`Provider::search`].
    async fn has_mode(&self, episode_id: u64, mode: &str) -> Result<bool>;

    /// The master playlist an episode streams from in `mode`, with
    /// the referer its fetches need.
    ///
    /// # Errors
    /// [`crate::error::AniError::NoResults`] when the mode has no
    /// embed or the embed carries no playlist, plus upstream and
    /// transport errors.
    async fn master_playlist_url(&self, episode_id: u64, mode: &str) -> Result<StreamSource>;

    /// A playlist body, fetched with `referer` when the source named
    /// one and whatever else the provider's CDN requires. Refuses
    /// challenge pages and non-success statuses as typed upstream
    /// errors, like every other provider fetch.
    ///
    /// # Errors
    /// Upstream refusals and transport errors.
    async fn playlist(&self, url: &str, referer: Option<&str>) -> Result<String>;

    /// The stream URL a quality setting selects from a master
    /// playlist — shared across providers, see
    /// [`crate::scraper::hls::stream_url`].
    ///
    /// # Errors
    /// The master fetch's own failure, verbatim.
    async fn quality_stream_url(&self, source: &StreamSource, quality: &str) -> Result<String> {
        crate::scraper::hls::stream_url(self, source, quality).await
    }

    /// The premiere year the show's detail page names, when it names
    /// one. A missing page or a page without the hint is the soft
    /// `Ok(None)`; a refusal, rate limit, or transport failure is
    /// not a missing hint and propagates.
    ///
    /// # Errors
    /// Provider blocks and transport errors, verbatim.
    async fn detail_year(&self, slug: &str) -> Result<Option<u32>>;

    /// The post-admission start of this provider's most recent
    /// attempt, when its transport tracks one. The walk stamps
    /// aggregate failure verdicts with it.
    fn last_attempt_at(&self) -> Option<tokio::time::Instant>;
}
