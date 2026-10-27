//! An availability row from a listing whose rows past Kitsu's count
//! no reading reached ([`crate::commands::play_native::PickedShow::unread_past`]).
//!
//! Those rows are listed as they came: they may be extras the provider
//! appended after the show's last episode or more of its episodes, and
//! nothing says which. A cap taken from the listing would count them as
//! episodes, so such a listing gives no count; and what it could not
//! read does not write over a count a reading proved, which stands.
//! Split from [`super::availability`] for the per-file complexity bar.

use super::availability::AvailabilityResponse;
use crate::app::AppState;
use crate::cache::meta_cache_get;

/// What a stamp for a served stream writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServedStamp {
    /// The row with this count and the listing's extras.
    Count(u32),
    /// The row that says the show is there and no count.
    NoCount,
    /// Nothing: the row that stands stays as it is.
    Standing,
}

/// The stamp for a served stream whose listing's cap is `cap`. The cap
/// is a count only in sub (`dub` names the dubbed slice) and only where
/// the rows past Kitsu's count were read (`count_unread` says they were
/// not). An unread count leaves a row that stands with a count of its
/// own (`standing_counted`) as it is.
#[must_use]
pub(crate) fn served_stamp(
    cap: Option<u32>,
    dub: bool,
    count_unread: bool,
    standing_counted: bool,
) -> ServedStamp {
    if count_unread && standing_counted {
        return ServedStamp::Standing;
    }
    match cap {
        Some(cap) if !dub && !count_unread => ServedStamp::Count(cap),
        _ => ServedStamp::NoCount,
    }
}

/// The positive row with a count that stands at `row`, if one does.
pub(crate) fn standing_counted_row(state: &AppState, row: &str) -> Option<AvailabilityResponse> {
    let body = meta_cache_get(&state.cache_pool, row).ok().flatten()?;
    let stood: AvailabilityResponse = serde_json::from_str(&body).ok()?;
    (stood.available && stood.episode_count.is_some()).then_some(stood)
}

/// The cap and extras a probe's pick gives: the listing's, unless the
/// rows past Kitsu's count were not read, which gives neither.
#[must_use]
pub(crate) fn probe_counts(
    picked: &crate::commands::play_native::PickedShow,
) -> (Option<u32>, Vec<String>) {
    if picked.unread_past.is_some() {
        return (None, Vec::new());
    }
    (
        crate::commands::play_native_numbering::kitsu_episode_cap(&picked.episodes),
        crate::commands::play_native_numbering::extra_episode_tags(&picked.episodes),
    )
}
