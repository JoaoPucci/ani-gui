//! Which title-match rows the retired-provider sweep removes — split
//! from [`super::cache_families`] for the per-file complexity bar.

use crate::app::AppState;
use crate::commands::kitsu::{LEGACY_TITLE_MATCH_SEGMENT, TITLE_MATCH_VERSION};
use crate::error::Result;
use crate::history::HistoryEntry;

use super::cache_keys::{rows, walked_label};
use super::evidence::is_cour;

/// Title matches under a segment that is neither a walked provider's
/// label nor the legacy one, unless a remaining row is read by them: a
/// row the app knows the provider of but no longer walks still files
/// its matches under that provider's label, and a row whose recorded
/// entry is gone is found through the retired segments its title was
/// filed under.
pub(super) fn title_matches(state: &AppState, remaining: &[HistoryEntry]) -> Result<Vec<String>> {
    let family = format!("title-match:v{TITLE_MATCH_VERSION}:");
    // Every prefix a remaining row's matches are read under: the
    // removal's own list, every version and retired segment included.
    let searched: Vec<String> = remaining
        .iter()
        .flat_map(|e| crate::commands::history_forget_titles::title_match_prefixes(&e.id, &e.title))
        .collect();
    Ok(rows(state, &family, None)?
        .into_iter()
        .map(|(key, _)| key)
        .filter(|key| {
            let segment = key[family.len()..].split(':').next().unwrap_or_default();
            segment != LEGACY_TITLE_MATCH_SEGMENT
                && !walked_label(state, segment)
                && !searched
                    .iter()
                    .any(|p| key.strip_prefix(p.as_str()).is_some_and(is_cour))
        })
        .collect())
}
