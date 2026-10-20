//! Display stamps an earlier version wrote with a row's former tag —
//! the tag a stitched listing gave a row before it was numbered as it
//! is now — and putting them right. Split from [`super`] for the
//! per-file complexity bar.

use super::numbering_offset_store::{rewrite, Row};
use super::{display_stamp, AppState};

#[path = "numbering_offset_former_read.rs"]
mod read;
pub use read::{former_stamp, stamp_beside};

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
    let Some((stamped, tag)) = display_stamp(state, slug) else {
        return;
    };
    if former_tags
        .iter()
        .any(|(slot, former, _)| *slot == stamped && *former == tag)
    {
        retag_row(state, slug, former_tags, shared);
    }
}

/// Put the slug's display stamp right: where it names one of
/// `former_tags`' slots by the tag an earlier version gave that row,
/// it names the row's tag now, and remembers the one it replaced.
/// Decided under the store's locks, against the row as it stands
/// there; any other row, or a slug with none, is left alone. A
/// replaced tag a row of the listing carries now (`shared`) is not
/// remembered: what is kept under it may be that row's.
fn retag_row(
    state: &AppState,
    slug: &str,
    former_tags: &[(u32, String, String)],
    shared: &dyn Fn(&str) -> bool,
) {
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
            row.former = (!shared(former)).then(|| (*slot, former.clone()));
        }
    };
    if let Err(e) = rewrite(state, change) {
        tracing::warn!(slug, error = ?e, "numbering stamp repair failed");
    }
}

/// [`retag_display`] from a resolve — a play's, a download's — for the
/// show it resolved.
pub fn retag_from_resolve(
    state: &AppState,
    native: &crate::commands::play_native_resolve::NativeResolved,
) {
    retag_display(state, &native.slug, &native.former_tags, &|former| {
        crate::commands::play_native_merge::names_a_row_now(former, native)
    });
    // The pick's other versions: each copy's history row reads a stamp
    // of its own, put right from that copy's listing.
    for (slug, episodes) in &native.other_copies {
        retag_from_listing(state, slug, episodes);
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

#[cfg(test)]
#[path = "numbering_offset_former_test.rs"]
mod tests;
