//! The resolution rows a removal takes: the ones that name a removed
//! show's key, and the ones resolved from a page the show was known by
//! — split from `history_forget` for the per-file complexity bar.
//!
//! Finding the rows and deleting them are two steps, so a removal can
//! drop the numbering of the other keys they name in between: a retry
//! after a failure finds those keys only through the rows, so the rows
//! go last.

use std::collections::HashSet;

use crate::app::AppState;
use crate::cache::{meta_cache_delete, meta_cache_entries_prefix};
use crate::error::Result;

/// The resolution rows a removal takes, and the show keys they name.
pub(crate) struct Resolutions {
    /// The rows' cache keys.
    rows: Vec<String>,
    /// The other show keys the rows name: keys a page of the show
    /// resolved under, another provider's when its walk failed over.
    pub(crate) named: Vec<String>,
    /// The show keys a resolution row the removal does not take names.
    still_named: HashSet<String>,
}

/// Find the resolution rows, of any schema, whose value names one of
/// `ids` as the show played, or one of `known_by` — no empty id among
/// them — as the page it was resolved from.
///
/// # Errors
/// Cache failures propagate.
pub(crate) fn find_resolutions(
    state: &AppState,
    ids: &[&str],
    known_by: &[String],
) -> Result<Resolutions> {
    let mut found = Resolutions {
        rows: Vec::new(),
        named: Vec::new(),
        still_named: HashSet::new(),
    };
    for (key, body) in meta_cache_entries_prefix(&state.cache_pool, "play:")? {
        let row = serde_json::from_str::<serde_json::Value>(&body).unwrap_or_default();
        let field = |name: &str| row.get(name).and_then(|v| v.as_str()).unwrap_or_default();
        let (show, page) = (field("show_id"), field("kitsu_id"));
        let of_a_show = ids.contains(&show);
        if !of_a_show && !known_by.iter().any(|k| k == page) {
            found.still_named.insert(show.to_owned());
            continue;
        }
        found.rows.push(key);
        if !of_a_show && !show.is_empty() {
            found.named.push(show.to_owned());
        }
    }
    Ok(found)
}

impl Resolutions {
    /// The other keys only the found rows name: the ones a delete takes
    /// the numbering of. A key a resolution row the delete leaves still
    /// names keeps it, whether or not that row can be played again: a
    /// live row's play writes the key's history row through it, and one
    /// expired or under an older schema's key keeps it until the
    /// history is cleared.
    pub(crate) fn only_named(&self) -> Vec<String> {
        self.named
            .iter()
            .filter(|key| !self.still_named.contains(*key))
            .cloned()
            .collect()
    }

    /// Delete the found rows.
    ///
    /// # Errors
    /// Cache failures propagate.
    pub(crate) fn forget(&self, state: &AppState) -> Result<()> {
        for key in &self.rows {
            meta_cache_delete(&state.cache_pool, key)?;
        }
        Ok(())
    }
}
