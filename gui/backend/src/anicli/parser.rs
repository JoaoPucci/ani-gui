//! The progress-line grammar the loading overlay renders.
//!
//! Historically this module parsed `ani-cli`'s stdout (search
//! results, debug-mode stream dumps, stderr classification). The
//! native resolver replaced every spawn, so what remains is the one
//! shape that outlived the script: the [`ProgressLine`] events the
//! resolve pipeline emits over SSE while the user waits.

use serde::{Deserialize, Serialize};

/// One step of progress emitted while the backend resolves a stream.
/// Forwarded to the renderer's loading overlay over SSE so the user
/// sees something happening during the wait.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProgressLine {
    /// A startup banner (`Searching anidb.app…`).
    Banner {
        /// The full banner text.
        text: String,
    },
    /// `<provider> links fetched` — the provider query succeeded and
    /// episode links are in hand.
    LinksFetched {
        /// Provider label (`anidb.app`).
        provider: String,
    },
    /// Any other status text; passed through verbatim so the overlay
    /// can fall back to raw-output mode for steps without a dedicated
    /// variant.
    Other {
        /// The original line, trimmed.
        text: String,
    },
}

#[cfg(test)]
#[allow(missing_docs)]
mod tests {
    use super::*;

    /// The SSE overlay decodes `{"kind": "...", ...}` snake_case
    /// events; this is a wire contract with the frontend, not an
    /// implementation detail.
    #[test]
    fn progress_line_serializes_with_snake_case_kind_tag() {
        let banner = serde_json::to_string(&ProgressLine::Banner { text: "hi".into() }).unwrap();
        assert_eq!(banner, r#"{"kind":"banner","text":"hi"}"#);
        let links = serde_json::to_string(&ProgressLine::LinksFetched {
            provider: "anidb.app".into(),
        })
        .unwrap();
        assert_eq!(links, r#"{"kind":"links_fetched","provider":"anidb.app"}"#);
    }
}
