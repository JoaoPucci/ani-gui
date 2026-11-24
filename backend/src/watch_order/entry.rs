//! Readings of one entry: its progress as a card shows it, the share
//! watched, whether it has aired, and its year.

use serde::Serialize;

use super::{EntryProgress, WatchEntry};

/// An entry's progress as a card reads it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProgressView {
    /// No row names it.
    None,
    /// A row reached its last episode.
    Watched,
    /// Part-way: the furthest episode, and the count where known.
    Partway {
        /// The furthest episode played.
        episode: f64,
        /// The entry's episode count, where Kitsu knows it.
        total: Option<u32>,
    },
}

/// `progress` is the entry's own, or none when it has none.
#[must_use]
pub fn progress_of(entry: &WatchEntry, progress: Option<EntryProgress>) -> ProgressView {
    match progress {
        None => ProgressView::None,
        Some(EntryProgress::Watched) => ProgressView::Watched,
        Some(EntryProgress::Episode(episode)) => ProgressView::Partway {
            episode,
            total: entry.episodes,
        },
    }
}

/// Share of the entry watched, 0–1, for a progress line.
#[must_use]
pub fn progress_share(view: ProgressView) -> f64 {
    match view {
        ProgressView::Watched => 1.0,
        ProgressView::Partway {
            episode,
            total: Some(total),
        } if total > 0 => (episode / f64::from(total)).min(1.0),
        _ => 0.0,
    }
}

/// Not aired yet: a future start, or no start at all. `today` is the
/// viewer's local day, `YYYY-MM-DD`.
#[must_use]
pub fn is_unaired(entry: &WatchEntry, today: &str) -> bool {
    entry.start.as_deref().is_none_or(|start| start > today)
}

/// "2023" from a start date, or none.
#[must_use]
pub fn year_of(entry: &WatchEntry) -> Option<String> {
    entry.start.as_ref().map(|s| s.chars().take(4).collect())
}
