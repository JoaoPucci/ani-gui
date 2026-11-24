//! Everything the show page draws from the watch order, in one reading:
//! the strip, its caption, the masthead eyebrow's position, each tile's
//! progress, the "You're watching" card, and the titles "Similar
//! titles" leaves out because the franchise page the strip links to
//! lists them.

use std::collections::BTreeSet;

use serde::Serialize;

use super::chain_view::{chain_franchise, own_franchise, seam_of, Seam, Side};
use super::franchise_link::rest_of_franchise;
use super::progress::progress_from_history;
use super::run::run_of;
use super::segments::loop_size;
use super::strip_view::{strip_for, watching_for, StripKind, StripView, Watching};
use super::{Franchise, HistoryRow, Progress, WatchOrder, WatchRun};

/// What the strip's centre says about this entry's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum StripCaption {
    /// "05 of 09".
    Position {
        /// Its number.
        position: usize,
        /// The run's length.
        total: usize,
    },
    /// A number with no total.
    Numeral {
        /// Its number.
        position: usize,
    },
    /// A run member's place in a franchise read in part: nothing about
    /// it is sure.
    Partial,
    /// A side story.
    Side,
    /// A side story the payload lists under films, or the one own title
    /// of a chain when it is a movie.
    Film,
    /// A member of a loop of `count`.
    Cycle {
        /// The loop's size.
        count: usize,
    },
    /// An unnumbered branch.
    Branch,
    /// The one own title of a chain that is not a film: nothing to count.
    Alone,
}

/// The caption for a view; `looped` is the size of the cycle `view` sits
/// in, if any. `complete` is false for a franchise read in part: no
/// number, fork or loop size is sure, and a run member's caption is
/// `partial`. `film` says the shown entry is listed under films.
#[must_use]
pub fn caption_of(view: &StripView, looped: usize, complete: bool, film: bool) -> StripCaption {
    if view.kind == StripKind::Side {
        return if film {
            StripCaption::Film
        } else {
            StripCaption::Side
        };
    }
    if !complete {
        return StripCaption::Partial;
    }
    match (view.position, view.total) {
        (Some(position), Some(total)) => StripCaption::Position { position, total },
        (Some(position), None) => StripCaption::Numeral { position },
        _ if view.in_cycle => StripCaption::Cycle { count: looped },
        _ => StripCaption::Branch,
    }
}

/// The caption of a run member on a chain: its place among the chain's
/// own titles, seams uncounted. A chain of one own title reads as a film
/// when it is one, and with no number otherwise.
fn chain_caption(franchise: &Franchise, id: &str, order: WatchOrder) -> StripCaption {
    if !franchise.complete {
        return StripCaption::Partial;
    }
    let strip = own_franchise(franchise).and_then(|own| strip_for(&own, id, order));
    let Some(strip) = strip else {
        let own = franchise
            .chain
            .as_ref()
            .and_then(|c| c.entries.iter().find(|e| e.id == id));
        return if own.and_then(|e| e.subtype.as_deref()) == Some("movie") {
            StripCaption::Film
        } else {
            StripCaption::Alone
        };
    };
    caption_of(&strip.view, loop_size(&strip.run, id, order), true, false)
}

/// The seams a strip's two columns show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Seams {
    /// The column before the entry.
    pub before: Option<Seam>,
    /// The column after it.
    pub after: Option<Seam>,
}

/// "05 of 09" in the masthead eyebrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Eyebrow {
    /// Its number.
    pub position: usize,
    /// The run's length.
    pub total: usize,
}

/// The show page's reading of the watch order for one id.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WatchOrderPage {
    /// The order it was read in.
    pub order: WatchOrder,
    /// The run the strip draws.
    pub run: WatchRun,
    /// The franchise's main story as a run: what the strip's franchise
    /// link stacks. `run` itself on a page in the main story.
    pub main_run: WatchRun,
    /// The main-story entry the strip's poster stack is fronted by.
    pub front: String,
    /// What the strip shows.
    pub view: StripView,
    /// Whether the franchise was read whole.
    pub complete: bool,
    /// What the strip's centre says.
    pub caption: StripCaption,
    /// The masthead eyebrow's place: only where the run is one line.
    pub eyebrow: Option<Eyebrow>,
    /// Progress per run member.
    pub progress: Progress,
    /// The entry the viewer is part-way through, when no tile shows it.
    pub watching: Option<Watching>,
    /// Ids "Similar titles" leaves out: the rest of the franchise.
    pub similar_hidden: BTreeSet<String>,
    /// Whether the page is outside the main story, drawn on a chain.
    pub outside: bool,
    /// Where a column holds a main-story title the chain meets, alone.
    pub seams: Seams,
}

/// The page's reading of `franchise` and `rows` for `id`, or none where
/// the page shows no strip.
#[must_use]
pub fn watch_order_page(
    franchise: &Franchise,
    rows: &[HistoryRow],
    id: &str,
    order: WatchOrder,
) -> Option<WatchOrderPage> {
    let drawn = chain_franchise(franchise);
    let strip = strip_for(drawn.as_ref().unwrap_or(franchise), id, order)?;
    let progress = progress_from_history(rows, &strip.run.entries);
    let watching = watching_for(&strip.run, &progress, id, &strip.view, order);
    let film = strip.run.films.iter().any(|e| e.id == id);
    let on_chain = drawn.is_some() && strip.view.kind == StripKind::Run;
    let caption = if on_chain {
        chain_caption(franchise, id, order)
    } else {
        caption_of(
            &strip.view,
            loop_size(&strip.run, id, order),
            strip.complete,
            film,
        )
    };
    let seams: &[String] = match (&franchise.chain, on_chain) {
        (Some(chain), true) => &chain.seams,
        _ => &[],
    };
    let main_run = drawn
        .as_ref()
        .and_then(|_| run_of(franchise))
        .unwrap_or_else(|| strip.run.clone());
    let eyebrow = match caption {
        StripCaption::Position { position, total } => Some(Eyebrow { position, total }),
        _ => None,
    };
    Some(WatchOrderPage {
        order,
        similar_hidden: rest_of_franchise(&main_run, id),
        seams: Seams {
            before: seam_of(&strip.view.before, seams, Side::Before),
            after: seam_of(&strip.view.after, seams, Side::After),
        },
        outside: drawn.is_some(),
        run: strip.run,
        main_run,
        front: strip.front,
        view: strip.view,
        complete: strip.complete,
        caption,
        eyebrow,
        progress,
        watching,
    })
}
