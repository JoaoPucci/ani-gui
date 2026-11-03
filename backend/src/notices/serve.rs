//! What the window is handed of the published notices.
//!
//! The notification center is the first feature a signed flags file
//! turns on: it ships off as a rollout ([`NOTIFICATION_CENTER`]), and
//! only a verified file may set it `true` (see [`crate::flags::decl`]).
//! While it is off the window is served nothing; while it is on, the
//! notices block of the document that decided the launch's flags,
//! parsed for this release. That block is only ever there when the
//! file verified (see [`crate::flags::signature`]).

use serde_json::Value;

use super::remote::{parse, Dropped, RemoteNotices};
use crate::flags::decl::{FlagDecl, FlagKind};
use crate::flags::Resolved;

/// The notification center's flag.
pub const NOTIFICATION_CENTER: FlagDecl = FlagDecl {
    name: "notification_center",
    kind: FlagKind::Rollout,
    safe_because: "off is this build's own behaviour, the app as it was before the center; \
                   on shows a bell, a list and toasts of notices, and only a signed file turns it on",
};

/// The notices the window is served under `flags`, from the deciding
/// document's block `raw`, for a build at `app_version`. What the
/// parser drops is logged, never served.
#[must_use]
pub fn served(flags: &Resolved, raw: Option<&Value>, app_version: &str) -> RemoteNotices {
    if !flags.enabled(&NOTIFICATION_CENTER) {
        return RemoteNotices::default();
    }
    let parsed = parse(raw, app_version);
    for dropped in &parsed.dropped {
        log_dropped(dropped);
    }
    parsed.notices
}

fn log_dropped(dropped: &Dropped) {
    tracing::warn!(?dropped, "a published notice was not used");
}
