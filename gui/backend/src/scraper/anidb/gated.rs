//! A gate-admitting [`AnidbFetch`] decorator: every provider request
//! — search, detail page, episodes, languages, embed, master — passes
//! through [`ScraperGate::admit`] before the transport runs. The
//! resolve walk's own pre-flight admit paces one slot per alias; this
//! wrapper is what holds the per-REQUEST spacing contract when a
//! background resolve fans out into candidate probes and the episode
//! chain.

use crate::error::Result;
use crate::scraper::gate::{ScrapePriority, ScraperGate};

use super::{AnidbFetch, FetchResponse};

/// See the module docs. Interactive admits are a no-op by the gate's
/// own contract, so click-path latency is untouched; background
/// requests each take a paced slot and are refused while the breaker
/// is open.
pub struct GatedFetch<'g, F> {
    inner: F,
    gate: Option<&'g ScraperGate>,
    priority: ScrapePriority,
    /// The half-open trial sanction, when this chain's first admit
    /// took the trial. One GatedFetch is one logical resolve chain,
    /// so carrying the sanction here lets the chain's remaining
    /// fetches ride the trial instead of being refused by it.
    sanction: std::sync::Mutex<Option<tokio::time::Instant>>,
    /// When the chain's latest attempt started, taken after
    /// admission. The breaker's stale filters compare an outcome's
    /// timestamp against the last recovery, so evidence stamped with
    /// the chain's START is discarded whenever a concurrent resolve
    /// recorded recovery mid-chain; the latest attempt is the fetch
    /// that actually observed the outcome. Post-admission, so paced
    /// queueing never backdates it.
    last_attempt_at: std::sync::Mutex<Option<tokio::time::Instant>>,
}

impl<'g, F> GatedFetch<'g, F> {
    /// Wrap `inner` so every request admits through `gate` at
    /// `priority`. A `None` gate is a plain passthrough (tests, and
    /// callers that pace elsewhere).
    pub fn new(inner: F, gate: Option<&'g ScraperGate>, priority: ScrapePriority) -> Self {
        Self {
            inner,
            gate,
            priority,
            sanction: std::sync::Mutex::new(None),
            last_attempt_at: std::sync::Mutex::new(None),
        }
    }

    /// The post-admission start of the chain's latest attempt —
    /// what a breaker outcome produced by this chain should be
    /// timestamped with. `None` before the first attempt.
    pub fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        *self.last_attempt_at.lock().expect("attempt stamp lock")
    }
}

#[async_trait::async_trait]
impl<F: AnidbFetch> AnidbFetch for GatedFetch<'_, F> {
    async fn get(&self, url: &str) -> Result<FetchResponse> {
        if let Some(gate) = self.gate {
            // A refusal only happens for background priority while
            // the breaker is open. It keeps its identity: mapped to
            // Network it would be recorded as an upstream failure,
            // and every background warmup would refresh the open
            // breaker's cooldown without contacting the provider.
            let held = *self.sanction.lock().expect("sanction lock");
            let granted = gate
                .admit_chained(self.priority, held)
                .await
                .map_err(|_| crate::error::AniError::GateRefused)?;
            *self.sanction.lock().expect("sanction lock") = granted;
        }
        *self.last_attempt_at.lock().expect("attempt stamp lock") =
            Some(tokio::time::Instant::now());
        self.inner.get(url).await
    }
}

#[cfg(test)]
#[path = "gated_test.rs"]
mod tests;
