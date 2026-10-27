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
/// names — `hianime` — not as the label a show key carries, so a
/// response field typed on one side matches the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    /// hianime — the provider ani-cli moved to when anidb.app went
    /// dark in September 2026.
    Hianime,
    /// A provider that exists only in tests: the orchestrator's walks
    /// are written for any number of providers, and their tests need
    /// two that are nobody's site.
    #[cfg(test)]
    TestA,
    /// The second test-only provider; see [`ProviderId::TestA`].
    #[cfg(test)]
    TestB,
}

impl ProviderId {
    /// Every provider the app knows, whether or not a state orders
    /// it: the set a gate exists for, and the set a stored label can
    /// name.
    pub const ALL: &'static [ProviderId] = &[Self::Hianime];

    /// The providers that exist only in tests. They have gates like
    /// every known provider and answer only from a table a test
    /// registers.
    #[cfg(test)]
    pub const TEST: &'static [ProviderId] = &[Self::TestA, Self::TestB];

    /// The provider a label names, when it names a known one. An
    /// absent or unknown label names none.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        Self::known().find(|p| p.label() == label)
    }

    /// Every provider the build knows: [`Self::ALL`], and in test
    /// builds [`Self::TEST`] too.
    pub(crate) fn known() -> impl Iterator<Item = Self> {
        let known = Self::ALL.iter();
        #[cfg(test)]
        let known = known.chain(Self::TEST);
        known.copied()
    }

    /// The label the renderer interpolates into its progress copy
    /// ("Searching {provider}…").
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            // The brand, not a domain: the site's domain churns and is
            // filtered per ISP, and the label names who answered.
            Self::Hianime => "hianime",
            #[cfg(test)]
            Self::TestA => "test-a",
            #[cfg(test)]
            Self::TestB => "test-b",
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
    /// The romanized name the card carries beside its display title
    /// (hianime's `data-jname`), when the markup carries one: another
    /// name the provider gives the show, read from the page the search
    /// already fetched.
    pub jname: Option<String>,
}

/// What a show's entry page says about it: the premiere year, the
/// other names it lists for the show — its Japanese title and its
/// synonyms, as written there — the genres it files the show under,
/// and the age rating it gives the show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryPage {
    /// The premiere year the page names, when it names one.
    pub year: Option<u32>,
    /// Every other name the page gives the show, in page order.
    pub names: Vec<String>,
    /// The genres the page lists, as written, in page order.
    pub genres: Vec<String>,
    /// The age rating the page's own heading carries ("PG-13", "R+",
    /// "Rx"), as written, when it carries one.
    pub rating: Option<String>,
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
    /// A tag an earlier version gave this row and may have stored in
    /// the numbering sidecar, when it differs from the row's tag now —
    /// set only by the stitched listing, whose recap before a
    /// cumulatively numbered part an earlier version tagged one part
    /// later. A request for it still finds the row.
    pub former_tag: Option<String>,
    /// The episode's title as the provider lists it, when it lists
    /// one: the evidence that tells an extra the provider appends
    /// after a show's last episode from an episode of the show.
    pub title: Option<String>,
}

pub use super::show_key::{slug_numeric_id, slug_search_term, slug_words, ShowKey};

/// The most a subtitle track's URL may run to, in bytes. A track URL
/// a CDN signs runs to a few hundred bytes — a path, a token, an
/// expiry — and a kibibyte leaves room for a long path and a long
/// signature both. The bound is the hand-offs': every track URL rides
/// on the player's command line, whose budget the packaged platforms
/// set differently and Windows sets smallest, at about thirty-two
/// kibibytes; sixteen tracks at this bound take half of it, leaving
/// the stream URL, the referer and the player's own flags the rest.
/// A provider drops a row past it where it drops a row it cannot
/// fetch, so a malformed page cannot fail a hand-off through an
/// optional track.
pub const SUBTITLE_URL_CAP: usize = 1024;

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

/// What an episode resolved to once a quality was selected: the URL
/// the player loads — the adaptive master, or the rendition the
/// quality setting chose — with the referer its fetches need and the
/// sidecar tracks beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStream {
    /// The URL the player loads, validated: it answered with a
    /// playlist.
    pub url: String,
    /// The `Referer` to send on every fetch of it, when the CDN
    /// checks one.
    pub referer: Option<String>,
    /// Sidecar subtitle tracks, outside the playlist.
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

    /// [`Self::playlist`] together with the URL the body was served
    /// from. The transport follows redirects, and a manifest's
    /// relative URIs are relative to where it landed, so the quality
    /// step joins a rendition to this URL rather than the one asked
    /// for. A provider whose transport does not report where a body
    /// came from answers with the URL asked for, which is what this
    /// default does.
    ///
    /// # Errors
    /// As [`Self::playlist`].
    async fn playlist_at(&self, url: &str, referer: Option<&str>) -> Result<(String, String)> {
        self.playlist(url, referer)
            .await
            .map(|body| (body, url.to_string()))
    }

    /// The stream URL a quality setting selects from a master
    /// playlist — shared across providers, see
    /// [`crate::scraper::hls::stream_url`].
    ///
    /// # Errors
    /// The master fetch's own failure, verbatim.
    async fn quality_stream_url(&self, source: &StreamSource, quality: &str) -> Result<String> {
        crate::scraper::hls::stream_url(self, source, quality).await
    }

    /// The stream an episode plays in `mode` at `quality`, validated
    /// the way the episode step needs it: the master answered with a
    /// playlist and the quality was selected from it, the rendition
    /// fetched when one was chosen. The default composes
    /// [`Provider::master_playlist_url`] and
    /// [`Provider::quality_stream_url`]; a provider that lists several
    /// servers per episode overrides it so the validation happens
    /// inside its walk of the servers, where a dead server can be
    /// stepped over for the next.
    ///
    /// # Errors
    /// As [`Provider::master_playlist_url`] and
    /// [`Provider::quality_stream_url`].
    async fn stream_for(
        &self,
        episode_id: u64,
        mode: &str,
        quality: &str,
    ) -> Result<ResolvedStream> {
        let source = self.master_playlist_url(episode_id, mode).await?;
        let url = self.quality_stream_url(&source, quality).await?;
        Ok(ResolvedStream {
            url,
            referer: source.referer,
            subtitles: source.subtitles,
        })
    }

    /// The premiere year the show's detail page names, when it names
    /// one. A missing page or a page without the hint is the soft
    /// `Ok(None)`; a refusal, rate limit, or transport failure is
    /// not a missing hint and propagates.
    ///
    /// # Errors
    /// Provider blocks and transport errors, verbatim.
    async fn detail_year(&self, slug: &str) -> Result<Option<u32>>;

    /// The show's entry page as [`EntryPage`] reads it — the year
    /// [`Self::detail_year`] answers, the other names the page gives
    /// the show, its genres and its rating — from the one request. A
    /// provider whose pages carry none of them answers the year alone,
    /// and the pick then takes none of its listings: a page that lists
    /// no genre and no rating is never read as safe.
    ///
    /// # Errors
    /// As [`Self::detail_year`].
    async fn detail_page(&self, slug: &str) -> Result<EntryPage> {
        Ok(EntryPage {
            year: self.detail_year(slug).await?,
            ..EntryPage::default()
        })
    }

    /// The post-admission start of this provider's most recent
    /// attempt, when its transport tracks one. The walk stamps
    /// aggregate failure verdicts with it. A provider may also answer
    /// `None` after a verdict no attempt produced — its own walk cutting
    /// a server off while the gate still held the chain's request, as
    /// hianime's does — and the episode step reads a timeout by it:
    /// stamped, a host stalled on an admitted request; unstamped, the
    /// time ran out with the request at the gate, and no host failed.
    fn last_attempt_at(&self) -> Option<tokio::time::Instant>;

    /// The deadline the walk's attempt against this provider runs
    /// under, told before the attempt runs, and `None` once it is
    /// over: a client outside an attempt carries no deadline, since
    /// the client an answer comes with outlives the attempt and is
    /// resolved against again. A provider that walks several servers
    /// per episode shares what remains among them; one that has
    /// nothing to share it among ignores it, which is the default.
    fn bound_attempt(&self, deadline: Option<tokio::time::Instant>) {
        let _ = deadline;
    }
}

#[cfg(test)]
#[path = "provider_test.rs"]
mod tests;
