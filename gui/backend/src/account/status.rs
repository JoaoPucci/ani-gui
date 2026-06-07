//! Unified [`ListStatus`] enum + provider-native translation helpers.
//!
//! Translation table mirrors `.planning/account-integration.md` §4.3:
//!
//! | Unified       | AniList     | MAL (status, is_rewatching) |
//! |---------------|-------------|-----------------------------|
//! | `Planning`    | `PLANNING`  | `("plan_to_watch", _)`     |
//! | `Watching`    | `CURRENT`   | `("watching", false)`       |
//! | `Completed`   | `COMPLETED` | `("completed", _)`         |
//! | `Paused`      | `PAUSED`    | `("on_hold", _)`           |
//! | `Dropped`     | `DROPPED`   | `("dropped", _)`           |
//! | `Rewatching`  | `REPEATING` | `("watching", true)`        |

use serde::{Deserialize, Serialize};

/// Unified watch-status value across providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListStatus {
    /// User intends to watch — AniList `PLANNING`, MAL `plan_to_watch`.
    Planning,
    /// First-time watching now.
    Watching,
    /// Finished at least one full run.
    Completed,
    /// Started but paused.
    Paused,
    /// Started but abandoned.
    Dropped,
    /// Watching again after `Completed`.
    Rewatching,
}

impl ListStatus {
    /// Translate to AniList's native `MediaListStatus` enum string
    /// (used in GraphQL queries and mutations).
    #[must_use]
    pub fn to_anilist(self) -> &'static str {
        // Stub — replaced in the next commit's green pair.
        "PLANNING"
    }

    /// Inverse of [`Self::to_anilist`]. `None` when the value isn't a
    /// known AniList status — caller decides whether to log + skip or
    /// hard-fail.
    #[must_use]
    pub fn from_anilist(_s: &str) -> Option<Self> {
        // Stub — replaced in the next commit's green pair.
        None
    }

    /// Translate to MAL's `(status, is_rewatching)` pair. MAL splits
    /// the rewatching state across two fields (a status string and a
    /// boolean) — the unified enum collapses them, so the inverse must
    /// take both back as inputs.
    #[must_use]
    pub fn to_mal(self) -> (&'static str, bool) {
        // Stub — replaced in the next commit's green pair.
        ("plan_to_watch", false)
    }

    /// Inverse of [`Self::to_mal`].
    #[must_use]
    pub fn from_mal(_status: &str, _is_rewatching: bool) -> Option<Self> {
        // Stub — replaced in the next commit's green pair.
        None
    }
}
