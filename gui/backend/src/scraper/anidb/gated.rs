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
        }
    }
}

#[async_trait::async_trait]
impl<F: AnidbFetch> AnidbFetch for GatedFetch<'_, F> {
    async fn get(&self, url: &str) -> Result<FetchResponse> {
        self.inner.get(url).await
    }
}

#[cfg(test)]
#[path = "gated_test.rs"]
mod tests;
