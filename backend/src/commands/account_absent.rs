//! A list write to one tracker entry, read against the tracker no
//! longer having that entry. Shared by the editor's save and the
//! mark-watched write-back.

use crate::account::provider::{EntryUpdate, ListEntry, ProviderMediaId, Tokens, UserListProvider};
use crate::error::{AniError, Result};

/// Write `update` to `native`: `Some` with the written entry, or `None`
/// when the tracker no longer has the entry, so nothing was written.
///
/// The tracker answering the write itself with absence (404/410) is
/// that answer. A write it refuses as a bad request (any other 4xx), or
/// answers with no entry it can be read as (AniList can send a refused
/// mutation's `errors` with HTTP 200), can mean the same thing in other
/// words — AniList validates a mutation's `mediaId` — so it is confirmed
/// with one live read of the entry: the write answers `None` only when
/// that read answers absent. A read the tracker rate-limits, or refuses
/// the bearer for, reports that, since it says what to do next; any
/// other read keeps the write's own error. Every other failure of the
/// write (a rate limit, a revoked bearer, a 5xx, the network) says
/// nothing about the entry, is not confirmed, and comes back as it came.
///
/// # Errors
/// The write's error, unless the tracker answered that the entry is
/// absent — or the confirming read's, when the tracker rate-limited it
/// or refused the bearer.
pub(crate) async fn update_unless_absent(
    provider: &dyn UserListProvider,
    tokens: &Tokens,
    native: ProviderMediaId,
    update: EntryUpdate,
) -> Result<Option<ListEntry>> {
    let err = match AniError::unless_absent(provider.update_entry(tokens, native, update).await) {
        Err(err) if needs_confirming(&err) => err,
        written => return written,
    };
    match provider.current_entry(tokens, native).await {
        Err(read) if read.is_not_found_shaped() => Ok(None),
        Err(read @ (AniError::RateLimited { .. } | AniError::InvalidToken)) => Err(read),
        _ => Err(err),
    }
}

/// Whether a write's `err` leaves open that the entry is gone: the
/// tracker rejecting it (a 4xx other than absence), or
/// answering with no entry. A 401 and a 429 reach here as their own
/// variants, not as a status.
fn needs_confirming(err: &AniError) -> bool {
    match err {
        // The trackers' transports answer a 4xx as `TrackerRejected`;
        // an `Upstream` 4xx is read the same way, defensively.
        AniError::Upstream { status } => (400..500).contains(status),
        AniError::TrackerRejected { .. } | AniError::ParseFailed { .. } => true,
        _ => false,
    }
}

#[cfg(test)]
#[path = "account_absent_prop_test.rs"]
mod prop_tests;
