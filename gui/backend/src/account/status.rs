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

#[cfg(test)]
mod tests {
    use super::*;

    /// Cross-product of every unified variant round-tripping through
    /// AniList's enum string. Catches accidental enum drift on either
    /// side (e.g. a code sweep renaming PLANNING → PLAN).
    #[test]
    fn anilist_round_trip_every_variant() {
        for &s in &[
            ListStatus::Planning,
            ListStatus::Watching,
            ListStatus::Completed,
            ListStatus::Paused,
            ListStatus::Dropped,
            ListStatus::Rewatching,
        ] {
            let wire = s.to_anilist();
            assert_eq!(
                ListStatus::from_anilist(wire),
                Some(s),
                "round-trip via {wire}"
            );
        }
    }

    #[test]
    fn anilist_uses_current_for_watching_and_repeating_for_rewatching() {
        // These two are the renames most easily fat-fingered: AniList
        // calls them CURRENT and REPEATING where MAL calls them
        // watching + watching+is_rewatching. Pin the wire values so a
        // future refactor doesn't silently swap them.
        assert_eq!(ListStatus::Watching.to_anilist(), "CURRENT");
        assert_eq!(ListStatus::Rewatching.to_anilist(), "REPEATING");
        assert_eq!(ListStatus::Planning.to_anilist(), "PLANNING");
        assert_eq!(ListStatus::Completed.to_anilist(), "COMPLETED");
        assert_eq!(ListStatus::Paused.to_anilist(), "PAUSED");
        assert_eq!(ListStatus::Dropped.to_anilist(), "DROPPED");
    }

    #[test]
    fn anilist_from_unknown_returns_none() {
        assert_eq!(ListStatus::from_anilist(""), None);
        assert_eq!(ListStatus::from_anilist("PLAN"), None);
        assert_eq!(ListStatus::from_anilist("planning"), None); // case-sensitive
    }

    /// MAL's wire enum + the is_rewatching flag — pin every variant
    /// individually because the two-field collapse to one unified
    /// enum is exactly the kind of mapping that breaks silently.
    #[test]
    fn mal_to_pairs_every_variant() {
        assert_eq!(ListStatus::Planning.to_mal(), ("plan_to_watch", false));
        assert_eq!(ListStatus::Watching.to_mal(), ("watching", false));
        assert_eq!(ListStatus::Completed.to_mal(), ("completed", false));
        assert_eq!(ListStatus::Paused.to_mal(), ("on_hold", false));
        assert_eq!(ListStatus::Dropped.to_mal(), ("dropped", false));
        assert_eq!(ListStatus::Rewatching.to_mal(), ("watching", true));
    }

    #[test]
    fn mal_from_distinguishes_watching_from_rewatching_via_flag() {
        // Both statuses share the wire string "watching" — the
        // is_rewatching flag is the only differentiator. Pin both
        // branches explicitly.
        assert_eq!(
            ListStatus::from_mal("watching", false),
            Some(ListStatus::Watching)
        );
        assert_eq!(
            ListStatus::from_mal("watching", true),
            Some(ListStatus::Rewatching)
        );
    }

    #[test]
    fn mal_from_round_trips_via_to_pair() {
        for &s in &[
            ListStatus::Planning,
            ListStatus::Watching,
            ListStatus::Completed,
            ListStatus::Paused,
            ListStatus::Dropped,
            ListStatus::Rewatching,
        ] {
            let (status, flag) = s.to_mal();
            assert_eq!(
                ListStatus::from_mal(status, flag),
                Some(s),
                "round-trip via ({status}, {flag})"
            );
        }
    }

    #[test]
    fn mal_from_ignores_rewatching_flag_on_non_watching_statuses() {
        // is_rewatching only carries meaning when status==watching.
        // For other statuses, the flag must be ignored (some users
        // may have stale is_rewatching=true on completed entries).
        assert_eq!(
            ListStatus::from_mal("completed", true),
            Some(ListStatus::Completed)
        );
        assert_eq!(
            ListStatus::from_mal("plan_to_watch", true),
            Some(ListStatus::Planning)
        );
        assert_eq!(
            ListStatus::from_mal("dropped", true),
            Some(ListStatus::Dropped)
        );
    }

    #[test]
    fn mal_from_unknown_status_returns_none() {
        assert_eq!(ListStatus::from_mal("watching_paused", false), None);
        assert_eq!(ListStatus::from_mal("", false), None);
        // case-sensitive
        assert_eq!(ListStatus::from_mal("Watching", false), None);
    }

    /// Serde lower-cases the snake-case enum names — pin the wire
    /// form so the JSON the frontend's account store receives stays
    /// stable across refactors.
    #[test]
    fn unified_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&ListStatus::Planning).unwrap(),
            "\"planning\""
        );
        assert_eq!(
            serde_json::to_string(&ListStatus::Rewatching).unwrap(),
            "\"rewatching\""
        );
    }
}
