//! The watch order's reads: the franchise walk (`commands::franchise`),
//! the history rows its entries resume from, as the resume lookup names
//! them (`commands::history_by_kitsu`), and the order setting. The
//! decisions are `crate::watch_order`'s.
//!
//! A franchise that cannot be read fails the read (the page shows no
//! strip, no card); history that cannot be read reads as no rows (a
//! strip without progress).

use std::collections::BTreeSet;

use crate::app::AppState;
use crate::error::Result;
use crate::watch_order::chain_end::Finale;
use crate::watch_order::handoff::{handoff_from, unclaimed_handoff, Handoff};
use crate::watch_order::up_next::FinaleSelf;
use crate::watch_order::wire::{
    franchise_view, show_view, up_next_view, FranchiseView, ShowView, UpNextView,
};
use crate::watch_order::{Franchise, HistoryRow, WatchOrder};

/// Every id the franchise names, its chain's included, each once.
#[must_use]
pub fn ids_of(f: &Franchise) -> Vec<String> {
    let mut seen = BTreeSet::new();
    f.entries
        .iter()
        .chain(&f.versions)
        .chain(&f.films)
        .chain(&f.sides)
        .chain(&f.recaps)
        .chain(f.chain.iter().flat_map(|c| &c.entries))
        .filter(|e| seen.insert(e.id.clone()))
        .map(|e| e.id.clone())
        .collect()
}

/// The rows `ids` resume from, or none when the history cannot be read.
fn rows_for(state: &AppState, ids: &[String]) -> Vec<HistoryRow> {
    crate::commands::history::history_by_kitsu_each(state, ids)
        .map(|named| {
            named
                .into_iter()
                .filter_map(|(kitsu_id, row)| {
                    row.map(|r| HistoryRow {
                        kitsu_id: Some(kitsu_id),
                        ep_no: r.ep_no,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The order setting, linked where the settings cannot be read.
fn order(state: &AppState) -> WatchOrder {
    let config = crate::config::read_config(&state.config_path).unwrap_or_default();
    WatchOrder::of(&config.watch_order)
}

/// `id`'s franchise and the rows its entries resume from.
async fn read(state: &AppState, id: &str) -> Result<(Franchise, Vec<HistoryRow>)> {
    let franchise = crate::commands::franchise::franchise(state, id).await?;
    let rows = rows_for(state, &ids_of(&franchise));
    Ok((franchise, rows))
}

/// What the show page draws for `id` on `today`.
///
/// # Errors
/// As the walk's.
pub async fn show(state: &AppState, id: &str, today: &str) -> Result<ShowView> {
    let (franchise, rows) = read(state, id).await?;
    Ok(show_view(&franchise, &rows, id, order(state), today))
}

/// What the franchise page draws for `id` on `today`.
///
/// # Errors
/// As the walk's.
pub async fn franchise_page(state: &AppState, id: &str, today: &str) -> Result<FranchiseView> {
    let (franchise, rows) = read(state, id).await?;
    Ok(franchise_view(&franchise, &rows, id, order(state), today))
}

/// What the player's Up next card shows for `id` at `episode`. The
/// player's own reading of the entry is its detail, the read the player
/// itself made (cached), where it can be read.
///
/// # Errors
/// As the walk's.
pub async fn up_next(state: &AppState, id: &str, today: &str, episode: f64) -> Result<UpNextView> {
    let (franchise, rows) = read(state, id).await?;
    let own = crate::commands::kitsu::kitsu_anime_detail(state, id)
        .await
        .ok()
        .map(|d| FinaleSelf {
            status: d.status,
            episodes: d.episode_count,
        });
    let at = Finale {
        id,
        order: order(state),
        today,
        episode,
        own: own.as_ref(),
    };
    Ok(up_next_view(&franchise, &rows, at))
}

/// The hand-off from `id`, the entry a Continue row played through: the
/// entry that comes next, once the resume lookup names no history row
/// for it. None where there is none, or the lookup fails.
///
/// # Errors
/// As the walk's.
pub async fn handoff(state: &AppState, id: &str, today: &str) -> Result<Option<Handoff>> {
    let franchise = crate::commands::franchise::franchise(state, id).await?;
    let candidate = handoff_from(&franchise, id, order(state), today, &BTreeSet::new());
    let named = match &candidate {
        Some(h) => {
            crate::commands::history::history_by_kitsu_each(state, std::slice::from_ref(&h.next.id))
                .ok()
                .map(|rows| rows.get(&h.next.id).is_some_and(Option::is_some))
        }
        None => None,
    };
    Ok(unclaimed_handoff(candidate, named))
}

#[cfg(test)]
#[path = "watch_order_test.rs"]
mod tests;
