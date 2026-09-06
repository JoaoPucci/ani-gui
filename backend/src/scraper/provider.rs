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

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Whether a response body is cloudflare's challenge interstitial
/// rather than provider content. Case-insensitive, like the script's
/// `grep -qi`: challenge pages have varied the title's spelling.
pub fn is_cloudflare_interstitial(body: &str) -> bool {
    body.to_ascii_lowercase().contains("just a moment")
}

/// Form-urlencode a search query: space→`+`, reserved and non-ASCII
/// bytes percent-encoded. The script's naive space swap sent `;` and
/// friends raw and the provider answers those with a 400.
pub fn encode_query(query: &str) -> String {
    url::form_urlencoded::byte_serialize(query.as_bytes()).collect()
}

/// Which provider answered. Stamped onto everything provider output
/// reaches — progress lines, breaker outcomes, cache rows — so a
/// failover between providers can attribute each attempt.
///
/// Serialized as the identifier the renderer's `StreamProvider`
/// names — `anidb`, `hianime` — not as the label a show key carries,
/// so a response field typed on one side matches the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    /// anidb.app — the provider ani-cli 5.0 scrapes.
    Anidb,
    /// hianime — the provider ani-cli moved to when anidb.app went
    /// dark in September 2026.
    Hianime,
}

impl ProviderId {
    /// The provider a label names. Anything that is not a known
    /// label — an absent one included — is anidb, the default every
    /// caller from before there were two providers means.
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        match label {
            "hianime" => Self::Hianime,
            _ => Self::Anidb,
        }
    }

    /// The label the renderer interpolates into its progress copy
    /// ("Searching {provider}…").
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Anidb => "anidb.app",
            // The brand, not a domain: the site's domain churns and is
            // filtered per ISP, and the label names who answered.
            Self::Hianime => "hianime",
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

/// The digits after a slug's last hyphen, when there are any —
/// the entry id both providers key their listings on.
#[must_use]
pub fn slug_numeric_id(slug: &str) -> Option<u64> {
    slug.rsplit('-').next()?.parse().ok()
}

/// The Kitsu-searchable text a slug carries: its hyphenated
/// words with the trailing numeric id removed (`one-piece-69` →
/// `one piece`). `None` when `slug` isn't slug-shaped — legacy
/// allanime ids are mixed-case and hyphenless, so they fall through
/// to their own resolve path.
#[must_use]
pub fn slug_search_term(slug: &str) -> Option<String> {
    slug_numeric_id(slug)?;
    let (words, _id) = slug.rsplit_once('-')?;
    if words.is_empty() {
        return None;
    }
    Some(words.replace('-', " "))
}

/// A show id that says whose id it is. Every store stamped by a
/// resolve — history rows, the numbering sidecar, the watched-at
/// stamps, the reverse mapping, the cache row — keys on this key's
/// string form. anidb's is the bare slug every existing row already
/// holds, so nothing migrates; any other provider's carries its label
/// as a prefix, so the read side can tell them apart.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShowKey {
    /// The provider whose slug this is.
    pub provider: ProviderId,
    /// The slug as the provider spells it.
    pub slug: String,
}

impl ShowKey {
    /// A key for `slug` on `provider`.
    pub fn new(provider: ProviderId, slug: impl Into<String>) -> Self {
        Self {
            provider,
            slug: slug.into(),
        }
    }

    /// The key a stored id names. A prefix the app does not know is
    /// part of the slug: only a known label qualifies an id, and a
    /// bare id is anidb's — including the allanime-era ids that
    /// predate slugs, which parse but carry no words.
    #[must_use]
    pub fn parse(id: &str) -> Self {
        for provider in [ProviderId::Hianime] {
            if let Some(slug) = id
                .strip_prefix(provider.label())
                .and_then(|r| r.strip_prefix(':'))
            {
                return Self::new(provider, slug);
            }
        }
        Self::new(ProviderId::Anidb, id)
    }

    /// The Kitsu-searchable words the slug carries, when it is
    /// slug-shaped — both providers spell theirs `words-id`.
    #[must_use]
    pub fn search_term(&self) -> Option<String> {
        slug_search_term(&self.slug)
    }
}

impl std::fmt::Display for ShowKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.provider {
            ProviderId::Anidb => f.write_str(&self.slug),
            other => write!(f, "{}:{}", other.label(), self.slug),
        }
    }
}

/// A sidecar subtitle track a provider lists beside the stream —
/// a `.vtt` outside the playlist, which nothing in the manifest
/// would ever tell the player about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleTrack {
    /// Language code (`en`).
    pub lang: String,
    /// Display label (`English`).
    pub label: String,
    /// Whether the player should select it by default.
    #[serde(default)]
    pub default: bool,
    /// The track's upstream URL, fetched with the source's referer.
    pub url: String,
}

/// What an episode resolved to: the master-playlist URL, the referer
/// the provider's CDN wants on every fetch of it and of what it
/// lists (`None` when the CDN wants none), and the sidecar subtitle
/// tracks listed beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSource {
    /// The master-playlist URL the provider's embed carried.
    pub master_url: String,
    /// The `Referer` to send on playlist and segment fetches, when
    /// the CDN checks one — the embed host's origin, typically.
    pub referer: Option<String>,
    /// Sidecar subtitle tracks, outside the playlist. Empty for a
    /// provider whose subtitles ride inside the manifest.
    pub subtitles: Vec<SubtitleTrack>,
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

#[cfg(test)]
#[path = "provider_test.rs"]
mod tests;
