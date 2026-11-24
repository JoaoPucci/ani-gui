//! The franchise page's main story as it is laid out: one row per
//! title, and the sections the rows stand in (numbered stretches, ways
//! to continue side by side, groups with no clear order, and titles with
//! no clear place).

use serde::Serialize;

use super::entry::{progress_of, progress_share};
use super::no_clear_place::gather_unplaced;
use super::place::placement;
use super::rail::{path_note, PathNote};
use super::segments::Segment;
use super::{EntryProgress, Progress, WatchEntry, WatchOrder, WatchRun};

/// One title of the main story.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FranchiseRow {
    /// The title.
    pub entry: WatchEntry,
    /// The oversized numeral, on numbered stretches only.
    pub position: Option<usize>,
    /// Its progress, where a row names it.
    pub progress: Option<EntryProgress>,
    /// Share watched, 0–1, for the progress line.
    pub share: f64,
    /// The title this one leads into, where it only leads into another.
    pub leads_into: Option<String>,
    /// The title the page was opened from.
    pub here: bool,
}

/// One part of the main story, as the page lays it out.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FranchiseSection {
    /// A numbered stretch, or the titles with no clear place.
    Line {
        /// Its rows.
        rows: Vec<FranchiseRow>,
        /// Titles with no clear place in the order: no number, and no
        /// note or a note into such a title.
        unplaced: bool,
    },
    /// Ways to continue, side by side.
    Ways {
        /// Each way's rows.
        ways: Vec<Vec<FranchiseRow>>,
    },
    /// A group with no clear order (a loop).
    Group {
        /// Its rows.
        rows: Vec<FranchiseRow>,
    },
}

/// What the rows of one page share.
#[derive(Debug, Clone, Copy)]
pub struct RowContext<'a> {
    /// The run.
    pub run: &'a WatchRun,
    /// The order.
    pub order: WatchOrder,
    /// Progress per member.
    pub progress: &'a Progress,
    /// The title the page was opened from.
    pub id: &'a str,
}

fn row_of(ctx: RowContext<'_>, entry: &WatchEntry, numbered: bool) -> FranchiseRow {
    let progress = ctx.progress.get(&entry.id).copied();
    let note = path_note(ctx.run, &entry.id, ctx.order);
    FranchiseRow {
        entry: entry.clone(),
        position: if numbered {
            placement(ctx.run, &entry.id, ctx.order).position
        } else {
            None
        },
        progress,
        share: progress_share(progress_of(entry, progress)),
        leads_into: match note {
            Some(PathNote::LeadsInto { entry }) => Some(entry.title),
            _ => None,
        },
        here: entry.id == ctx.id,
    }
}

/// One segment as the page draws it. A fork left with one way (its other
/// ways start at a join or in a loop) is drawn as a line.
fn section_of(ctx: RowContext<'_>, seg: &Segment) -> FranchiseSection {
    let rows = |list: &[WatchEntry], numbered: bool| -> Vec<FranchiseRow> {
        list.iter().map(|e| row_of(ctx, e, numbered)).collect()
    };
    match seg {
        Segment::Fork { branches, .. } if branches.len() > 1 => FranchiseSection::Ways {
            ways: branches.iter().map(|b| rows(b, false)).collect(),
        },
        Segment::Cycle { entries } => FranchiseSection::Group {
            rows: rows(entries, false),
        },
        Segment::Fork { branches, .. } => FranchiseSection::Line {
            rows: rows(&branches.concat(), true),
            unplaced: false,
        },
        Segment::Line { entries } => FranchiseSection::Line {
            rows: rows(entries, true),
            unplaced: false,
        },
    }
}

/// The main story's sections, from the run's segments. The titles with
/// no clear place leave their lines and gather in one stretch at the
/// end, so nothing numbered follows it.
#[must_use]
pub fn main_story(ctx: RowContext<'_>, segs: &[Segment]) -> Vec<FranchiseSection> {
    gather_unplaced(ctx, segs.iter().map(|s| section_of(ctx, s)).collect())
}
