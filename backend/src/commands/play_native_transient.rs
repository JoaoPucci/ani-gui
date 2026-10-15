//! The transient verdict of an exhausted alias walk — split from
//! `play_native_resolve` so each file stays inside the complexity
//! ratchet's per-file bar, and shared with the pick walk the
//! availability probes and the range download's start resolve through
//! (`play_native_walk`).
//!
//! A walk meets failures it does not stop on: a search that died or
//! could not be read, a pick whose probes died, an episode chain that
//! did not serve. None ends the walk — the next alias may carry the
//! show — and when every alias is spent, one of them is the verdict
//! the caller sees and the breaker hears. This module keeps them and
//! chooses: the one from furthest along the walk, since a chain that
//! did not serve says the show was found, which a search that died on
//! another alias cannot unsay — and the connection that search would
//! blame is the one the other search got through on; within a stage,
//! the one that says most — a status a host answered over a page the
//! client could not read over a connection that dropped — and of
//! equals the first. Neither walk substitutes one failure for
//! another: what it met is what it reports, so the connection is
//! blamed when a transport failure is the furthest a walk got.

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
    /// The failure kept so far, with the stage it was met at.
    kept: Option<(WalkStage, AniError)>,
    /// The instant of the last failing attempt — the moment the
    /// breaker record carries, whichever failure is surfaced
    /// ([`NativeError::failed_at`]).
    last_failure_at: Option<tokio::time::Instant>,
}

impl Transients {
    /// Note a transient failure met at `stage`, observed by the
    /// attempt that started at `at`. It replaces the failure kept so
    /// far when it comes from further along the walk, or from the
    /// same stage and says more ([`rank`]); of equals the first stays.
    pub(super) fn note(&mut self, stage: WalkStage, error: AniError, at: tokio::time::Instant) {
        self.note_at(stage, error, Some(at));
    }

    /// [`Self::note`] for a failure that may have been produced by no
    /// attempt — `at` is `None` when the provider stamps none on it.
    pub(super) fn note_at(
        &mut self,
        stage: WalkStage,
        error: AniError,
        at: Option<tokio::time::Instant>,
    ) {
        let at = at.unwrap_or_else(tokio::time::Instant::now);
        self.last_failure_at = Some(at);
        let louder = match &self.kept {
            None => true,
            Some((kept_stage, kept)) => (stage, rank(&error)) > (*kept_stage, rank(kept)),
        };
        if louder {
            self.kept = Some((stage, error));
        }
    }

    /// The exhausted walk's transient verdict: `None` when no
    /// transient failure was met, else the failure kept, under its
    /// own name, stamped with the last failing attempt's instant and
    /// never a clean miss.
    pub(super) fn verdict(self) -> Option<NativeError> {
        self.kept.map(|(_, error)| NativeError {
            error,
            clean_miss: false,
            failed_at: self.last_failure_at,
        })
    }
}

/// How much a transient failure says, among those met at one stage:
/// a status a host answered says the provider answered — the breaker
/// hears health where every other kind is distress — a page the
/// client could not read says the site answered, and a connection
/// that dropped, a timeout, or a stream server that did not serve say
/// the least.
fn rank(error: &AniError) -> u8 {
    match error {
        AniError::Upstream { .. } => 2,
        AniError::ParseFailed { .. } => 1,
        _ => 0,
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
