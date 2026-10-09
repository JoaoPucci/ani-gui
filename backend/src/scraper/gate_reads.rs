//! The gate's pure reads of its own state — refusing, recovered, and
//! the breaker check that hands out the half-open trial; split from
//! [`super`] so each file stays inside the CRAP gate's per-file bar.

use super::*;

/// Whether the provider is refusing at `now`: the breaker open, or an
/// advertised pause running. Pure, so the read's contract can be
/// stated as a property beside [`recovered_at`].
pub(super) fn refusing_at(
    open_until: Option<Instant>,
    paused_until: Option<Instant>,
    now: Instant,
) -> bool {
    open_until.is_some_and(|until| now < until) || paused_until.is_some_and(|paused| now < paused)
}

/// Whether the provider has recovered at `now`: the provider has
/// `answered` through the gate, nothing has failed since it last
/// did (`consecutive_failures` is zero), the breaker is closed — by
/// a success, never merely cooled down — and no advertised pause is
/// still running. Pure, the mirror of [`refusing_at`]: recovered
/// implies not refusing, and a breaker past its cooldown without a
/// success is neither, as is a fresh gate nothing has answered
/// through, as is a gate with a failure run under way. The gate
/// lives in the process while the verdicts it stands behind live on
/// disk, so "never opened" alone would count a provider recovered on
/// an app started during its outage; and the verdicts a recovered
/// gate stands behind are served without a request, so a failure run
/// short of the threshold would otherwise never grow past it.
pub(super) fn recovered_at(
    answered: bool,
    consecutive_failures: u32,
    open_until: Option<Instant>,
    paused_until: Option<Instant>,
    now: Instant,
) -> bool {
    answered
        && consecutive_failures == 0
        && open_until.is_none()
        && paused_until.is_none_or(|paused| now >= paused)
}

/// Breaker check under the gate lock: refuses while the breaker is
/// open, and once the cooldown elapses hands the half-open trial role
/// to exactly one caller — everyone else stays refused until the
/// trial reports or goes stale. The returned stamp is the sanction
/// itself: the caller holds it as proof, and the exemption lasts only
/// while `half_open_trial_at` still equals it — any state clearing
/// retires the sanction along with the cycle that granted it. A trial
/// whose future was dropped stops blocking after
/// [`HALF_OPEN_TRIAL_STALE`]. `consecutive_failures` is left as-is,
/// so a single failed trial snaps the breaker shut.
pub(super) fn breaker_gate(
    s: &mut GateState,
    now: Instant,
    held: Option<Instant>,
) -> Result<Option<Instant>, GateClosed> {
    let Some(until) = s.open_until else {
        return Ok(None);
    };
    // The holder of the outstanding trial's sanction IS the trial:
    // its chain's further admits pass on the same stamp, for as long
    // as the gate still holds exactly that stamp.
    if held.is_some() && held == s.half_open_trial_at {
        return Ok(held);
    }
    if now < until {
        return Err(GateClosed);
    }
    if let Some(t) = s.half_open_trial_at {
        if now - t < HALF_OPEN_TRIAL_STALE {
            return Err(GateClosed);
        }
    }
    s.half_open_trial_at = Some(now);
    Ok(Some(now))
}
