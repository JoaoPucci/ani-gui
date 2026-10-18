//! Whether Kitsu's relations explain the rows past the count
//! ([`super`]).

use super::{EntryTail, Related};

/// Whether Kitsu's relations, read whole, name what the `past` rows
/// are — a finished OVA, ONA or special exactly that long, whatever
/// Kitsu calls its relation — and no sequel airing now, which the rows
/// could be the start of. A sequel only announced has aired nothing.
pub(super) fn explained(past: usize, tail: &EntryTail) -> bool {
    let finished = |r: &Related| r.status.as_deref() == Some("finished");
    let length = u32::try_from(past).ok();
    let extra = |r: &Related| {
        r.anime
            && finished(r)
            && matches!(r.subtype.as_deref(), Some("OVA" | "ONA" | "special"))
            && r.episode_count == length
    };
    let airing_sequel =
        |r: &Related| r.anime && r.role == "sequel" && r.status.as_deref() == Some("current");
    tail.related_complete
        && tail.related.iter().any(extra)
        && !tail.related.iter().any(airing_sequel)
}
