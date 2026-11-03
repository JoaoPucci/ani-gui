//! How the walk builds a run (rules 1–3 of the proposal's section 4).

use std::collections::{BTreeMap, BTreeSet};

use super::Lookup;

/// A decided run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    /// Each member and the entry before it on its path to the root.
    pub(crate) members: BTreeMap<String, Option<String>>,
    /// The entries rules 2 and 3 left out.
    pub(crate) left_out: BTreeSet<String>,
    /// Every listing the walk read.
    pub(crate) consulted: BTreeSet<String>,
    /// Whether the walk ran to its end within its bounds.
    pub(crate) complete: bool,
    /// Whether the run is the anchor's parent's, the anchor a side story.
    pub(crate) side_anchor: bool,
}

/// Whether `a` and `b` list each other, either way, as alternatives.
pub(crate) fn alt(look: &Lookup<'_>, a: &str, b: &str) -> bool {
    let _ = (look.known, look.details, a, b);
    false
}

/// The anchor's run.
///
/// # Errors
/// The listings to read before the walk can go on.
pub(crate) fn run(anchor: &str, look: &Lookup<'_>) -> Result<Run, Vec<String>> {
    let _ = alt(look, anchor, anchor);
    Ok(Run {
        members: BTreeMap::from([(anchor.to_string(), None)]),
        left_out: BTreeSet::new(),
        consulted: BTreeSet::new(),
        complete: false,
        side_anchor: false,
    })
}
