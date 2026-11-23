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

/// The sidecar phase's deadline for a listing of `tracks`, chosen once
/// as the phase starts:
/// [`SIDECAR_PHASE_DEADLINE`](crate::commands::download::SIDECAR_PHASE_DEADLINE)
/// for a host that stalls, [`SIDECAR_PHASE_DEADLINE_LIVE`](super::SIDECAR_PHASE_DEADLINE_LIVE) while
/// playback is live — both sized for a listing of
/// [`TRACKS_THE_DEADLINE_COVERS`] — plus each further track's share,
/// since a listing of every language a provider offers runs to dozens:
/// a [`SEGMENT_REFILL`](crate::proxy::host_budget::SEGMENT_REFILL)
/// while nothing plays, the wait for its token once the host's burst
/// is spent; and while playback is live, where the tracks go one at a
/// time behind the player's need, the share of the live deadline each
/// of the sixteen it was sized for has.
#[must_use]
pub(crate) fn sidecar_phase_deadline(playback_live: bool, tracks: usize) -> Duration {
    let (base, step) = if playback_live {
        let live = super::SIDECAR_PHASE_DEADLINE_LIVE;
        (live, live / 16)
    } else {
        (
            crate::commands::download::SIDECAR_PHASE_DEADLINE,
            crate::proxy::host_budget::SEGMENT_REFILL,
        )
    };
    let past = u32::try_from(tracks.saturating_sub(TRACKS_THE_DEADLINE_COVERS)).unwrap_or(u32::MAX);
    base.saturating_add(step.saturating_mul(past))
}

/// The listing the base deadlines were sized for.
const TRACKS_THE_DEADLINE_COVERS: usize = 16;
