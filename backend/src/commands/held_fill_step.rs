//! The step a retry takes after each run ([`super`]). Split from the
//! registry so each file stays inside the CRAP gate's per-file bar.

/// Retries a held-back fill gets: one per pause it is held in, up to
/// this many pauses.
pub(crate) const MAX_RETRIES: u32 = 3;

/// What a retry does after a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// The fill is in the cache.
    Landed,
    /// Held back again: wait out this pause too and run once more.
    Again,
    /// Not landed, and no pause to wait out — a failure that is not a
    /// rate limit — or out of retries.
    GiveUp,
}

/// The step after run number `attempt` (from 1): `landed` when the fill
/// is now in the cache, `paused` when the gate is holding background
/// fills back.
pub(crate) fn after_run(landed: bool, paused: bool, attempt: u32) -> Step {
    if landed {
        Step::Landed
    } else if paused && attempt < MAX_RETRIES {
        Step::Again
    } else {
        Step::GiveUp
    }
}
