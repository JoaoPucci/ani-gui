//! What a download's sidecar phase is given, chosen once as the phase
//! starts from whether playback is live.

use std::time::Duration;

/// How many subtitle tracks a download fetches at once beside its
/// transfer, chosen once as the phase starts: one while playback is
/// live, so the phase has at most one fetch in flight;
/// [`SIDECAR_FETCH_CONCURRENCY`](crate::commands::download::SIDECAR_FETCH_CONCURRENCY)
/// otherwise. Across downloads, [`SIDECAR_LANE`](super::SIDECAR_LANE) bounds how many are
/// in flight while playback is live.
#[must_use]
pub(crate) fn sidecar_concurrency(playback_live: bool) -> usize {
    if playback_live {
        1
    } else {
        crate::commands::download::SIDECAR_FETCH_CONCURRENCY
    }
}

/// The sidecar phase's deadline, chosen once as the phase starts:
/// [`SIDECAR_PHASE_DEADLINE`](crate::commands::download::SIDECAR_PHASE_DEADLINE)
/// for a host that stalls, [`SIDECAR_PHASE_DEADLINE_LIVE`](super::SIDECAR_PHASE_DEADLINE_LIVE) while
/// playback is live.
#[must_use]
pub(crate) fn sidecar_phase_deadline(playback_live: bool) -> Duration {
    if playback_live {
        super::SIDECAR_PHASE_DEADLINE_LIVE
    } else {
        crate::commands::download::SIDECAR_PHASE_DEADLINE
    }
}
