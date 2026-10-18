//! The mark-watched write-back read through the show's episode map:
//! the episode goes to the entry its segment names, under that entry's
//! number, completing it at its own last episode; the entries of the
//! segments before it are brought to their totals first. Where the map
//! is identity the write is the one [`super::account`] has always made.

use crate::account::episode_map::{EarlierSegment, MappedWrite, Target};
use crate::account::provider::{EntryUpdate, ListEntry, ProviderMediaId, TrackedMedia};
use crate::account::status::ListStatus;
use crate::error::Result;

use super::account::{push_progress_read, push_progress_via, reconcile_monotonic, write_through};
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
            .await
            .target(episode)
        {
            Target::Mapped(write) => write_mapped(ask, &write, &update, None).await,
            Target::Identity => Ok(None),
        };
    };
    let media = ask.provider.tracked_media(ask.tokens, native).await?;
    match episode_map_for(ask, kitsu_id, native, media.episodes)
        .await
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
        Target::Mapped(write) => write_mapped(ask, &write, &update, Some((native, media))).await,
    }
}

/// The user's row on `entry`, from the read already made when it is the
/// resolved one.
async fn row_on(
    ask: &MapAsk<'_>,
    entry: u32,
    read: Option<(ProviderMediaId, TrackedMedia)>,
) -> Result<TrackedMedia> {
    match read {
        Some((native, media)) if native.0 == entry => Ok(media),
        _ => {
            ask.provider
                .tracked_media(ask.tokens, ProviderMediaId(entry))
                .await
        }
    }
}

/// Bring an earlier segment's entry to its total — completed where the
/// tracker has it finished — unless the row is already there. The
/// monotonic reconcile keeps it from lowering a row and keeps a rewatch.
async fn bring_to_total(
    ask: &MapAsk<'_>,
    earlier: &EarlierSegment,
    read: Option<(ProviderMediaId, TrackedMedia)>,
) -> Result<()> {
    let row = row_on(ask, earlier.entry, read).await?.entry;
    let done = row.is_some_and(|r| {
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
    if let Some(update) = reconcile_monotonic(update, row) {
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
/// absent or at 0), then the episode's own number on its own entry,
/// completed where it is that entry's last on a finished entry. The
/// fan-out's own finale status is not read here: the map knows which
/// entry the finale is the end of. A score the update carries goes with
/// the episode.
async fn write_mapped(
    ask: &MapAsk<'_>,
    write: &MappedWrite,
    update: &EntryUpdate,
    read: Option<(ProviderMediaId, TrackedMedia)>,
) -> Result<Option<ListEntry>> {
    let row = row_on(ask, write.entry, read).await?.entry;
    let starts_segment = row.is_none_or(|r| r.progress_episodes == 0);
    if starts_segment {
        for earlier in &write.earlier {
            bring_to_total(ask, earlier, read).await?;
        }
    }
    let ours = EntryUpdate {
        status: write.complete.then_some(ListStatus::Completed),
        progress_episodes: Some(write.progress),
        score_0_to_100: update.score_0_to_100,
        repeat_count: None,
    };
    let Some(ours) = reconcile_monotonic(ours, row) else {
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
    .map(Some)
}

#[cfg(test)]
#[path = "account_mapped_test.rs"]
mod tests;
