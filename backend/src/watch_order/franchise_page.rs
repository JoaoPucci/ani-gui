//! Everything the franchise page draws, in one reading of the franchise
//! payload and the watch history: the masthead (the name, its artwork,
//! the title count and where Resume goes), the main story as numbered
//! stretches, branches side by side and groups with no clear order, and
//! the rails around it. A franchise read in part gets no count, number
//! or beginning.

use serde::Serialize;

use super::place::placement;
use super::progress::progress_from_history;
use super::run::run_of;
use super::sections::{main_story, FranchiseRow, FranchiseSection, RowContext};
use super::segments::segments;
use super::sequence::sequence;
use super::strip_view::{current_in, franchise_summary};
use super::{Franchise, HistoryRow, WatchEntry, WatchOrder, WatchRun};

/// Where Resume goes: the title the viewer is part-way through.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resume {
    /// The title.
    pub entry: WatchEntry,
    /// The furthest episode played.
    pub episode: f64,
    /// Its number, for a franchise read whole.
    pub position: Option<usize>,
}

/// The franchise page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FranchisePageModel {
    /// The order it was read in.
    pub order: WatchOrder,
    /// The run.
    pub run: WatchRun,
    /// The franchise's name, as the walk decides it.
    pub name: String,
    /// The title the name comes from, whose artwork and accent the
    /// masthead takes.
    pub art: WatchEntry,
    /// Every title the page lists; none for a franchise read in part.
    pub total: Option<usize>,
    /// The franchise is the show alone, from a read whole.
    pub alone: bool,
    /// Where Resume goes.
    pub resume: Option<Resume>,
    /// Where "Start from the beginning" goes; none for a franchise read
    /// in part, whose first title read may not be the first.
    pub start: Option<WatchEntry>,
    /// The main story.
    pub sections: Vec<FranchiseSection>,
    /// How many recaps the heading counts: none for a read in part.
    pub recap_count: Option<usize>,
    /// The franchise read stopped early, so there may be more titles.
    pub partial: bool,
    /// The title the page was opened from, where the page lists it: the
    /// topbar trail's link back.
    pub opened_from: Option<WatchEntry>,
}

/// `section` as a franchise read in part shows it: without its numbers.
fn unnumbered(section: FranchiseSection) -> FranchiseSection {
    let blank = |rows: Vec<FranchiseRow>| -> Vec<FranchiseRow> {
        rows.into_iter()
            .map(|r| FranchiseRow {
                position: None,
                ..r
            })
            .collect()
    };
    match section {
        FranchiseSection::Ways { ways } => FranchiseSection::Ways {
            ways: ways.into_iter().map(blank).collect(),
        },
        FranchiseSection::Line { rows, unplaced } => FranchiseSection::Line {
            rows: blank(rows),
            unplaced,
        },
        FranchiseSection::Group { rows } => FranchiseSection::Group { rows: blank(rows) },
    }
}

/// Whether `run` lists `id` and nothing else.
fn is_alone(run: &WatchRun, id: &str) -> bool {
    let mut all = run.all();
    matches!((all.next(), all.next()), (Some(e), None) if e.id == id)
}

/// The page for `id`, or none where there is no franchise to show.
#[must_use]
pub fn franchise_page(
    franchise: &Franchise,
    rows: &[HistoryRow],
    id: &str,
    order: WatchOrder,
) -> Option<FranchisePageModel> {
    if franchise.anchor != id {
        return None;
    }
    let run = run_of(franchise)?;
    let progress = progress_from_history(rows, &run.entries);
    let ctx = RowContext {
        run: &run,
        order,
        progress: &progress,
        id,
    };
    // Read in part, the walk may have missed titles before the first one
    // it reached: nothing counted from that title is sure.
    let whole = franchise.complete;
    let art = run
        .entries
        .iter()
        .find(|e| e.id == franchise.name_from)
        .unwrap_or(&run.entries[0])
        .clone();
    let opened_from = run.all().find(|e| e.id == id).cloned();
    let segs = segments(&run, order);
    let sections = main_story(ctx, &segs);
    let resume = current_in(&run, &progress, order).map(|now| Resume {
        position: if whole {
            placement(&run, &now.entry.id, order).position
        } else {
            None
        },
        entry: now.entry,
        episode: now.episode,
    });
    Some(FranchisePageModel {
        order,
        name: franchise.name.clone(),
        total: whole.then(|| franchise_summary(&run, &art.id).total),
        alone: whole && is_alone(&run, id),
        resume,
        start: if whole {
            sequence(&run, order).first().map(|e| (*e).clone())
        } else {
            None
        },
        sections: if whole {
            sections
        } else {
            sections.into_iter().map(unnumbered).collect()
        },
        recap_count: whole.then_some(run.recaps.len()),
        partial: !franchise.complete,
        opened_from,
        art,
        run,
    })
}
