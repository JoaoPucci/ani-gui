//! The mark-watched write-back read through the show's episode map:
//! the episode goes to the entry its segment names, under that entry's
//! number, completing it at its own last episode; the entries of the
//! segments before it are brought to their totals first. Where the map
//! is identity the write is the one [`super::account`] has always made.

use crate::account::episode_map::{EarlierSegment, MappedWrite, Target};
use crate::account::provider::{CurrentEntry, EntryUpdate, ListEntry, ProviderMediaId};
use crate::account::status::ListStatus;
use crate::error::Result;

use super::account::{push_progress_read, push_progress_via, reconcile_for, write_through};
use super::tracker_map::{episode_map_for, episode_map_for_unresolved, MapAsk};

/// Push `update` for Kitsu's `kitsu_id`, which Kitsu's mappings resolve
/// to `native` on the asking tracker (or to nothing), through the show's
/// map.
///
/// A write without progress — a status or a score alone — goes to the
/// resolved entry as it always has. A watched episode is read through
/// the map: where no segment covers it the write is the identity one,
/// fitted to the resolved entry's total; where one does it goes to that
/// segment's entry ([`write_mapped`]). A show Kitsu resolves to nothing
/// writes only where the map files it under another entry.
pub(crate) async fn push_progress_mapped(
    ask: &MapAsk<'_>,
    kitsu_id: &str,
    native: Option<ProviderMediaId>,
    update: EntryUpdate,
) -> Result<Option<ListEntry>> {
    let Some(episode) = update.progress_episodes else {
        return match native {
            Some(native) => {
                push_progress_via(
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
        };
    };
    let Some(native) = native else {
        return match episode_map_for_unresolved(ask, kitsu_id)
            .await?
            .target(episode)
        {
            Target::Mapped(write) => write_mapped(ask, &write, &update).await,
            Target::Identity => Ok(None),
        };
    };
    // The resolved entry's read — whose total the map is checked against
    // — and an identity write on it are one read-then-write, under the
    // entry's lock.
    let guard = hold_entry(ask, native.0).await;
    let media = ask.provider.tracked_media(ask.tokens, native).await?;
    match episode_map_for(ask, kitsu_id, native, media.episodes)
        .await?
        .target(episode)
    {
        Target::Identity => {
            push_progress_read(
                ask.state,
                ask.kind,
                ask.provider,
                ask.tokens,
                native,
                update,
                media,
            )
            .await
        }
        // The resolved entry is a map's first segment, so a write to it
        // starts no later segment: it is written on the read just made.
        Target::Mapped(write) if write.entry == native.0 && write.earlier.is_empty() => {
            write_episode(ask, &write, &update, media.entry).await
        }
        Target::Mapped(write) => {
            drop(guard);
            write_mapped(ask, &write, &update).await
        }
    }
}

/// Wait for, and hold, the lock guarding read-then-write on tracker
/// entry `entry`.
async fn hold_entry(ask: &MapAsk<'_>, entry: u32) -> tokio::sync::OwnedMutexGuard<()> {
    ask.state
        .account_write_locks
        .hold_entry(ask.kind, ProviderMediaId(entry))
        .await
}

/// The user's row on tracker entry `entry`.
async fn row_on(ask: &MapAsk<'_>, entry: u32) -> Result<Option<CurrentEntry>> {
    Ok(ask
        .provider
        .tracked_media(ask.tokens, ProviderMediaId(entry))
        .await?
        .entry)
}

/// Bring an earlier segment's entry to its total — completed where the
/// tracker has it finished — unless the row is already there, reading
/// and writing it under its own lock. The reconcile ([`reconcile_for`])
/// keeps it from lowering a row and keeps an AniList rewatch; a MAL one
/// is a Watching row there, so it is closed — back at Completed, its
/// rewatch counted.
async fn bring_to_total(ask: &MapAsk<'_>, earlier: &EarlierSegment) -> Result<()> {
    let _guard = hold_entry(ask, earlier.entry).await;
    let row = row_on(ask, earlier.entry).await?;
    let done = row.map(|r| r.as_kept_by(ask.kind)).is_some_and(|r| {
        r.progress_episodes >= earlier.total
            && matches!(r.status, ListStatus::Completed | ListStatus::Rewatching)
    });
    if done {
        return Ok(());
    }
    let update = EntryUpdate {
        status: earlier.finished.then_some(ListStatus::Completed),
        progress_episodes: Some(earlier.total),
        ..Default::default()
    };
    if let Some(update) = reconcile_for(ask.kind, update, row) {
        write_through(
            ask.state,
            ask.kind,
            ask.provider,
            ask.tokens,
            ProviderMediaId(earlier.entry),
            update,
        )
        .await?;
    }
    Ok(())
}

/// Write a mapped episode: the earlier segments' entries brought to
/// their totals first when this write starts its segment (its row is
/// absent or at 0), then the episode's own number on its own entry.
/// Each entry is read and written under its own lock, one at a time:
/// the episode's entry is read, released while the earlier entries are
/// brought up, and read again before its write.
async fn write_mapped(
    ask: &MapAsk<'_>,
    write: &MappedWrite,
    update: &EntryUpdate,
) -> Result<Option<ListEntry>> {
    let mut guard = hold_entry(ask, write.entry).await;
    let mut row = row_on(ask, write.entry).await?;
    let starts_segment = row.is_none_or(|r| r.progress_episodes == 0);
    if starts_segment && !write.earlier.is_empty() {
        drop(guard);
        for earlier in &write.earlier {
            bring_to_total(ask, earlier).await?;
        }
        guard = hold_entry(ask, write.entry).await;
        row = row_on(ask, write.entry).await?;
    }
    let written = write_episode(ask, write, update, row).await;
    drop(guard);
    written
}

/// The episode's own number on its own entry, against the entry's row
/// `row` read under the entry's lock: completed where it is that
/// entry's last on a finished entry. The fan-out's own finale status is
/// not read: the map knows which entry the finale is the end of. A
/// score the update carries goes with the episode.
async fn write_episode(
    ask: &MapAsk<'_>,
    write: &MappedWrite,
    update: &EntryUpdate,
    row: Option<CurrentEntry>,
) -> Result<Option<ListEntry>> {
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
    )
    .await
}

#[cfg(test)]
#[path = "account_mapped_test.rs"]
mod tests;

#[cfg(test)]
#[path = "account_mapped_wire_test.rs"]
mod wire_tests;
