//! A replay over a standing positive availability row proves one
//! fact the row may not hold: the version that served carries the
//! mode it served. The row is otherwise left as it stands — its body
//! but for that one version's mode, and its window — so the merge
//! never stands in for the reprobe the row's expiry schedules.

use crate::app::AppState;
use crate::cache::{meta_cache_rebody, meta_cache_row};
use crate::commands::availability::AvailabilityResponse;
use crate::commands::play_native_versions::{VersionInfo, VersionKind};

/// How many times the merge re-reads a row another writer changed
/// under it. Every writer but an airing cut holds the row's lock while
/// the merge runs, so only a cut can land in between, and a cut
/// changes the window once.
const ATTEMPTS: usize = 3;

/// Merge into the row at `key` that the version `played` names in
/// `versions` carries the row's mode. A row that names no such
/// version, or already knows it carries the mode, is left byte for
/// byte; a mode the row knows is never taken back.
pub(crate) fn merge_served_mode(
    state: &AppState,
    key: &str,
    versions: &[VersionInfo],
    played: Option<VersionKind>,
) {
    let Some(served) = played.and_then(|kind| versions.iter().find(|v| v.kind == kind)) else {
        return;
    };
    for _ in 0..ATTEMPTS {
        let Ok(Some(row)) = meta_cache_row(&state.cache_pool, key) else {
            return;
        };
        let Some(body) = merged_body(&row.body, served) else {
            return;
        };
        match meta_cache_rebody(&state.cache_pool, key, &row, &body) {
            Ok(false) => continue,
            _ => return,
        }
    }
}

/// `body` with `served`'s version marked as carrying the mode, or
/// `None` when there is nothing to merge: a row that does not parse,
/// is not positive, does not list the version, or already says it
/// carries the mode.
fn merged_body(body: &str, served: &VersionInfo) -> Option<String> {
    let mut row: AvailabilityResponse = serde_json::from_str(body).ok()?;
    if !row.available {
        return None;
    }
    let version = row
        .versions
        .iter_mut()
        .find(|v| v.kind == served.kind && v.show_key == served.show_key)?;
    if version.has_mode == Some(true) {
        return None;
    }
    version.has_mode = Some(true);
    serde_json::to_string(&row).ok()
}

#[cfg(test)]
#[path = "availability_served_mode_test.rs"]
mod tests;
