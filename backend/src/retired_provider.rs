//! The boot sweep over what a provider the app no longer walks left in
//! the user's stores.
//!
//! A provider's resolves stamp its show keys on every store a play
//! writes: history rows, the numbering sidecar beside them, watched-at
//! stamps, the reverse mapping to Kitsu, resolution rows, availability
//! verdicts and title matches. Removing the provider from
//! [`crate::provider_set::ProviderSet::shipped`] leaves all of it on
//! disk. The read side already refuses to serve most of it, and
//! nothing reclaims a cache row past its lifetime, so without this it
//! would stay for good. A key is a retired provider's when it names no
//! provider the app walks (a bare id included); nothing here names a
//! provider, so the next removal needs no change.
//!
//! Each launch, before the API serves:
//!
//! 1. **History** ([`history`]). The rule is
//!    [`crate::retired_provider_plan`]: a retired row is removed when a
//!    walked row that recorded the same Kitsu entry resumes over it, by
//!    the same removal the page uses, which takes what is kept about
//!    it beside it. Every other retired row is kept — one still resumed
//!    from, one no walked row recorded the show of, one shown as
//!    nothing — its episode rewritten in the app's own numbering, the
//!    number the page already shows for it. A row is never given a
//!    Kitsu id here: a guess never becomes a row's id.
//! 2. **Numbering** ([`numbering`]). Rows for a retired key no
//!    remaining row reads through go. A walked key's rows stay, rowless
//!    ones included: a resolve stamps them before the show's first row.
//! 3. **Cache** ([`cache`]). Rows only a retired provider's key or
//!    verdict could be read by go: stamps and mappings of a retired key
//!    with no history row; resolution rows whose show key is retired,
//!    which are never replayed; availability rows naming no walked
//!    provider, which are never served; title matches filed under a
//!    retired provider's segment that no remaining row searches.
//!
//! The sweep is idempotent and keeps no marker: what it looks for is
//! the retired data itself, so a launch that finds none writes nothing,
//! and a history or cache restored from an older copy is swept on the
//! next launch. Only one app runs per profile (the shell's
//! single-instance lock), and the stores it writes take their own
//! locks: the history's guard, the numbering store's file lock, and
//! SQLite's for the cache.

mod cache;
mod cache_families;
mod cache_keys;
mod cache_titles;
mod evidence;
mod history;
mod numbering;
mod renumber;

use serde::Serialize;

use crate::app::AppState;
#[cfg(test)]
use crate::scraper::provider::ProviderId;
use crate::scraper::provider::ShowKey;

/// What one sweep changed. Empty on every launch that finds nothing a
/// retired provider left.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RetiredSweepReport {
    /// Titles of the history rows removed because a walked provider's
    /// row for the same show supersedes them.
    pub superseded: Vec<String>,
    /// Titles of the history rows kept, their episode rewritten in the
    /// app's own numbering.
    pub renumbered: Vec<String>,
    /// Numbering-sidecar rows of retired keys removed.
    pub numbering_rows: usize,
    /// Cache rows the cache pass removed; what a removed history row
    /// took with it is not counted here.
    pub cache_rows: usize,
}

impl RetiredSweepReport {
    /// Whether the sweep changed nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Whether `id` names a provider the state walks.
pub(crate) fn walks(state: &AppState, id: &str) -> bool {
    ShowKey::parse(id)
        .provider
        .is_some_and(|p| state.providers.order().contains(&p))
}

/// Run the sweep. Never fails the caller: a store that cannot be read
/// or written is logged and left for the next launch.
#[must_use]
pub fn sweep(state: &AppState) -> RetiredSweepReport {
    let mut report = RetiredSweepReport::default();
    let numbering_before = numbering::retired(state).len();
    if let Err(e) = history::sweep(state, &mut report) {
        tracing::warn!(error = ?e, "retired-provider sweep: history left as it is");
    }
    let remaining = match crate::history::read_all(&state.history_path) {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = ?e, "retired-provider sweep: history unreadable");
            return report;
        }
    };
    if let Err(e) = numbering::sweep(state, &remaining) {
        tracing::warn!(error = ?e, "retired-provider sweep: numbering left as it is");
    }
    report.numbering_rows = numbering_before.saturating_sub(numbering::retired(state).len());
    match cache::sweep(state, &remaining) {
        Ok(n) => report.cache_rows = n,
        Err(e) => tracing::warn!(error = ?e, "retired-provider sweep: cache left as it is"),
    }
    if !report.is_empty() {
        tracing::info!(
            target: "retired_provider",
            superseded = report.superseded.len(),
            renumbered = report.renumbered.len(),
            numbering_rows = report.numbering_rows,
            cache_rows = report.cache_rows,
            "removed what a provider the app no longer walks left behind"
        );
    }
    report
}

#[cfg(test)]
#[path = "retired_provider_test.rs"]
mod tests;
