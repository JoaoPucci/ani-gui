//! What the show page's watch-order strip shows for one entry: its place
//! in its run, or, for a side story, the stories Kitsu names as its
//! parents; the franchise's counts; and the entry the viewer is on.

use serde::Serialize;

use super::place::placement;
use super::run::{has_strip, run_of};
use super::sequence::sequence;
use super::{EntryProgress, Franchise, Progress, WatchEntry, WatchOrder, WatchRun};

/// Whether the strip shows a run member or a side story beside the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StripKind {
    /// A member of the run.
    Run,
    /// A side story beside it.
    Side,
}

/// The strip's place for one entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StripView {
    /// A member of the run, or a side story beside it.
    pub kind: StripKind,
    /// Its number, where it has one.
    pub position: Option<usize>,
    /// The run's length, where the order is one line.
    pub total: Option<usize>,
    /// The column before it: prequels, or a side story's parents.
    pub before: Vec<WatchEntry>,
    /// The column after it.
    pub after: Vec<WatchEntry>,
    /// Whether it sits in a cycle.
    pub in_cycle: bool,
}

/// `id`'s strip in `run`.
#[must_use]
pub fn strip_view(run: &WatchRun, id: &str, order: WatchOrder) -> StripView {
    let Some(parents) = run.parents.get(id) else {
        let p = placement(run, id, order);
        return StripView {
            kind: StripKind::Run,
            position: p.position,
            total: p.total,
            before: p.before,
            after: p.after,
            in_cycle: p.in_cycle,
        };
    };
    let all: Vec<&WatchEntry> = run
        .entries
        .iter()
        .chain(&run.versions)
        .chain(&run.films)
        .chain(&run.sides)
        .collect();
    StripView {
        kind: StripKind::Side,
        position: None,
        total: None,
        before: parents
            .iter()
            .filter_map(|p| all.iter().find(|e| e.id == *p).map(|e| (*e).clone()))
            .collect(),
        after: Vec::new(),
        in_cycle: false,
    }
}

/// The counts behind the "Whole franchise" control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FranchiseSummary {
    /// Every entry the franchise page lists, recaps included.
    pub total: usize,
    /// The main story's titles.
    pub main: usize,
    /// Films and other versions.
    pub films: usize,
    /// Side stories.
    pub sides: usize,
    /// Recaps.
    pub recaps: usize,
    /// Up to three entries with a poster to fan out, from different
    /// parts of the franchise, the front entry first.
    pub posters: Vec<WatchEntry>,
}

/// The counts of `run`, its poster stack fronted by the main-story entry
/// `front`.
#[must_use]
pub fn franchise_summary(run: &WatchRun, front: &str) -> FranchiseSummary {
    // A payload naming no entry of the run fronts its first, as the
    // franchise page takes the first entry's artwork then.
    let first = run
        .entries
        .iter()
        .find(|e| e.id == front)
        .or(run.entries.first());
    let candidates = [
        first,
        run.films.first().or(run.versions.first()),
        run.sides.first(),
    ]
    .into_iter()
    .chain(
        run.entries
            .iter()
            .filter(|e| !first.is_some_and(|f| std::ptr::eq(*e, f)))
            .map(Some),
    );
    let mut posters: Vec<&WatchEntry> = Vec::new();
    for c in candidates.flatten() {
        if c.poster.is_some() && !posters.iter().any(|p| std::ptr::eq(*p, c)) && posters.len() < 3 {
            posters.push(c);
        }
    }
    FranchiseSummary {
        total: run.all().count(),
        main: run.entries.len(),
        films: run.films.len() + run.versions.len(),
        sides: run.sides.len(),
        recaps: run.recaps.len(),
        posters: posters.into_iter().cloned().collect(),
    }
}

/// The entry the viewer is part-way through, and the episode.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Watching {
    /// The entry.
    pub entry: WatchEntry,
    /// The furthest episode played.
    pub episode: f64,
}

/// The entry the viewer is on: the latest started, not finished, in order.
#[must_use]
pub fn current_in(run: &WatchRun, progress: &Progress, order: WatchOrder) -> Option<Watching> {
    sequence(run, order)
        .into_iter()
        .rev()
        .find_map(|e| match progress.get(&e.id) {
            Some(EntryProgress::Episode(episode)) => Some(Watching {
                entry: e.clone(),
                episode: *episode,
            }),
            _ => None,
        })
}

/// The entry the viewer is part-way through, for the strip's "You're
/// watching" line: none when it is the entry being viewed or already one
/// of the strip's tiles, which shows its progress itself.
#[must_use]
pub fn watching_for(
    run: &WatchRun,
    progress: &Progress,
    id: &str,
    view: &StripView,
    order: WatchOrder,
) -> Option<Watching> {
    let now = current_in(run, progress, order)?;
    if now.entry.id == id {
        return None;
    }
    let shown = view
        .before
        .iter()
        .chain(&view.after)
        .any(|e| e.id == now.entry.id);
    (!shown).then_some(now)
}

/// The strip on one page: the run, its front entry, the view, and
/// whether the franchise was read whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripModel {
    /// The run the ordering works on.
    pub run: WatchRun,
    /// The main-story entry the poster stack is fronted by.
    pub front: String,
    /// What the strip shows.
    pub view: StripView,
    /// False when the walk stopped at its bound or read a listing cut
    /// short: entries may be missing on either side, so the strip claims
    /// nothing that needs the whole.
    pub complete: bool,
}

/// What the strip shows on `id`'s page, or none: the payload answers
/// another id, the run is `id` alone with no parents, or `id` is neither
/// a member nor a side story with parents in the run.
#[must_use]
pub fn strip_for(franchise: &Franchise, id: &str, order: WatchOrder) -> Option<StripModel> {
    if franchise.anchor != id {
        return None;
    }
    let run = run_of(franchise)?;
    if !has_strip(&run, id) {
        return None;
    }
    Some(StripModel {
        view: strip_view(&run, id, order),
        run,
        front: franchise.name_from.clone(),
        complete: franchise.complete,
    })
}
