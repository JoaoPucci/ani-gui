//! The transient verdict of an exhausted alias walk — split from
//! `play_native_resolve` so each file stays inside the complexity
//! ratchet's per-file bar.
//!
//! A walk meets failures it does not stop on: a search that died or
//! could not be read, a pick whose probes died, an episode chain that
//! did not serve. None ends the walk — the next alias may carry the
//! show — and when every alias is spent, one of them is the verdict
//! the caller sees and the breaker hears. This module keeps them and
//! chooses.

use crate::error::AniError;
use crate::scraper::provider::Provider;

use super::play_native_resolve::NativeError;

/// How far a walk had got when a transient failure met it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum WalkStage {
    /// The alias's own search.
    Search,
    /// The search answered; the pick's probes of its candidates did
    /// not.
    Pick,
    /// A show was picked; its episode chain did not serve.
    Chain,
}

/// The transient failures a walk has met so far, reduced to the one
/// that will be its verdict.
#[derive(Debug, Default)]
pub(super) struct Transients {
    /// Whether any transient failure was met at all.
    met: bool,
    /// A non-blocking status an episode chain answered, kept for the
    /// exhausted walk to surface as itself: the breaker reads it as
    /// the provider answering, where the transport failure
    /// substituted for it would read as distress.
    chain_status: Option<AniError>,
    /// The instant of the last failing attempt — the moment the
    /// breaker record carries, whichever failure is surfaced
    /// ([`NativeError::failed_at`]).
    last_failure_at: Option<tokio::time::Instant>,
}

impl Transients {
    /// Note a transient failure met at `stage`, observed by the
    /// attempt that started at `at`.
    pub(super) fn note(&mut self, stage: WalkStage, error: AniError, at: tokio::time::Instant) {
        self.met = true;
        self.last_failure_at = Some(at);
        if stage == WalkStage::Chain && matches!(error, AniError::Upstream { .. }) {
            self.chain_status = Some(error);
        }
    }

    /// The exhausted walk's transient verdict: `None` when no
    /// transient failure was met, else a transport failure — or the
    /// status an episode chain answered, which stands as itself —
    /// stamped with the last failing attempt's instant and never a
    /// clean miss.
    pub(super) fn verdict(self) -> Option<NativeError> {
        self.met.then(|| NativeError {
            error: self.chain_status.unwrap_or(AniError::Network),
            clean_miss: false,
            failed_at: self.last_failure_at,
        })
    }
}

/// The post-admission start of the client's most recent attempt — the
/// moment a transient failure is stamped with — or now, for a
/// transport that tracks none.
pub(super) fn attempt_at<P: Provider + ?Sized>(client: &P) -> tokio::time::Instant {
    client
        .last_attempt_at()
        .unwrap_or_else(tokio::time::Instant::now)
}

#[cfg(test)]
#[path = "play_native_transient_test.rs"]
mod tests;
