//! Which tips this install is offered, and which of them it has not
//! seen. Pure: the registry, the install's record, the running release
//! and which flags are on go in; nothing is read or written here.
//!
//! The rules:
//!
//! - **A window per tip.** A tip is offered from the release that
//!   shipped it until [`OFFERED_MINORS`] minor releases later, or until
//!   the end it declares: shipped in 0.19.0, it is offered in 0.19.x
//!   and 0.20.x and gone from 0.21.0.
//! - **A flagged feature counts from its flag.** Its tip is offered
//!   only while the flag is on, and its window counts from the release
//!   of the first read of the tips with the flag on
//!   ([`Record::flag_on_since`]; the renderer reads them at every
//!   launch), with the declared end as a hard limit.
//!   Counted from the release that shipped the code, a flag turned on
//!   late would leave the tip no window at all.
//! - **Seen is forever.** A tip seen is never pending again.
//! - **A first-ever install has nothing new to be shown.** The first
//!   read of the tips after a first-ever launch
//!   ([`Record::baseline_pending`]) marks seen every tip offered then, so later releases still introduce
//!   their own features. A flagged tip whose flag is off then is left
//!   alone: when the flag comes on, the feature is new to that install
//!   too.
//! - **An on-demand tip is never offered** ([`TipDecl::on_demand`]).

use super::decl::{TipDecl, OFFERED_MINORS};
use super::release::Release;
use super::state::Record;

/// The release a tip's window opens at for this install, and the first
/// release past it; `None` when the tip is not offered to this install
/// at all (on demand, its flag off, or its flag first on at or after
/// its declared end).
#[must_use]
pub fn window(
    tip: &TipDecl,
    record: &Record,
    current: Release,
    flag_on: impl Fn(&str) -> bool,
) -> Option<(Release, Release)> {
    if tip.on_demand {
        return None;
    }
    let Some(flag) = &tip.flag else {
        let end = tip.until.unwrap_or(tip.since.plus_minors(OFFERED_MINORS));
        return Some((tip.since, end));
    };
    if !flag_on(flag) {
        return None;
    }
    // A flag on now with no launch recorded is on from this release:
    // the record catches up on the next sync.
    let on_since = record.flag_on_since(flag).unwrap_or(current);
    let opens = tip.since.max(on_since);
    let counted = opens.plus_minors(OFFERED_MINORS);
    let end = tip.until.map_or(counted, |hard| hard.min(counted));
    // A flag first on at or after the hard end leaves no window.
    (opens < end).then_some((opens, end))
}

/// Whether `current` is inside the tip's window for this install.
#[must_use]
pub fn offered(
    tip: &TipDecl,
    record: &Record,
    current: Release,
    flag_on: impl Fn(&str) -> bool,
) -> bool {
    window(tip, record, current, flag_on)
        .is_some_and(|(opens, end)| opens <= current && current < end)
}

/// Offered and not yet seen.
#[must_use]
pub fn pending(
    tip: &TipDecl,
    record: &Record,
    current: Release,
    flag_on: impl Fn(&str) -> bool,
) -> bool {
    !record.seen.contains(&tip.id) && offered(tip, record, current, flag_on)
}

/// The record after this launch has been taken into it: each flag a
/// tip names that is on, and that the record has no launch for, is on
/// from `current`; and a first-ever install's baseline marks seen
/// every tip offered now. Applying it twice changes nothing more.
#[must_use]
pub fn sync(
    tips: &[TipDecl],
    record: &Record,
    current: Release,
    flag_on: impl Fn(&str) -> bool,
) -> Record {
    let mut next = record.clone();
    for flag in tips.iter().filter_map(|t| t.flag.as_deref()) {
        if flag_on(flag) && next.flag_on_since(flag).is_none() {
            next.flag_on_since.insert(flag.to_owned(), current);
        }
    }
    if next.baseline_pending {
        let known: Vec<String> = tips
            .iter()
            .filter(|t| offered(t, &next, current, &flag_on))
            .map(|t| t.id.clone())
            .collect();
        next.seen.extend(known);
        next.baseline_pending = false;
    }
    next
}

#[cfg(test)]
#[path = "eligibility_test.rs"]
mod tests;

#[cfg(test)]
#[path = "eligibility_prop_test.rs"]
mod prop_tests;
