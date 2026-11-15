//! A list write to one tracker entry, read against the tracker no
//! longer having that entry. Shared by the editor's save and the
//! mark-watched write-back.

use crate::account::provider::{EntryUpdate, ListEntry, ProviderMediaId, Tokens, UserListProvider};
use crate::error::{AniError, Result};

/// Write `update` to `native`: `Some` with the written entry, or `None`
/// when the tracker no longer has the entry, so nothing was written.
///
/// The tracker answering the write itself with absence (404/410) is
/// that answer. A write it refuses as a bad request (any other 4xx) can
/// mean the same thing in other words — AniList validates a mutation's
/// `mediaId` — so it is confirmed with one live read of the entry: the
/// write answers `None` only when that read answers absent, and keeps
/// its own error otherwise. Every other failure (a rate limit, a revoked
/// bearer, a 5xx, the network) says nothing about the entry, is not
/// confirmed, and comes back as it came.
///
/// # Errors
/// The write's error, unless the tracker answered that the entry is
/// absent.
pub(crate) async fn update_unless_absent(
    provider: &dyn UserListProvider,
    tokens: &Tokens,
    native: ProviderMediaId,
    update: EntryUpdate,
) -> Result<Option<ListEntry>> {
    let err = match AniError::unless_absent(provider.update_entry(tokens, native, update).await) {
        Err(err) if refused_as_bad_request(&err) => err,
        written => return written,
    };
    match provider.current_entry(tokens, native).await {
        Err(read) if read.is_not_found_shaped() => Ok(None),
        _ => Err(err),
    }
}

/// Whether `err` is the tracker refusing the request as a bad one: a
/// 4xx other than absence. A 401 and a 429 reach here as their own
/// variants, not as a status.
fn refused_as_bad_request(err: &AniError) -> bool {
    matches!(err, AniError::Upstream { status } if (400..500).contains(status))
}

#[cfg(test)]
#[path = "account_absent_prop_test.rs"]
mod prop_tests;
