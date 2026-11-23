//! The watch order as the renderer reads it: each page's decisions, with
//! every entry carrying what its card says about it (aired or not, its
//! progress), so the renderer only words and draws them. Served by
//! `api::watch_order`; the renderer's copies of these types live in
//! `frontend/src/lib/api.ts`, held to `backend/wire-contract.json`.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::chain_end::{player_up_next, ChainEndCard, Finale, PlayerUpNext};
use super::entry::{is_unaired, progress_of, ProgressView};
use super::franchise_link::franchise_link_only;
use super::franchise_page::franchise_page;
use super::handoff::{handoff_ask, played_ids, rail_handoffs, AskRow, Handoff, PlayedRow};
use super::page_model::{watch_order_page, Eyebrow, Seams, StripCaption};
use super::sections::{FranchiseRow, FranchiseSection};
use super::strip_view::{franchise_summary, FranchiseSummary, StripKind};
use super::up_next::UpNext;
use super::{Franchise, HistoryRow, Progress, WatchEntry, WatchOrder, WatchRun};

/// An entry as a watch-order card shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ViewEntry {
    /// Its Kitsu id.
    pub id: String,
    /// Kitsu's canonical title.
    pub title: String,
    /// Its subtype as Kitsu serves it.
    pub subtype: Option<String>,
    /// The day it started, `YYYY-MM-DD`.
    pub start: Option<String>,
    /// Its episode count.
    pub episodes: Option<u32>,
    /// Its status.
    pub status: Option<String>,
    /// Its poster.
    pub poster: Option<String>,
    /// Its cover.
    pub cover: Option<String>,
    /// Not aired yet on the viewer's day: a future start, or none.
    pub unaired: bool,
    /// Its progress, as the card reads it.
    pub progress: ProgressView,
}

fn view_entry(e: &WatchEntry, progress: &Progress, today: &str) -> ViewEntry {
    ViewEntry {
        id: e.id.clone(),
        title: e.title.clone(),
        subtype: e.subtype.clone(),
        start: e.start.clone(),
        episodes: e.episodes,
        status: e.status.clone(),
        poster: e.poster.clone(),
        cover: e.cover.clone(),
        unaired: is_unaired(e, today),
        progress: progress_of(e, progress.get(&e.id).copied()),
    }
}

/// What a view's entries are read against.
#[derive(Clone, Copy)]
struct Reading<'a> {
    progress: &'a Progress,
    today: &'a str,
}

impl Reading<'_> {
    fn entry(self, e: &WatchEntry) -> ViewEntry {
        view_entry(e, self.progress, self.today)
    }

    fn entries(self, list: &[WatchEntry]) -> Vec<ViewEntry> {
        list.iter().map(|e| self.entry(e)).collect()
    }
}

/// The poster stack that links to the franchise page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stack {
    /// The main-story entry it is fronted by.
    pub front: String,
    /// Its counts and posters.
    pub summary: FranchiseSummary,
}

fn stack_of(run: &WatchRun, front: &str) -> Stack {
    Stack {
        front: front.to_string(),
        summary: franchise_summary(run, front),
    }
}

/// The entry the viewer is part-way through.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WatchingView {
    /// The entry.
    pub entry: ViewEntry,
    /// The furthest episode played.
    pub episode: f64,
}

/// The show page's strip.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StripWire {
    /// A run member, or a side story beside the run.
    pub kind: StripKind,
    /// What the strip's centre says.
    pub caption: StripCaption,
    /// The masthead eyebrow's place, where the run is one line.
    pub eyebrow: Option<Eyebrow>,
    /// Whether the franchise was read whole.
    pub complete: bool,
    /// Whether the page is outside the main story, drawn on a chain.
    pub outside: bool,
    /// Where a column holds a main-story title the chain meets, alone.
    pub seams: Seams,
    /// The column before the entry.
    pub before: Vec<ViewEntry>,
    /// The column after it.
    pub after: Vec<ViewEntry>,
    /// The entry being watched, when no tile shows it.
    pub watching: Option<WatchingView>,
    /// The link to the franchise page, stacked from the main story.
    pub stack: Stack,
    /// Ids "Similar titles" leaves out: the rest of the franchise.
    pub similar_hidden: Vec<String>,
}

/// The franchise link a show page shows on its own, with no strip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkWire {
    /// The link's poster stack.
    pub stack: Stack,
    /// Whether the franchise was read whole.
    pub complete: bool,
    /// Ids "Similar titles" leaves out: the rest of the franchise.
    pub similar_hidden: Vec<String>,
}

/// What the show page draws from the watch order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShowView {
    /// The order setting it was read in.
    pub order: WatchOrder,
    /// The strip, where the page shows one.
    pub strip: Option<StripWire>,
    /// The lone franchise link, where the page shows no strip.
    pub link: Option<LinkWire>,
}

/// The show page's view of `franchise` for `id`.
#[must_use]
pub fn show_view(
    franchise: &Franchise,
    rows: &[HistoryRow],
    id: &str,
    order: WatchOrder,
    today: &str,
) -> ShowView {
    let strip = watch_order_page(franchise, rows, id, order).map(|p| {
        let read = Reading {
            progress: &p.progress,
            today,
        };
        StripWire {
            kind: p.view.kind,
            caption: p.caption,
            eyebrow: p.eyebrow,
            complete: p.complete,
            outside: p.outside,
            seams: p.seams,
            before: read.entries(&p.view.before),
            after: read.entries(&p.view.after),
            watching: p.watching.as_ref().map(|w| WatchingView {
                entry: read.entry(&w.entry),
                episode: w.episode,
            }),
            stack: stack_of(&p.main_run, &p.front),
            similar_hidden: p.similar_hidden.into_iter().collect(),
        }
    });
    let link = if strip.is_some() {
        None
    } else {
        franchise_link_only(franchise, id).map(|l| LinkWire {
            stack: stack_of(&l.run, &l.front),
            complete: l.complete,
            similar_hidden: l.similar_hidden.into_iter().collect(),
        })
    };
    ShowView { order, strip, link }
}

/// One title of the franchise page's main story.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RowWire {
    /// The title.
    pub entry: ViewEntry,
    /// The oversized numeral, on numbered stretches only.
    pub position: Option<usize>,
    /// Share watched, 0–1, for the progress line.
    pub share: f64,
    /// The title this one leads into, where it only leads into another.
    pub leads_into: Option<String>,
    /// The title the page was opened from.
    pub here: bool,
}

/// One part of the main story.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SectionWire {
    /// A numbered stretch, or the titles with no clear place.
    Line {
        /// Its rows.
        rows: Vec<RowWire>,
        /// The titles with no clear place in the order.
        unplaced: bool,
    },
    /// Ways to continue, side by side.
    Ways {
        /// Each way's rows.
        ways: Vec<Vec<RowWire>>,
    },
    /// A group with no clear order.
    Group {
        /// Its rows.
        rows: Vec<RowWire>,
    },
}

/// Where Resume goes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResumeWire {
    /// The title.
    pub entry: ViewEntry,
    /// The furthest episode played.
    pub episode: f64,
    /// Its number, for a franchise read whole.
    pub position: Option<usize>,
}

/// A title the topbar trail links back to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TitleRef {
    /// Its Kitsu id.
    pub id: String,
    /// Its title.
    pub title: String,
}

/// The franchise page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FranchiseWire {
    /// The franchise's name.
    pub name: String,
    /// The title the name comes from: the masthead's artwork and accent.
    pub art: WatchEntry,
    /// Every title the page lists; none for a franchise read in part.
    pub total: Option<usize>,
    /// The franchise is the show alone, from a read whole.
    pub alone: bool,
    /// Where Resume goes.
    pub resume: Option<ResumeWire>,
    /// Where "Start from the beginning" goes.
    pub start: Option<ViewEntry>,
    /// The main story.
    pub sections: Vec<SectionWire>,
    /// Other versions.
    pub versions: Vec<WatchEntry>,
    /// Films.
    pub films: Vec<WatchEntry>,
    /// Side stories.
    pub sides: Vec<WatchEntry>,
    /// Recaps.
    pub recaps: Vec<WatchEntry>,
    /// How many recaps the heading counts: none for a read in part.
    pub recap_count: Option<usize>,
    /// The franchise read stopped early.
    pub partial: bool,
    /// The title the page was opened from, where the page lists it.
    pub opened_from: Option<TitleRef>,
}

/// What the franchise page draws.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FranchiseView {
    /// The order setting it was read in.
    pub order: WatchOrder,
    /// The page, where there is a franchise to show.
    pub page: Option<FranchiseWire>,
}

fn row_wire(read: Reading<'_>, r: FranchiseRow) -> RowWire {
    RowWire {
        entry: read.entry(&r.entry),
        position: r.position,
        share: r.share,
        leads_into: r.leads_into,
        here: r.here,
    }
}

fn section_wire(read: Reading<'_>, s: FranchiseSection) -> SectionWire {
    let rows = |rows: Vec<FranchiseRow>| rows.into_iter().map(|r| row_wire(read, r)).collect();
    match s {
        FranchiseSection::Line { rows: r, unplaced } => SectionWire::Line {
            rows: rows(r),
            unplaced,
        },
        FranchiseSection::Ways { ways } => SectionWire::Ways {
            ways: ways.into_iter().map(rows).collect(),
        },
        FranchiseSection::Group { rows: r } => SectionWire::Group { rows: rows(r) },
    }
}

/// The franchise page's view of `franchise` for `id`.
#[must_use]
pub fn franchise_view(
    franchise: &Franchise,
    rows: &[HistoryRow],
    id: &str,
    order: WatchOrder,
    today: &str,
) -> FranchiseView {
    let page = franchise_page(franchise, rows, id, order).map(|p| {
        let progress: Progress = super::progress::progress_from_history(rows, &p.run.entries);
        let read = Reading {
            progress: &progress,
            today,
        };
        FranchiseWire {
            name: p.name,
            total: p.total,
            alone: p.alone,
            resume: p.resume.map(|r| ResumeWire {
                entry: read.entry(&r.entry),
                episode: r.episode,
                position: r.position,
            }),
            start: p.start.as_ref().map(|e| read.entry(e)),
            sections: p
                .sections
                .into_iter()
                .map(|s| section_wire(read, s))
                .collect(),
            versions: p.run.versions.clone(),
            films: p.run.films.clone(),
            sides: p.run.sides.clone(),
            recaps: p.run.recaps.clone(),
            recap_count: p.recap_count,
            partial: p.partial,
            opened_from: p.opened_from.map(|e| TitleRef {
                id: e.id,
                title: e.title,
            }),
            art: p.art,
        }
    });
    FranchiseView { order, page }
}

/// What follows a finale, its entries as the card shows them.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NextWire {
    /// One entry that has aired.
    One {
        /// The entry.
        entry: ViewEntry,
    },
    /// One entry that has not aired yet.
    Unaired {
        /// The entry.
        entry: ViewEntry,
    },
    /// Several ways on.
    Choices {
        /// The entries.
        entries: Vec<ViewEntry>,
    },
}

/// The one card the player shows at a finale.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "card", rename_all = "lowercase")]
pub enum UpNextWire {
    /// What comes next.
    Entry(NextWire),
    /// The franchise page, at a chain's dead end.
    Franchise(ChainEndCard),
}

/// What the player's Up next card shows at `episode`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpNextView {
    /// The card, where the episode is a finale that has one.
    pub card: Option<UpNextWire>,
}

/// The player's view of `franchise` for `id` at `episode`. `own` is the
/// player's own reading of the entry, where it has one.
#[must_use]
pub fn up_next_view(franchise: &Franchise, rows: &[HistoryRow], at: Finale<'_>) -> UpNextView {
    let card = player_up_next(franchise, rows, at).map(|card| match card {
        PlayerUpNext::Franchise(c) => UpNextWire::Franchise(c),
        PlayerUpNext::Entry(c) => {
            let read = Reading {
                progress: &c.progress,
                today: at.today,
            };
            UpNextWire::Entry(match c.up_next {
                UpNext::One { entry } => NextWire::One {
                    entry: read.entry(&entry),
                },
                UpNext::Unaired { entry } => NextWire::Unaired {
                    entry: read.entry(&entry),
                },
                UpNext::Choices { entries } => NextWire::Choices {
                    entries: read.entries(&entries),
                },
                // A finale card never says nothing follows.
                UpNext::None => NextWire::Choices {
                    entries: Vec::new(),
                },
            })
        }
    });
    UpNextView { card }
}

/// What Home sends to read its rail's hand-offs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RailRequest {
    /// The rail's rows, in order.
    pub rows: Vec<AskRow>,
    /// Every history row, for the played set.
    pub history: Vec<PlayedRow>,
    /// Each history row's Kitsu match: its id, or none for a row matched
    /// to nothing. A row still being matched is absent.
    pub matches: BTreeMap<String, Option<String>>,
    /// The hand-offs read so far, by the Kitsu id that asked: each
    /// series' candidate (none where it has none, or it failed).
    pub series: BTreeMap<String, Option<Handoff>>,
}

/// Home's rail: which ids to read a hand-off for, and the hand-offs it
/// shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RailView {
    /// Each row's ask: the Kitsu id whose hand-off it needs, or none.
    pub asks: BTreeMap<String, Option<String>>,
    /// The hand-off each row shows, by row id.
    pub handoffs: BTreeMap<String, Handoff>,
}

/// Home's rail from what it has read so far. A hand-off shows once the
/// played set is known (every row matched), its next entry is not
/// played, and no row earlier in the rail moves on to the same entry.
#[must_use]
pub fn rail_view(req: &RailRequest) -> RailView {
    let asks: Vec<(String, Option<String>)> = req
        .rows
        .iter()
        .map(|r| (r.row_id.clone(), handoff_ask(r)))
        .collect();
    let played: Option<BTreeSet<String>> = played_ids(&req.history, &req.matches);
    let handoffs = match &played {
        None => BTreeMap::new(),
        Some(played) => rail_handoffs(&asks, |k| {
            req.series
                .get(k)
                .cloned()
                .flatten()
                .filter(|h| !played.contains(&h.next.id))
        }),
    };
    RailView {
        asks: asks.into_iter().collect(),
        handoffs,
    }
}
