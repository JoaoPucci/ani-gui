//! How long a positive availability row may be served for a show that
//! is still airing.
//!
//! A positive row carries the provider's episode count, and on a
//! weekly show that count goes stale the moment the next episode
//! drops. A flat ongoing window written an hour before a drop holds
//! the old count for most of a day after it, so the window is cut at
//! the next scheduled airing — plus a grace for the provider to list
//! the episode. The cut only ever shortens: a show with no schedule
//! keeps the window it had.

/// Never shorter than this: the cut must not turn every page open
/// into a provider request while a schedule row is stale.
pub(crate) const FLOOR_SECS: u64 = 60 * 60;

/// How long past a scheduled airing the provider is given to list it.
pub(crate) const GRACE_SECS: u64 = 60 * 60;

/// `base` cut at the next airing plus [`GRACE_SECS`], never below
/// [`FLOOR_SECS`] (nor above `base`). No schedule → `base`.
#[must_use]
pub(crate) fn bounded_by_next_airing(base: u64, next_airing_at: Option<u64>, now: u64) -> u64 {
    let Some(at) = next_airing_at else {
        return base;
    };
    let until_listed = at.saturating_sub(now).saturating_add(GRACE_SECS);
    let cut = if at <= now {
        FLOOR_SECS
    } else {
        until_listed.max(FLOOR_SECS)
    };
    cut.min(base)
}

#[cfg(test)]
#[path = "availability_ttl_test.rs"]
mod tests;
