//! The resolution rows a removal takes: the ones that name a removed
//! show's key, and the ones resolved from a page the show was known by
//! — split from `history_forget` for the per-file complexity bar.

use std::collections::HashSet;

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_entries_prefix};
use crate::error::Result;

/// Delete the resolution rows, of any schema, whose value names one of
/// `ids` as the show played, or one of `known_by` — no empty id among
/// them — as the page it was resolved from.
///
/// Returns the other show keys the deleted rows named that no
/// surviving resolution row names. A key some row still names can be
/// played from the cache without resolving again, and that play writes
/// the key's history row through its numbering; a key nothing names
/// has its numbering stamped afresh by the resolve its next play needs.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn forget_resolutions(
    state: &AppState,
    ids: &[&str],
    known_by: &[String],
) -> Result<Vec<String>> {
    let mut named = Vec::new();
    let mut still_named = HashSet::new();
    for (key, body) in meta_cache_entries_prefix(&state.cache_pool, "play:")? {
        let row = serde_json::from_str::<serde_json::Value>(&body).unwrap_or_default();
        let field = |name: &str| row.get(name).and_then(|v| v.as_str()).unwrap_or_default();
        let (show, page) = (field("show_id"), field("kitsu_id"));
        let of_a_show = ids.contains(&show);
        if !of_a_show && !known_by.iter().any(|k| k == page) {
            still_named.insert(show.to_owned());
            continue;
        }
        meta_cache_delete(&state.cache_pool, &key)?;
        if !of_a_show && !show.is_empty() {
            named.push(show.to_owned());
        }
    }
    named.retain(|key| !still_named.contains(key));
    Ok(named)
}
