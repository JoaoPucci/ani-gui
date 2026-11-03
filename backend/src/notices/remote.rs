//! The notices block of the flags file: typed fields, rendered by the
//! app as plain text. No HTML, ever — the renderer draws each string as
//! text, and nothing in the block can name markup, a script or a style.
//!
//! ```json
//! { "flags": {
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
//! **Strict.** Every known field is checked; an `about` other than the
//! string `"source"` is the one ignored rather than refused. A notice
//! with a field that is malformed — a link that is not plain `http`/`https`, text past
//! its limit or carrying control or direction-override characters, a
//! priority this build does not know — is dropped, with its reason in
//! [`Parsed::dropped`]; it is never shown in part. Pure: no I/O, no
//! clock. The block is kept unparsed with the launch's flags
//! (`FlagStore::notices`), only from a verified file, and parsed when
//! the window asks for it (see [`super::serve`]).

use serde::Serialize;
use serde_json::{Map, Value};

use super::fields::{in_range, link, priority};
use super::text_fields::{id, text};

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
/// The longest link, in bytes of its serialised form (the `_CHARS`
/// names the limit's family; the count is bytes).
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
    let Some(raw) = raw else {
        return Parsed::default();
    };
    let Some(block) = raw.as_object() else {
        return dropped_block("the notices block is not an object");
    };
    if block.get("format").and_then(Value::as_u64) != Some(FORMAT) {
        return dropped_block("the notices block's format is not one this build reads");
    }
    let mut parsed = Parsed::default();
    parsed.notices.donation_ask = donation_ask(block, &mut parsed.dropped);
    match block.get("items") {
        None => {}
        Some(Value::Array(items)) => read_items(items, app_version, &mut parsed),
        Some(_) => parsed.dropped.push(Dropped::Items),
    }
    parsed
}

fn dropped_block(reason: &'static str) -> Parsed {
    Parsed {
        notices: RemoteNotices::default(),
        dropped: vec![Dropped::Block(reason)],
    }
}

fn donation_ask(block: &Map<String, Value>, dropped: &mut Vec<Dropped>) -> Option<u32> {
    let raw = block.get("donation_ask")?;
    let n = raw.as_u64().and_then(|n| u32::try_from(n).ok());
    if n.is_none() {
        dropped.push(Dropped::DonationAsk);
    }
    n
}

fn read_items(items: &[Value], app_version: &str, parsed: &mut Parsed) {
    for (index, raw) in items.iter().enumerate() {
        let outcome = if parsed.notices.items.len() >= MAX_ITEMS {
            Err("past the number of notices kept")
        } else {
            item(raw, app_version).and_then(|n| match n {
                Some(n) if parsed.notices.items.iter().any(|m| m.id == n.id) => {
                    Err("its id repeats an earlier notice's")
                }
                other => Ok(other),
            })
        };
        match outcome {
            Ok(Some(n)) => parsed.notices.items.push(n),
            Ok(None) => {}
            Err(reason) => parsed.dropped.push(Dropped::Item { index, reason }),
        }
    }
}

/// One notice: `Ok(None)` when it is well formed and aimed at other
/// builds, `Err` with the reason when it is malformed.
fn item(raw: &Value, app_version: &str) -> Result<Option<RemoteNotice>, &'static str> {
    let obj = raw.as_object().ok_or("a notice is not an object")?;
    let id = id(obj.get("id"))?;
    let title = text(obj.get("title"), MAX_TITLE_CHARS, false)?.ok_or("a notice has no title")?;
    let body = text(obj.get("body"), MAX_BODY_CHARS, true)?;
    let link = link(obj.get("link"))?;
    let priority = priority(obj.get("priority"))?;
    let about_source = obj.get("about").and_then(Value::as_str) == Some("source");
    let in_range = in_range(obj.get("min_app"), obj.get("max_app"), app_version)?;
    Ok(in_range.then_some(RemoteNotice {
        id,
        title,
        body,
        link,
        priority,
        about_source,
    }))
}

#[cfg(test)]
#[path = "remote_test.rs"]
mod tests;

#[cfg(test)]
#[path = "remote_prop_test.rs"]
mod prop_tests;
