//! The last step of a mapped write: the episode's own number on its own
//! entry, against the row read under the entry's lock. Split from
//! `account_mapped.rs` so each file stays inside the CRAP gate's
//! per-file bar.

use super::*;

/// The episode's own number on its own entry, against the entry's row
/// `row` read under the entry's lock: completed where it is that
/// entry's last on a finished entry. The fan-out's own finale status is
/// not read: the map knows which entry the finale is the end of. A
/// score the update carries goes with the episode.
pub(super) async fn write_episode(
    ask: &MapAsk<'_>,
    write: &MappedWrite,
    update: &EntryUpdate,
    row: Option<CurrentEntry>,
    began: u64,
) -> Result<Option<ListEntry>> {
    // A write that needs the episodes before it on the entry is sent only
    // where the entry holds them.
    if write
        .needs
        .is_some_and(|needs| row.map_or(0, |r| r.progress_episodes) < needs)
    {
        return Ok(None);
    }
    let ours = EntryUpdate {
        status: write.complete.then_some(ListStatus::Completed),
        progress_episodes: Some(write.progress),
        score_0_to_100: update.score_0_to_100,
        repeat_count: None,
    };
    let Some(ours) = reconcile_for(ask.kind, ours, row) else {
        return Ok(None);
    };
    write_through(
        ask.state,
        ask.kind,
        ask.provider,
        ask.tokens,
        ProviderMediaId(write.entry),
        ours,
        began,
    )
    .await
}
