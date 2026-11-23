use std::collections::HashSet;
use std::marker::PhantomData;

use super::{Ask, Purpose};

/// Every purpose. [`exhaustive`] fails to compile when a variant is
/// added and not listed here, so the checks below cover each one.
const ALL: [Purpose; 14] = [
    Purpose::Trending,
    Purpose::Banner,
    Purpose::Stills,
    Purpose::Airing,
    Purpose::TrackerIds,
    Purpose::WatchLaterIds,
    Purpose::TrackerMap,
    Purpose::Account,
    Purpose::UserList,
    Purpose::ListWrite,
    Purpose::ListDelete,
    Purpose::ListEntry,
    Purpose::TrackedMedia,
    Purpose::MediaEvidence,
];

#[allow(dead_code)]
fn exhaustive(p: Purpose) {
    match p {
        Purpose::Trending
        | Purpose::Banner
        | Purpose::Stills
        | Purpose::Airing
        | Purpose::TrackerIds
        | Purpose::WatchLaterIds
        | Purpose::TrackerMap
        | Purpose::Account
        | Purpose::UserList
        | Purpose::ListWrite
        | Purpose::ListDelete
        | Purpose::ListEntry
        | Purpose::TrackedMedia
        | Purpose::MediaEvidence => {}
    }
}

#[test]
fn every_purpose_has_its_own_stable_label() {
    // A label two purposes shared would make the log say one feature
    // spent the other's requests.
    let mut seen = HashSet::new();
    for p in ALL {
        let name = p.name();
        assert!(seen.insert(name), "{name} labels two purposes");
        assert!(
            !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "{name} is not a snake_case label"
        );
    }
}

#[test]
fn no_purpose_is_a_catch_all() {
    // A label that names no feature is the fallback a new caller would
    // reach for, and the log would stop saying who asked.
    for p in ALL {
        let name = p.name();
        for vague in ["unknown", "other", "default", "misc", "none", "generic"] {
            assert_ne!(name, vague, "{p:?} is a catch-all");
        }
    }
}

// Whether a type has a default, asked without naming one: the probe's
// `HasDefault` method is found first, by value, when the type has one;
// otherwise method lookup goes on to `&Probe`'s `NoDefault`.
struct Probe<T>(PhantomData<T>);
trait HasDefault {
    fn has_default(&self) -> bool {
        true
    }
}
impl<T: Default> HasDefault for Probe<T> {}
trait NoDefault {
    fn has_default(&self) -> bool {
        false
    }
}
impl<T> NoDefault for &Probe<T> {}

// The borrow is what lets lookup fall through to `NoDefault`; where the
// type has a default it is redundant, which is what clippy sees.
#[allow(clippy::needless_borrow)]
#[test]
fn a_caller_cannot_leave_the_purpose_out() {
    // Every gate send takes an `Ask`, and neither it nor its purpose
    // has a default: there is no `Ask::default()` a caller could pass
    // to send unlabelled.
    assert!(!(&Probe::<Purpose>(PhantomData)).has_default());
    assert!(!(&Probe::<Ask>(PhantomData)).has_default());
    // The probe itself tells the two apart.
    assert!((&Probe::<u32>(PhantomData)).has_default());
}

#[test]
fn an_ask_counts_the_shows_it_carries() {
    assert_eq!(Ask::one(Purpose::Banner).shows, Some(1));
    assert_eq!(Ask::shows(Purpose::Stills, 29).shows, Some(29));
    assert_eq!(Ask::unkeyed(Purpose::Trending).shows, None);
    assert_eq!(Ask::unkeyed(Purpose::Trending).purpose, Purpose::Trending);
}
