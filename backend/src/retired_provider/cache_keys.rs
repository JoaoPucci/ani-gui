//! Reading the cache's key families for the retired-provider sweep —
//! split from [`super::cache`] for the per-file complexity bar.

use rusqlite::params;

use crate::app::AppState;
use crate::error::{AniError, Result};
use crate::scraper::provider::ProviderId;

/// Every row under `prefix`, which ends in `:`, expired or not, with
/// the text of the JSON field at `path` when one is asked for: `None`
/// for a body that is not JSON or has no such field. Read as a range of
/// the key index.
pub(super) fn rows(
    state: &AppState,
    prefix: &str,
    path: Option<&str>,
) -> Result<Vec<(String, Option<String>)>> {
    let conn = state.cache_pool.get().map_err(|_| AniError::Cache)?;
    let mut stmt = conn
        .prepare(
            "SELECT key, CASE WHEN ?2 IS NOT NULL AND json_valid(body) \
                    THEN json_extract(body, ?2) END \
             FROM meta_cache WHERE key >= ?1 AND key < ?3",
        )
        .map_err(|_| AniError::Cache)?;
    let found = stmt
        .query_map(params![prefix, path, past(prefix)], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1).ok().flatten(),
            ))
        })
        .map_err(|_| AniError::Cache)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| AniError::Cache)?;
    Ok(found)
}

/// The first key past every key under `prefix`, which ends in `:`.
pub(super) fn past(prefix: &str) -> String {
    debug_assert!(prefix.ends_with(':'));
    format!("{};", &prefix[..prefix.len() - 1])
}

/// The show key a reverse-mapping row is for: under a version
/// (`allmanga2kitsu:v<N>:<key>`) or the mark a play leaves beside it
/// (`allmanga2kitsu:played:v1:<key>`).
pub(super) fn mapping_key_id(key: &str) -> Option<&str> {
    let rest = key.strip_prefix("allmanga2kitsu:")?;
    if let Some(id) = rest.strip_prefix("played:v1:") {
        return Some(id);
    }
    let (version, id) = rest.split_once(':')?;
    let digits = version.strip_prefix('v')?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

/// Whether a verdict's `provider` field names a provider the state
/// walks, read the way the verdict itself is deserialized.
pub(super) fn walked_provider(state: &AppState, provider: &str) -> bool {
    serde_json::from_value::<ProviderId>(serde_json::Value::String(provider.to_string()))
        .is_ok_and(|p| state.providers.order().contains(&p))
}

/// Whether a title match's segment is a walked provider's label.
pub(super) fn walked_label(state: &AppState, segment: &str) -> bool {
    state.providers.order().iter().any(|p| p.label() == segment)
}

#[cfg(test)]
#[path = "cache_keys_test.rs"]
mod tests;
