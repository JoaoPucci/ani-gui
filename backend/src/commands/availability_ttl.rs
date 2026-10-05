//! How long an availability row may be served for a show that is
//! still airing.
//!
//! A positive row carries the provider's episode count, and on a
//! weekly show that count goes stale the moment the next episode
//! drops. A flat ongoing window written an hour before a drop holds
//! the old count for most of a day after it, so the window is cut at
//! the next scheduled airing — plus a grace for the provider to list
//! the episode. The cut only ever shortens: a show with no schedule
//! keeps the window it had. A negative row is cut the same way with a
//! longer grace, [`NEGATIVE_GRACE_SECS`].

/// Never shorter than this: the cut must not turn every page open
/// into a provider request while a schedule row is stale.
pub(crate) const FLOOR_SECS: u64 = 60 * 60;

/// How long past a scheduled airing the provider is given to list it.
pub(crate) const GRACE_SECS: u64 = 60 * 60;

/// `base` cut at the next airing plus [`GRACE_SECS`], never below
/// [`FLOOR_SECS`] (nor above `base`). No schedule → `base`.
#[must_use]
pub(crate) fn bounded_by_next_airing(base: u64, next_airing_at: Option<u64>, now: u64) -> u64 {
    bounded_with_grace(base, next_airing_at, now, GRACE_SECS)
}

/// [`bounded_by_next_airing`] with the grace past the airing given.
fn bounded_with_grace(base: u64, next_airing_at: Option<u64>, now: u64, grace: u64) -> u64 {
    let Some(at) = next_airing_at else {
        return base;
    };
    let until_listed = at.saturating_sub(now).saturating_add(grace);
    let cut = if at <= now {
        FLOOR_SECS
    } else {
        until_listed.max(FLOOR_SECS)
    };
    cut.min(base)
}

/// Most episodes one drop moves the schedule's aired count by: one a
/// week, two for a double-episode premiere. A wider gap says the
/// schedule and the listing number different entries, not that the
/// provider is behind.
pub(crate) const MAX_DROP_EPISODES: u32 = 2;

/// The airing a positive row is bounded by. When the schedule says an
/// episode aired that the listing's count does not carry yet — AniList
/// moved on to the following week while the provider has not listed
/// the drop — the drop has already happened, so `now`; otherwise
/// `next_airing_at`. Both counts are whole episodes in the entry's own
/// numbering; extras never enter `count`.
#[must_use]
pub(crate) fn next_airing_for_count(
    next_airing_at: Option<u64>,
    _count: Option<u32>,
    _aired: Option<u32>,
    _now: u64,
) -> Option<u64> {
    next_airing_at
}

/// A positive row already in the cache when the schedule arrives,
/// re-cut as if it had been written knowing it: the row's remaining
/// life is bounded by [`bounded_by_next_airing`], and the new total
/// TTL (`age` + that) is returned when it is shorter than `ttl`.
///
/// `None` — leave the row as it is — when there is no schedule, when
/// the row has expired, and when the cut would not shorten it. A
/// finished show's long row is safe by the first: it has no next
/// airing.
#[must_use]
pub(crate) fn rescheduled_ttl(
    ttl: u64,
    age: u64,
    next_airing_at: Option<u64>,
    now: u64,
) -> Option<u64> {
    rescheduled_with_grace(ttl, age, next_airing_at, now, GRACE_SECS)
}

/// The re-cut both row kinds share, `grace` past the airing.
fn rescheduled_with_grace(
    ttl: u64,
    age: u64,
    next_airing_at: Option<u64>,
    now: u64,
    grace: u64,
) -> Option<u64> {
    if age >= ttl {
        return None;
    }
    let remaining = ttl - age;
    let cut = bounded_with_grace(remaining, next_airing_at, now, grace);
    (cut < remaining).then_some(age + cut)
}

/// How long past a scheduled airing a negative row stands: the
/// catalogue adds a show more slowly than it lists a new episode.
pub(crate) const NEGATIVE_GRACE_SECS: u64 = 3 * 60 * 60;

/// [`rescheduled_ttl`] for a negative row: the cut sits
/// [`NEGATIVE_GRACE_SECS`] past the airing — so a premiere moved
/// earlier cuts the negative its old date sized.
#[must_use]
pub(crate) fn rescheduled_negative_ttl(
    ttl: u64,
    age: u64,
    next_airing_at: Option<u64>,
    now: u64,
) -> Option<u64> {
    rescheduled_with_grace(ttl, age, next_airing_at, now, NEGATIVE_GRACE_SECS)
}

#[cfg(test)]
#[path = "availability_ttl_test.rs"]
mod tests;
