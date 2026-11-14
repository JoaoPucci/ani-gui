//! A page's own chain, when the main story (`franchise_walk_root.rs`)
//! leaves the page out: the franchise's entries outside the main story
//! joined to the page by prequel and sequel links among themselves (its
//! own titles), and the main-story titles one of them is linked to,
//! where the chain leaves the main story or leads into it (its seams).
//! A link counts from either side of the listings read.
//!
//! Every page that reaches the same franchise reads it the same way, so
//! every title of a chain answers with the same chain where Kitsu lists
//! each link from both sides (a link listed on one side only is seen
//! only by a page that reaches that listing). A page in the main story,
//! a page outside the franchise, and a page linked to nothing else have
//! none, but a side story whose parents the main story leaves out is
//! drawn on its earliest parent's chain (`franchise_walk_run.rs`).
//! Music videos are no titles of a chain, the page aside.

use std::collections::BTreeSet;

use super::rules::listed;
use super::walking::Run;
use super::Lookup;

/// A page's chain, by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChainIds {
    /// The chain's own titles, the page among them (a side story drawn
    /// on its parent's chain is not).
    pub(crate) own: BTreeSet<String>,
    /// The main-story titles the chain is linked to.
    pub(crate) seams: BTreeSet<String>,
}

/// Whether `a` and `b` are linked by prequel or sequel, either listing
/// naming the other.
fn linked(look: &Lookup<'_>, a: &str, b: &str) -> bool {
    let names =
        |of: &str, other: &str| listed(look, of, &["prequel", "sequel"]).any(|r| r.id == other);
    names(a, b) || names(b, a)
}

/// The chain of `anchor` among the `franchise`'s entries outside `main`.
pub(crate) fn chain(
    anchor: &str,
    franchise: &BTreeSet<String>,
    main: &Run,
    look: &Lookup<'_>,
) -> Option<ChainIds> {
    if main.members.contains_key(anchor) || !franchise.contains(anchor) {
        return None;
    }
    let music = |m: &str| {
        look.entry(m, &main.consulted)
            .is_some_and(|e| e.subtype.as_deref() == Some("music"))
    };
    let outside: Vec<&String> = franchise
        .iter()
        .filter(|m| !main.members.contains_key(*m))
        .filter(|m| m.as_str() == anchor || !music(m))
        .collect();
    let mut own = BTreeSet::from([anchor.to_string()]);
    let mut todo = vec![anchor.to_string()];
    while let Some(at) = todo.pop() {
        for m in &outside {
            if !own.contains(*m) && linked(look, &at, m) {
                own.insert((*m).clone());
                todo.push((*m).clone());
            }
        }
    }
    let seams: BTreeSet<String> = main
        .members
        .keys()
        .filter(|m| own.iter().any(|o| linked(look, o, m)))
        .cloned()
        .collect();
    if own.len() < 2 && seams.is_empty() {
        return None;
    }
    Some(ChainIds { own, seams })
}
