//! Dropping a former tag a stamp repair kept, once a listing at hand
//! carries it. Split from [`super`] for the per-file complexity bar.

use super::super::numbering_offset_store::{rewrite, Row};
use super::{former_stamp, AppState};

/// The former tag a stamp keeps beside it, once a listing at hand is
/// read: none where a row of the listing carries it now (`shared`),
/// since a position or a file kept under it may be that row's.
#[must_use]
pub(super) fn kept_former(
    former: Option<(u32, String)>,
    shared: &dyn Fn(&str) -> bool,
) -> Option<(u32, String)> {
    former.filter(|(_, tag)| !shared(tag))
}

/// Drop the slug's former tag where [`kept_former`] keeps none, decided
/// under the store's locks against the row as it stands there; a slug
/// whose former tag is kept, or that has none, is not written.
pub(super) fn drop_shared_former(state: &AppState, slug: &str, shared: &dyn Fn(&str) -> bool) {
    let dropped = |former: &Option<(u32, String)>| {
        former.is_some() && kept_former(former.clone(), shared).is_none()
    };
    if !dropped(&former_stamp(state, slug)) {
        return;
    }
    let change = |rows: &mut Vec<Row>| {
        if let Some(row) = rows.iter_mut().find(|r| r.slug == slug) {
            row.former = kept_former(row.former.take(), shared);
        }
    };
    if let Err(e) = rewrite(state, change) {
        tracing::warn!(slug, error = ?e, "numbering former tag drop failed");
    }
}
