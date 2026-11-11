//! What counts as a series: a TV series, OVA or ONA of more than one
//! episode, or whose episode count Kitsu does not give (an entry still
//! airing or only announced, which Kitsu records with its count once
//! it is out, as one episode when it is a one-off). Films, specials,
//! music videos, one-episode OVAs and ONAs, and an entry of no known
//! format are one-offs. A count of zero is no count.

use crate::meta::kitsu::FranchiseEntry;

/// The formats a series comes in.
const SERIES_FORMATS: [&str; 3] = ["TV", "OVA", "ONA"];

/// Whether `e` is a series rather than a one-off.
#[must_use]
pub(crate) fn is_series(e: &FranchiseEntry) -> bool {
    let format = e
        .subtype
        .as_deref()
        .is_some_and(|s| SERIES_FORMATS.contains(&s));
    format && !matches!(e.episodes, Some(1))
}
