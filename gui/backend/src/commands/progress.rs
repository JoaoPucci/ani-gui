//! The vocabulary the resolver speaks to the loading overlay.
//!
//! Resolving a stream takes several seconds — a search, a listing, an
//! episode's languages row, then the embed — so the play and download
//! commands report their position as they go and the API layer relays
//! each one as an SSE `progress` event.
//!
//! Every variant is structured rather than prose. The resolver runs in
//! the backend, which never returns localized strings: it names the
//! provider or the show it matched, and the renderer interpolates that
//! into its own copy.

use serde::{Deserialize, Serialize};

/// One step of a native resolve, as the renderer's loading overlay
/// consumes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProgressLine {
    /// The resolver started searching a provider.
    Searching {
        /// Provider label (`anidb.app`).
        provider: String,
    },
    /// The resolver picked a show out of the search results.
    Matched {
        /// The picked show's display title.
        title: String,
    },
    /// The resolver has the episode's embeds in hand and is about to
    /// extract a playable URL from them.
    LinksFetched {
        /// Provider label (`anidb.app`).
        provider: String,
    },
}
