//! Explicit list-entry edits driven by the detail-page editor.
//!
//! Split out of [`crate::commands::account`] to keep that file's CRAP
//! under the ratchet ceiling. Unlike the automatic mark-watched fan-out
//! ([`account::push_progress`], which is monotonic), these write the
//! user's *deliberate* value verbatim: the editor opens on the live
//! tracker state (via [`get_entry`]) and the user can move status
//! anywhere or correct an over-count downward (via [`set_entry`]).
//! Deviation safety comes from reading the live entry before editing,
//! not from clamping the write.
//!
//! Each public command builds the provider via
//! [`account::provider_for_kind`] then delegates to a `*_via` helper
//! that takes the provider injected — so tests drive the real
//! `update_entry`/`current_entry`/`delete_entry` round-trips against a
//! wiremock-backed provider (the provider's own HTTP parsing is covered
//! in `meta::{anilist,mal}_user`).

use std::sync::Arc;

use crate::account::cache_upsert;
use crate::account::provider::{
    CurrentEntry, EntryUpdate, ListEntry, ProviderKind, Tokens, UserListProvider,
};
use crate::app::AppState;
use crate::commands::account::{provider_for_kind, resolve_native_media_id};
use crate::error::{AniError, Result};

/// Read the user's current list entry for a show (status + watched
/// count), or `Ok(None)` when the show isn't mapped to the provider or
/// isn't on the user's list. Drives the detail-page editor so it opens
/// showing the *live* tracker state — the user never edits from a stale
/// local snapshot.
pub async fn get_entry(
    state: &Arc<AppState>,
    kind: ProviderKind,
    tokens: &Tokens,
    kitsu_id: &str,
) -> Result<Option<CurrentEntry>> {
    let Some(provider) = provider_for_kind(state, kind) else {
        return Err(AniError::Metadata);
    };
    get_entry_via(state, kind, provider.as_ref(), tokens, kitsu_id).await
}

async fn get_entry_via(
    state: &Arc<AppState>,
    kind: ProviderKind,
    provider: &dyn UserListProvider,
    tokens: &Tokens,
    kitsu_id: &str,
) -> Result<Option<CurrentEntry>> {
    // Take the per-show lock keyed on the Kitsu id BEFORE resolving the
    // native id, so the seed read serializes with a mark-watched write
    // (which takes the same lock before its own resolve) across the entire
    // resolve→read — the read can't slip in during the write's resolve
    // window and observe a value the write is about to change (Codex P2
    // #3427461062 / #3428252253). A later explicit Save would otherwise
    // force-write that stale value over the just-synced higher one.
    let show_lock = state.account_write_locks.for_show(kind, kitsu_id);
    let _read_guard = show_lock.lock().await;
    let Some(native) = resolve_native_media_id(state, kind, kitsu_id, None).await? else {
        return Ok(None);
    };
    provider.current_entry(tokens, native).await
}

/// Write an *explicit* user edit (status and/or progress) to a tracker
/// for a show — the detail-page list editor. Unlike
/// [`account::push_progress`], this does NOT run `reconcile_monotonic`:
/// the user's deliberate value is written verbatim, so they can correct
/// an over-count downward or move status anywhere.
///
/// `Ok(None)` when the show can't be mapped to the provider (a non-error
/// "nothing to write").
pub async fn set_entry(
    state: &Arc<AppState>,
    kind: ProviderKind,
    tokens: &Tokens,
    kitsu_id: &str,
    update: EntryUpdate,
) -> Result<Option<ListEntry>> {
    let Some(provider) = provider_for_kind(state, kind) else {
        return Err(AniError::Metadata);
    };
    set_entry_via(state, kind, provider.as_ref(), tokens, kitsu_id, update).await
}

async fn set_entry_via(
    state: &Arc<AppState>,
    kind: ProviderKind,
    provider: &dyn UserListProvider,
    tokens: &Tokens,
    kitsu_id: &str,
    update: EntryUpdate,
) -> Result<Option<ListEntry>> {
    // Per-show lock keyed on the Kitsu id, taken before resolve so an
    // explicit edit and a concurrent mark-watched write serialize across
    // their whole resolve→write (Codex P2 #3428252253).
    let show_lock = state.account_write_locks.for_show(kind, kitsu_id);
    let _write_guard = show_lock.lock().await;
    let Some(native) = resolve_native_media_id(state, kind, kitsu_id, None).await? else {
        return Ok(None);
    };
    let entry = provider.update_entry(tokens, native, update).await?;
    // Force the explicit value into the local cache (overwrites a higher
    // progress, unlike the monotonic mark-watched write-through) so the
    // rail/editor reflect a downward correction immediately. Best-effort
    // — the authoritative tracker write already succeeded.
    if let Ok(profile) = provider.me(tokens).await {
        let _ = cache_upsert::upsert_entry_force(&state.cache_pool, kind, &profile.user_id, &entry);
    }
    Ok(Some(entry))
}

/// Remove a show from the user's tracker list (the editor's "Remove from
/// list"). Deletes the provider entry, then drops the local cache row so
/// the rail/editor reflect the removal immediately. Returns `Ok(false)`
/// when the show isn't mapped to the provider (nothing to remove).
pub async fn remove_entry(
    state: &Arc<AppState>,
    kind: ProviderKind,
    tokens: &Tokens,
    kitsu_id: &str,
) -> Result<bool> {
    let Some(provider) = provider_for_kind(state, kind) else {
        return Err(AniError::Metadata);
    };
    remove_entry_via(state, kind, provider.as_ref(), tokens, kitsu_id).await
}

async fn remove_entry_via(
    state: &Arc<AppState>,
    kind: ProviderKind,
    provider: &dyn UserListProvider,
    tokens: &Tokens,
    kitsu_id: &str,
) -> Result<bool> {
    // Per-show lock keyed on the Kitsu id, taken before resolve so the
    // delete serializes with a concurrent mark-watched write / editor read
    // across their whole resolve→write (Codex P2 #3428252253).
    let show_lock = state.account_write_locks.for_show(kind, kitsu_id);
    let _write_guard = show_lock.lock().await;
    let Some(native) = resolve_native_media_id(state, kind, kitsu_id, None).await? else {
        return Ok(false);
    };
    // The title may already be gone upstream (double-click Remove, or
    // removed in another client). Providers signal that differently —
    // MAL returns Upstream{404}, AniList fails the row-id lookup with a
    // parse error (Codex P2 #3423108945 / #3423227862) — so don't match
    // on a status. Instead, on ANY delete error, confirm with a live
    // read: if the show is no longer on the list the delete's goal is
    // already met (idempotent success, still drop the cache row); a
    // still-present show means the failure is real and propagates.
    if let Err(e) = provider.delete_entry(tokens, native).await {
        match provider.current_entry(tokens, native).await {
            Ok(None) => {}
            _ => return Err(e),
        }
    }
    // Drop the cache row (best-effort) so the rail stops showing it.
    if let Ok(profile) = provider.me(tokens).await {
        let _ = cache_upsert::delete_entry_row(&state.cache_pool, kind, &profile.user_id, native.0);
    }
    Ok(true)
}

#[cfg(test)]
#[path = "account_edit_test.rs"]
mod tests;
