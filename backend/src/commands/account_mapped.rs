//! The mark-watched write-back read through the show's episode map:
//! the episode goes to the entry its segment names, under that entry's
//! number, completing it at its own last episode; the entries of the
//! segments before it are brought to their totals first. Where the map
//! is identity the write is the one [`super::account`] has always made.

use crate::account::provider::{EntryUpdate, ListEntry, ProviderMediaId};
use crate::error::Result;

use super::tracker_map::MapAsk;

/// Push `update` for Kitsu's `kitsu_id`, which Kitsu's mappings resolve
/// to `native` on the asking tracker (or to nothing), through the show's
/// map.
pub(crate) async fn push_progress_mapped(
    ask: &MapAsk<'_>,
    kitsu_id: &str,
    native: Option<ProviderMediaId>,
    update: EntryUpdate,
) -> Result<Option<ListEntry>> {
    let _ = kitsu_id;
    match native {
        Some(native) => {
            super::account::push_progress_via(
                ask.state,
                ask.kind,
                ask.provider,
                ask.tokens,
                native,
                update,
            )
            .await
        }
        None => Ok(None),
    }
}

#[cfg(test)]
#[path = "account_mapped_test.rs"]
mod tests;
