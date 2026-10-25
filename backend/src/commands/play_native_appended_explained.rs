//! Whether Kitsu's relations explain the rows past the count
//! ([`super`]).

use super::{EntryTail, Related};

/// Whether Kitsu's relations, read whole, name what the `past` rows
/// are — a finished OVA, ONA or special exactly that long, whatever
/// Kitsu calls its relation — and no entry airing now, which the rows
/// could be the start of, whatever Kitsu calls the relation and
/// whatever its format: a non-TV entry's page is never read, so
/// nothing but its airing tells its cour from extras. A music video
/// is no cour, and an entry only announced has aired nothing.
pub fn explained(past: usize, tail: &EntryTail) -> bool {
    let finished = |r: &Related| r.status.as_deref() == Some("finished");
    let length = u32::try_from(past).ok();
    let extra = |r: &Related| {
        r.anime
            && finished(r)
            && matches!(r.subtype.as_deref(), Some("OVA" | "ONA" | "special"))
            && r.episode_count == length
    };
    let airing = |r: &Related| {
        r.anime && r.subtype.as_deref() != Some("music") && r.status.as_deref() == Some("current")
    };
    tail.related_complete && tail.related.iter().any(extra) && !tail.related.iter().any(airing)
}
