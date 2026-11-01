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
    CurrentEntry, EntryUpdate, ListEntry, ProviderKind, ProviderMediaId, Tokens, UserListProvider,
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
    // window and observe a value the write is about to change. A later explicit
    // Save would otherwise force-write that stale value over the just-synced
    // higher one.
    let _read_guard = state.account_write_locks.hold(kind, kitsu_id).await;
    let Some(native) = resolve_native_media_id(state, kind, kitsu_id, None).await? else {
        return Ok(None);
    };
    // And the entry's own lock, which writes reaching it from another
    // Kitsu entry (a part filed under it) take too.
    let _entry_guard = state.account_write_locks.hold_entry(kind, native).await;
    read_entry_at(state, kind, provider, tokens, native).await
}

/// The live read of `native`'s entry, with the show's locks held by the
/// caller.
pub(crate) async fn read_entry_at(
    state: &Arc<AppState>,
    kind: ProviderKind,
    provider: &dyn UserListProvider,
    tokens: &Tokens,
    native: ProviderMediaId,
) -> Result<Option<CurrentEntry>> {
    // Noted, so a removal's local clear acts only on what this read found.
    let revisions = &state.account_write_locks.cache_revisions;
    let read = revisions.start_read(kind);
    let entry = provider.current_entry(tokens, native).await?;
    revisions.note_read(kind, native.0, read, entry.is_none());
    Ok(entry)
}

/// Write an *explicit* user edit (status and/or progress) to a tracker
/// for a show — the detail-page list editor. Unlike
/// [`account::push_progress`], this does NOT run `reconcile_monotonic`:
/// the user's deliberate value is written verbatim, so they can correct
/// an over-count downward or move status anywhere.
///
/// `Ok(None)` when the show can't be mapped to the provider, or when the
/// edit carries nothing ([`EntryUpdate::is_empty`]) — a non-error
/// "nothing to write".
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
    // An edit with nothing in it is a no-op: no request, `None`.
    if update.is_empty() {
        return Ok(None);
    }
    // The connection the save arrived in, before its first wait: a
    // disconnect while it is out purges the cache, and its write-through
    // must not undo that.
    let began = state.account_write_locks.epochs.current(kind);
    // Per-show lock keyed on the Kitsu id, taken before resolve so an
    // explicit edit and a concurrent mark-watched write serialize across
    // their whole resolve→write.
    let _write_guard = state.account_write_locks.hold(kind, kitsu_id).await;
    let Some(native) = resolve_native_media_id(state, kind, kitsu_id, None).await? else {
        return Ok(None);
    };
    let _entry_guard = state.account_write_locks.hold_entry(kind, native).await;
    // Resolve the cache owner before the mutation: the write may spend
    // the last allowance, so a profile request afterwards can be refused.
    let owner = super::account_owner::before_write(provider, tokens).await?;
    let entry = provider.update_entry(tokens, native, update).await?;
    let owner = super::account_owner::after_write(provider, tokens, owner).await;
    // Force the explicit value into the local cache (overwrites a higher
    // progress, unlike the monotonic mark-watched write-through) so the
    // rail/editor reflect a downward correction immediately. Best-effort
    // — the authoritative tracker write already succeeded.
    super::account_owner::cache_row(state, kind, began, native.0, owner.as_deref(), |owner| {
        cache_upsert::upsert_entry_force(&state.cache_pool, kind, owner, &entry)
    });
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
    let began = state.account_write_locks.epochs.current(kind);
    // Per-show lock keyed on the Kitsu id, taken before resolve so the
    // delete serializes with a concurrent mark-watched write / editor read
    // across their whole resolve→write.
    let _write_guard = state.account_write_locks.hold(kind, kitsu_id).await;
    let Some(native) = resolve_native_media_id(state, kind, kitsu_id, None).await? else {
        return Ok(false);
    };
    let _entry_guard = state.account_write_locks.hold_entry(kind, native).await;
    // The title may already be gone upstream (double-click Remove, or
    // removed in another client). Providers signal that differently —
    // MAL returns Upstream{404}, AniList fails the row-id lookup with a
    // parse error — so don't match
    // on a status. Instead, on ANY delete error, confirm with a live
    // read: if the show is no longer on the list the delete's goal is
    // already met (idempotent success, still drop the cache row); a
    // still-present show means the failure is real and propagates.
    let owner = super::account_owner::before_write(provider, tokens).await?;
    if let Err(e) = provider.delete_entry(tokens, native).await {
        match provider.current_entry(tokens, native).await {
            Ok(None) => {}
            _ => return Err(e),
        }
    }
    let owner = super::account_owner::after_write(provider, tokens, owner).await;
    // Drop the cache row (best-effort) so the rail stops showing it.
    super::account_owner::cache_row(state, kind, began, native.0, owner.as_deref(), |owner| {
        cache_upsert::delete_entry_row(&state.cache_pool, kind, owner, native.0)
    });
    Ok(true)
}

/// Clear `kitsu_id`'s cached row from `user_id`'s list, asking the
/// tracker nothing: the removal's live read found the show absent there,
/// so only the local copy is stale. Only on the evidence of the row's
/// latest-sent live read, and only while no write has marked the row since
/// that read was sent and no refresh started after it has been cached; the
/// cleared row is marked as of the read, so a refresh that started before
/// it cannot put the row back. Nothing for an account disconnected since,
/// or for a show no tracker id maps.
pub async fn forget_cached_entry(
    state: &Arc<AppState>,
    kind: ProviderKind,
    kitsu_id: &str,
    user_id: &str,
) -> Result<()> {
    let began = state.account_write_locks.epochs.current(kind);
    let _write_guard = state.account_write_locks.hold(kind, kitsu_id).await;
    let Some(native) = resolve_native_media_id(state, kind, kitsu_id, None).await? else {
        return Ok(());
    };
    clear_cached_row(state, kind, began, native.0, user_id);
    Ok(())
}

/// Clear `media`'s cached row from `user_id`'s list, for a request that
/// arrived in connection epoch `began` — only if the row's latest-sent live
/// read found it absent and nothing asked after that read has answered for
/// the row (`account_revisions`).
pub(crate) fn clear_cached_row(
    state: &Arc<AppState>,
    kind: ProviderKind,
    began: u64,
    media: u32,
    user_id: &str,
) {
    let locks = &state.account_write_locks;
    let _ = locks.epochs.write_if_current(kind, began, || {
        locks
            .cache_revisions
            .clear_if_still_absent(kind, media, || {
                cache_upsert::delete_entry_row(&state.cache_pool, kind, user_id, media)
            })
    });
}

#[cfg(test)]
#[path = "account_edit_test.rs"]
mod tests;

#[cfg(test)]
#[path = "account_edit_rate_limit_test.rs"]
mod rate_limit_tests;
