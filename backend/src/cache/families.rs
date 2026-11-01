//! Every key family the app keeps in `meta_cache`, declared once.
//!
//! A family is the prefix every key of one kind starts with, its
//! version included: `availability:v25:` names this build's
//! availability verdicts, and a row under `availability:v24:` is one
//! no build of this version reads. Each key builder in the app takes
//! its prefix from here, so a version bump moves the builder and the
//! registry together, and the version before it stops being declared
//! the moment the bump lands.
//!
//! What the registry is for: [`super::reclaim`] deletes, at launch,
//! every row whose key no family declares, and every row past its
//! lifetime in a family nothing reads past its lifetime. Without it,
//! a re-keyed family's old rows and every expired row stayed in the
//! file for good, since nothing else deletes a row once it expires.
//!
//! A family is read after expiry when some reader takes its rows
//! whatever their age, and those rows are kept until they are written
//! again or the history removal takes them:
//!
//! - the availability verdicts, whose window a reschedule rewrites in
//!   place and whose served mode a replay merges into an expired row
//!   (`availability_reschedule`, `availability_served_mode`);
//! - AniList's trending list, served stale while AniList is down
//!   (`kitsu_trending_anilist`);
//! - the reverse mappings and the title matches, which a history
//!   removal reads to find every Kitsu id a show was known by
//!   (`history_forget*`), and which the detail page's lookup reads for
//!   a row whose recorded entry Kitsu answered gone
//!   (`history_forget_titles::title_match_ids`).

/// One key family: the prefix its keys start with, or the one key it
/// is when it is a single row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Family {
    /// The prefix, or the whole key when [`Self::whole_key`] holds.
    pub prefix: String,
    /// Whether the family is one row, keyed by exactly the prefix.
    pub whole_key: bool,
    /// Whether some reader takes the family's rows past their lifetime.
    pub read_after_expiry: bool,
}

impl Family {
    fn prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            whole_key: false,
            read_after_expiry: false,
        }
    }

    fn key(key: impl Into<String>) -> Self {
        Self {
            whole_key: true,
            ..Self::prefix(key)
        }
    }

    fn read_after_expiry(self) -> Self {
        Self {
            read_after_expiry: true,
            ..self
        }
    }

    /// Whether `key` is one of this family's.
    #[must_use]
    pub fn holds(&self, key: &str) -> bool {
        if self.whole_key {
            key == self.prefix
        } else {
            key.starts_with(&self.prefix)
        }
    }
}

// The prefixes of the families whose builders take them from here.
// Each ends where the key's own part starts.

/// Kitsu's trending list for the discovery row.
pub(crate) const KITSU_TRENDING: &str = "kitsu:v2:trending";
/// Kitsu's top-rated list for the discovery row.
pub(crate) const KITSU_TOP_RATED: &str = "kitsu:v2:top_rated";
/// AniList's trending list, bridged to Kitsu.
pub(crate) const ANILIST_TRENDING: &str = "kitsu:v6:trending_anilist";
/// A Kitsu entry's episode pages.
pub(crate) const KITSU_EPISODES: &str = "kitsu:episodes:";
/// A Kitsu entry's relations.
pub(crate) const KITSU_RELATIONS: &str = "kitsu:relations:v3:";
/// A Kitsu entry looked up by its slug.
pub(crate) const KITSU_ANIME_SLUG: &str = "kitsu:v2:anime-slug:";
/// A Kitsu entry's detail.
pub(crate) const KITSU_ANIME: &str = "kitsu:v5:anime:";
/// A page of Kitsu's search.
pub(crate) const KITSU_SEARCH: &str = "kitsu:v4:search:";
/// A Kitsu entry the detail fetch found gone.
pub(crate) const KITSU_GONE: &str = "kitsu:dead:";
/// A Kitsu entry's ids on other databases.
pub(crate) const KITSU_EXTERNAL_IDS: &str = "kitsu:external-ids:v2:k";
/// A MyAnimeList id's Kitsu entry.
pub(crate) const KITSU_MAL_MAP: &str = "kitsu:mal-map:v1:";
/// A show's watched-at stamp.
pub(crate) const WATCHED_AT: &str = "watched-at:v1:";
/// The mark a play leaves beside the reverse mapping it stored.
pub(crate) const MAPPING_PLAYED: &str = "allmanga2kitsu:played:v1:";
/// An airing show's status.
pub(crate) const AIRING: &str = "airing:v2:";
/// An episode's skip times.
pub(crate) const ANISKIP: &str = "aniskip:v2:";
/// The reading an entry's appended extras were proved to have.
pub(crate) const APPENDED: &str = "appended:v2:";
/// AniList's banner for a Kitsu entry.
pub(crate) const ANILIST_BANNER: &str = "anilist:banner:v1:k";
/// AniList's episode thumbnails for a Kitsu entry.
pub(crate) const ANILIST_EPS_THUMBS: &str = "anilist:eps-thumbs:v2:k";
/// A MyAnimeList id's AniList media id.
pub(crate) const ANILIST_MEDIA_FOR_MAL: &str = "anilist:media-id-for-mal:v1:";
/// An AniList media id's MyAnimeList id.
pub(crate) const ANILIST_MAL_FOR_MEDIA: &str = "anilist:mal-id-for-media:v1:";
/// The AniList media recording one MyAnimeList id.
pub(crate) const ANILIST_MEDIA_RECORDING_MAL: &str = "anilist:media-ids-recording-mal:v1:";
/// The extra tags a show's listing carried, per show key.
pub(crate) const LISTING_TAGS: &str = "listing-tags:v3:";
/// An image the proxy found gone.
pub(crate) const IMAGE_DEAD: &str = "image:dead:v1:";
/// A tracker's mapping of a Kitsu entry.
pub(crate) const TRACKER_MAP: &str = "tracker-map:v3:";

/// The reverse mappings, at the version this build reads.
#[must_use]
pub(crate) fn mapping() -> String {
    format!(
        "allmanga2kitsu:v{}:",
        crate::commands::kitsu::ALLMANGA_KITSU_VERSION
    )
}

/// The title matches, at the version this build reads.
#[must_use]
pub(crate) fn title_match() -> String {
    format!(
        "title-match:v{}:",
        crate::commands::kitsu::TITLE_MATCH_VERSION
    )
}

/// The play resolutions, at the schema this build reads.
#[must_use]
pub(crate) fn play() -> String {
    format!("play:{}:", crate::commands::play_resolution_cache::SCHEMA)
}

/// The availability verdicts, at the version this build reads.
#[must_use]
pub(crate) fn availability() -> String {
    format!(
        "availability:v{}:",
        crate::commands::availability::CACHE_VERSION
    )
}

/// Every family this build reads or writes.
#[must_use]
pub fn live() -> Vec<Family> {
    let mut families: Vec<Family> = [
        KITSU_EPISODES,
        KITSU_RELATIONS,
        KITSU_ANIME_SLUG,
        KITSU_ANIME,
        KITSU_SEARCH,
        KITSU_GONE,
        KITSU_EXTERNAL_IDS,
        KITSU_MAL_MAP,
        WATCHED_AT,
        MAPPING_PLAYED,
        AIRING,
        ANISKIP,
        APPENDED,
        ANILIST_BANNER,
        ANILIST_EPS_THUMBS,
        ANILIST_MEDIA_FOR_MAL,
        ANILIST_MAL_FOR_MEDIA,
        ANILIST_MEDIA_RECORDING_MAL,
        IMAGE_DEAD,
        TRACKER_MAP,
        LISTING_TAGS,
    ]
    .into_iter()
    .map(Family::prefix)
    .collect();
    families.push(Family::prefix(play()));
    families.extend([
        Family::key(KITSU_TRENDING),
        Family::key(KITSU_TOP_RATED),
        Family::key(ANILIST_TRENDING).read_after_expiry(),
        Family::prefix(availability()).read_after_expiry(),
        Family::prefix(mapping()).read_after_expiry(),
        Family::prefix(title_match()).read_after_expiry(),
    ]);
    families
}

/// What becomes of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// A family reads it.
    Keep,
    /// No family declares its key: no reader of this build asks for it.
    Undeclared,
    /// Past its lifetime, in a family nothing reads past it.
    Expired,
}

/// The fate of the row under `key`, `expired` or not, among `families`.
#[must_use]
pub fn fate(key: &str, expired: bool, families: &[Family]) -> Fate {
    let mut holding = families.iter().filter(|f| f.holds(key)).peekable();
    if holding.peek().is_none() {
        return Fate::Undeclared;
    }
    if expired && !holding.any(|f| f.read_after_expiry) {
        return Fate::Expired;
    }
    Fate::Keep
}

/// Whether some family of this build declares `key`.
#[must_use]
pub fn declared(key: &str) -> bool {
    live().iter().any(|f| f.holds(key))
}

#[cfg(test)]
#[path = "families_test.rs"]
mod tests;

#[cfg(test)]
thread_local! {
    static UNDECLARED_ALLOWED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Test builds only: a row this thread stores or reads must belong to a
/// declared family, so code that starts keeping a family the registry
/// does not name fails the suite instead of losing its rows to the
/// launch sweep. A case that seeds rows earlier builds kept — the
/// very rows the sweep is for — runs inside [`undeclared_allowed`].
///
/// Only the app's own code is held to it: a call made from a test file
/// (`*_test.rs`, named by the caller's location) seeds or inspects rows
/// as the case needs, legacy keys included.
#[cfg(test)]
#[track_caller]
pub(crate) fn check(key: &str) {
    let caller = std::panic::Location::caller().file();
    let from_a_case = caller.ends_with("_test.rs");
    let allowed = UNDECLARED_ALLOWED.with(std::cell::Cell::get);
    assert!(
        from_a_case || allowed || declared(key),
        "cache key {key:?} belongs to no family in cache::families::live (called from {caller})"
    );
}

/// Allow rows of undeclared families on this thread for the rest of
/// the case: for a file of cases that key their rows by a fixed key of
/// an earlier version, the key being beside the point. Each case runs
/// on a thread of its own.
#[cfg(test)]
pub(crate) fn allow_undeclared_on_this_thread() {
    UNDECLARED_ALLOWED.with(|a| a.set(true));
}

/// Run `f` with rows of undeclared families allowed on this thread: for
/// a case about rows earlier builds kept.
#[cfg(test)]
pub(crate) fn undeclared_allowed<T>(f: impl FnOnce() -> T) -> T {
    let before = UNDECLARED_ALLOWED.with(|a| a.replace(true));
    let out = f();
    UNDECLARED_ALLOWED.with(|a| a.set(before));
    out
}
