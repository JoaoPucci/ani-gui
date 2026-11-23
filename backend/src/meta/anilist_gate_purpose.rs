//! What a request through the gate ([`super`]) is for: the stable label
//! its sent line carries, so the app's log says which kind of request
//! spent each request of AniList's budget. A label names the job a
//! request does — a show's episode stills, a tracker's id bridge — and a job
//! several screens share carries one label; the line's class tells a
//! background fill from what the user is looking at. Every transport
//! takes an [`Ask`] and the label has no default: a new caller names
//! its purpose or does not compile. A fetcher that does one job sends
//! under that job's label; one that serves several takes the purpose
//! from its caller. Split from [`super`] so each file stays inside the
//! CRAP gate's per-file bar.

/// Why a request goes to AniList. One per job a request does, never a
/// catch-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Purpose {
    /// The home page's Trending Now feed.
    Trending,
    /// A show's banner: a Continue strip's warm asks it for every warmed
    /// show Kitsu has no cover for, a detail read for a show whose Kitsu
    /// entry has none.
    Banner,
    /// A show's streaming episodes: episode stills and the listing.
    Stills,
    /// A show's airing status and next episode.
    Airing,
    /// An id bridge between AniList and MAL that resolves the entry a
    /// tracker read or write addresses: the list editor, progress
    /// write-back and the episode map's resolution all ask it.
    TrackerIds,
    /// The Watch Later rail's bridge from MAL ids to AniList's.
    WatchLaterIds,
    /// Every AniList media recording a MAL id, which the episode map
    /// reads for ownership.
    TrackerMap,
    /// The signed-in account's profile.
    Account,
    /// The signed-in account's list.
    UserList,
    /// A write to an entry on the account's list.
    ListWrite,
    /// The lookup and the mutation that delete an entry from the list.
    ListDelete,
    /// The account's entry for one show: status and progress.
    ListEntry,
    /// What AniList records for one show the account tracks.
    TrackedMedia,
    /// The evidence the episode map reads for one show.
    MediaEvidence,
}

impl Purpose {
    /// The label the sent line carries.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Trending => "trending",
            Self::Banner => "banner",
            Self::Stills => "stills",
            Self::Airing => "airing",
            Self::TrackerIds => "tracker_ids",
            Self::WatchLaterIds => "watch_later_ids",
            Self::TrackerMap => "tracker_map",
            Self::Account => "account",
            Self::UserList => "user_list",
            Self::ListWrite => "list_write",
            Self::ListDelete => "list_delete",
            Self::ListEntry => "list_entry",
            Self::TrackedMedia => "tracked_media",
            Self::MediaEvidence => "media_evidence",
        }
    }
}

/// One request's label: its [`Purpose`], and how many shows it carries
/// when it asks about shows by id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ask {
    /// Why it is sent.
    pub(crate) purpose: Purpose,
    /// How many shows it asks about, counted as the ids it carries;
    /// `None` for a request not keyed by show — the trending feed, the
    /// account and its list.
    pub(crate) shows: Option<usize>,
}

impl Ask {
    /// A request about `shows` shows.
    pub(crate) const fn shows(purpose: Purpose, shows: usize) -> Self {
        Self {
            purpose,
            shows: Some(shows),
        }
    }

    /// A request about one show.
    pub(crate) const fn one(purpose: Purpose) -> Self {
        Self::shows(purpose, 1)
    }

    /// A request not keyed by show.
    pub(crate) const fn unkeyed(purpose: Purpose) -> Self {
        Self {
            purpose,
            shows: None,
        }
    }
}

#[cfg(test)]
#[path = "anilist_gate_purpose_test.rs"]
mod tests;
