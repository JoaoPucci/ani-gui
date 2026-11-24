//! The franchise page as the show page reaches it: what it lists beside
//! the show, and the lone link where the show has no strip to carry it.

use std::collections::BTreeSet;

use serde::Serialize;

use super::run::{has_strip, run_of};
use super::{Franchise, WatchRun};

/// Every title the franchise page lists for `run` but `id` itself: the
/// run, its other versions, films, side stories and recaps.
#[must_use]
pub fn rest_of_franchise(run: &WatchRun, id: &str) -> BTreeSet<String> {
    run.all()
        .map(|e| e.id.clone())
        .filter(|x| x != id)
        .collect()
}

/// The way to the franchise page where the show has no strip to carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FranchiseLinkModel {
    /// The run the link's stack draws from.
    pub run: WatchRun,
    /// The main-story entry the link's poster stack is fronted by.
    pub front: String,
    /// Whether the franchise was read whole.
    pub complete: bool,
    /// Ids "Similar titles" leaves out: the rest of the franchise.
    pub similar_hidden: BTreeSet<String>,
}

/// The franchise link the page shows on its own: where `id` has no strip
/// (it is its run alone, or a film, version, recap or side story outside
/// the run with no parent story in it) but the franchise page would list
/// other titles. None where the strip shows, which carries the link
/// itself, and where the franchise page would list the show alone.
#[must_use]
pub fn franchise_link_only(franchise: &Franchise, id: &str) -> Option<FranchiseLinkModel> {
    if franchise.anchor != id {
        return None;
    }
    let run = run_of(franchise)?;
    if has_strip(&run, id) {
        return None;
    }
    let rest = rest_of_franchise(&run, id);
    if rest.is_empty() {
        return None;
    }
    Some(FranchiseLinkModel {
        run,
        front: franchise.name_from.clone(),
        complete: franchise.complete,
        similar_hidden: rest,
    })
}
