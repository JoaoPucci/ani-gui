//! The notices block of the flags file: typed fields, rendered by the
//! app as plain text. No HTML, ever — the renderer draws each string as
//! text, and nothing in the block can name markup, a script or a style.
//!
//! ```json
//! { "flags": {
//!     "notification_center": true,
//!     "notices": {
//!       "format": 1,
//!       "donation_ask": 2,
//!       "items": [{
//!         "id": "source-2026-11",
//!         "title": "The streaming source is down for maintenance",
//!         "body": "It should be back tonight.",
//!         "link": "https://github.com/JoaoPucci/ani-gui/discussions/1",
//!         "priority": "warning",
//!         "about": "source",
//!         "min_app": "0.20.0",
//!         "max_app": "0.20.9"
//!       }]
//!     }
//! } }
//! ```
//!
//! **Versioned and forward-compatible.** `format` is the block's
//! version. A build reads only the format it knows, [`FORMAT`]; a
//! block of any other format is dropped whole, which is how a later
//! change that older builds must not misread is published. A change
//! older builds may safely ignore — a button whose action comes from a
//! fixed list, an image served from this repository — is published as
//! new fields under the same format: every field this build does not
//! know is ignored, so an older build shows the notice without it.
//! `min_app` and `max_app` aim a notice at a range of releases, so a
//! notice can also name the builds that understand it.
//!
//! **Strict.** Every known field is checked. A notice with a field that
//! is malformed — a link that is not plain `http`/`https`, text past
//! its limit or carrying control or direction-override characters, a
//! priority this build does not know — is dropped and the reason
//! logged; it is never shown in part. Pure: no I/O, no clock.

use serde::Serialize;
use serde_json::Value;

/// The key of the flags file's `flags` object the block sits under.
/// No flag may be declared with this name (a test holds that).
pub const NOTICES_KEY: &str = "notices";

/// The only block format this build reads.
pub const FORMAT: u64 = 1;

/// The most notices kept; later ones are dropped.
pub const MAX_ITEMS: usize = 8;
/// The longest id, in characters.
pub const MAX_ID_CHARS: usize = 64;
/// The longest title, in characters. One line.
pub const MAX_TITLE_CHARS: usize = 120;
/// The longest body, in characters. Line breaks allowed.
pub const MAX_BODY_CHARS: usize = 1000;
/// The longest link, in bytes of its serialised form.
pub const MAX_LINK_CHARS: usize = 300;

/// How loudly a notice is shown; the renderer maps it to a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// News: neutral.
    Info,
    /// Something to know before playing: amber.
    Warning,
    /// Something broken: oxblood.
    Critical,
}

/// One notice as the renderer receives it: every string checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteNotice {
    /// Stable id: lower-case ASCII letters, digits, `.`, `_` and `-`.
    pub id: String,
    /// One line of plain text.
    pub title: String,
    /// Plain text, line breaks allowed.
    pub body: Option<String>,
    /// An `http` or `https` address with no credentials.
    pub link: Option<String>,
    /// How loudly it is shown.
    pub priority: Priority,
    /// Whether it explains the streaming source not answering
    /// (`"about": "source"`), so the renderer shows it with that
    /// notice rather than as a row of its own.
    pub about_source: bool,
}

/// What the block passes on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RemoteNotices {
    /// The donation-ask number: an install asks once each time it
    /// sees it rise past the last one it acted on.
    pub donation_ask: Option<u32>,
    /// The notices, in the file's order.
    pub items: Vec<RemoteNotice>,
}

/// Something the block held that was not passed on, for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dropped {
    /// The whole block: not an object, or a format this build does
    /// not read.
    Block(&'static str),
    /// `donation_ask` was present and not a non-negative integer.
    DonationAsk,
    /// `items` was present and not a list.
    Items,
    /// One notice, by its place in the list.
    Item {
        /// Its index in `items`.
        index: usize,
        /// Why.
        reason: &'static str,
    },
}

/// The outcome of reading a block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// What is passed on.
    pub notices: RemoteNotices,
    /// What was not, and why.
    pub dropped: Vec<Dropped>,
}

/// Read the notices block `raw` for a build at `app_version`.
#[must_use]
pub fn parse(raw: Option<&Value>, app_version: &str) -> Parsed {
    let _ = (raw, app_version);
    Parsed::default()
}

#[cfg(test)]
#[path = "remote_test.rs"]
mod tests;

#[cfg(test)]
#[path = "remote_prop_test.rs"]
mod prop_tests;
