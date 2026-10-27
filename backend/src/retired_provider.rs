//! The boot sweep over what a provider the app no longer walks left in
//! the user's stores.
//!
//! See [`crate::retired_provider_plan`] for the rule over history rows.

use serde::Serialize;

use crate::app::AppState;
#[cfg(test)]
use crate::scraper::provider::ProviderId;

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
    /// Numbering-sidecar rows removed.
    pub numbering_rows: usize,
    /// Cache rows removed.
    pub cache_rows: usize,
}

/// Run the sweep. Never fails the caller.
#[must_use]
pub fn sweep(state: &AppState) -> RetiredSweepReport {
    let _ = state;
    RetiredSweepReport::default()
}

#[cfg(test)]
#[path = "retired_provider_test.rs"]
mod tests;
