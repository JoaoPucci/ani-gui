//! Display stamps an earlier version wrote with a row's former tag —
//! the tag a stitched listing gave a row before it was numbered as it
//! is now — and putting them right. Split from [`super`] for the
//! per-file complexity bar.

use super::numbering_offset_store::{parse, rewrite, store_path, Row};
use super::{display_stamp, AppState};

/// Put a display stamp an earlier version wrote right: when the slug's
/// stamp names one of `former_tags`' slots with the tag that version
/// gave it, it names the tag the row has now. Any other stamp is left
/// alone, and a show whose stamp names no such row is not written.
///
/// `shared` says whether a former tag is one a row of the listing
/// carries now.
pub fn retag_display(
    state: &AppState,
    slug: &str,
    former_tags: &[(u32, String, String)],
    shared: &dyn Fn(&str) -> bool,
) {
    let _ = shared;
    let Some((stamped, tag)) = display_stamp(state, slug) else {
        return;
    };
    if former_tags
        .iter()
        .any(|(slot, former, _)| *slot == stamped && *former == tag)
    {
        retag_row(state, slug, former_tags);
    }
}

/// Put the slug's display stamp right: where it names one of
/// `former_tags`' slots by the tag an earlier version gave that row,
/// it names the row's tag now, and remembers the one it replaced.
/// Decided under the store's locks, against the row as it stands
/// there; any other row, or a slug with none, is left alone.
fn retag_row(state: &AppState, slug: &str, former_tags: &[(u32, String, String)]) {
    let change = |rows: &mut Vec<Row>| {
        let Some(row) = rows.iter_mut().find(|r| r.slug == slug) else {
            return;
        };
        let Some((stamped, tag)) = row.display.clone() else {
            return;
        };
        if let Some((slot, former, now)) = former_tags
            .iter()
            .find(|(slot, former, _)| *slot == stamped && *former == tag)
        {
            row.display = Some((*slot, now.clone()));
            row.former = Some((*slot, former.clone()));
        }
    };
    if let Err(e) = rewrite(state, change) {
        tracing::warn!(slug, error = ?e, "numbering stamp repair failed");
    }
}

/// [`retag_display`] from a listing at hand — a probe's, a
/// download's — for the show `slug` keys.
pub fn retag_from_listing(
    state: &AppState,
    slug: &str,
    episodes: &[crate::scraper::provider::EpisodeRef],
) {
    let shared = |former: &str| {
        episodes.iter().any(|e| {
            e.number2
                .as_deref()
                .is_some_and(|t| crate::commands::play_native_episode::tag_matches(t, former))
        })
    };
    retag_display(
        state,
        slug,
        &crate::commands::play_native_merge::former_tags_of(episodes),
        &shared,
    );
}

/// The (slot, tag) a repaired display stamp named its row by before
/// [`retag_display`] put it right.
pub fn former_stamp(state: &AppState, slug: &str) -> Option<(u32, String)> {
    former_of(&row_of(state, slug)?)
}

/// The display stamp the sidecar beside the history file at
/// `history_path` keeps for the slug — [`super::display_stamp`] for a
/// caller that holds the path rather than the state.
pub fn stamp_beside(history_path: &std::path::Path, slug: &str) -> Option<(u32, String)> {
    let body = std::fs::read_to_string(store_path(history_path)).unwrap_or_default();
    parse(&body).into_iter().find(|r| r.slug == slug)?.display
}

fn row_of(state: &AppState, slug: &str) -> Option<Row> {
    let body = std::fs::read_to_string(store_path(&state.history_path)).unwrap_or_default();
    parse(&body).into_iter().find(|r| r.slug == slug)
}

/// The row's former pair, while its stamp still names that slot.
fn former_of(row: &Row) -> Option<(u32, String)> {
    let (slot, _) = row.display.as_ref()?;
    row.former.clone().filter(|(was, _)| was == slot)
}

#[cfg(test)]
#[path = "numbering_offset_former_test.rs"]
mod tests;
