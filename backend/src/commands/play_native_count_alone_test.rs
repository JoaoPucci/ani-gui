//! Property coverage for the count-alone rule: an entry with a count
//! above one and no year takes a probed candidate only on its name;
//! every other entry is left exactly as it was.

use super::{count_stands_alone, unvouched_counted};
use crate::commands::play_native_title_marker::EntryTitles;
use crate::commands::play_native_wide_listing::UNFIT;
use crate::scraper::provider::BrowseHit;
use proptest::prelude::*;

const ENTRY: [&str; 2] = ["Show: Arc", "Shou Arc"];
const NEEDLE: &str = "show: arc";

/// Candidate titles with whether the entry's names vouch for them:
/// the title searched (as written, case aside), the entry under its
/// alias, and titles the entry does not name — another show, the
/// franchise's name alone, and a sequel of the entry's own.
const TITLES: [(&str, bool); 6] = [
    ("Show: Arc", true),
    ("SHOW: ARC", true),
    ("Shou Arc", true),
    ("Other", false),
    ("Show", false),
    ("Show: Arc 2", false),
];

/// One probed row: a title index, a count distance (or [`UNFIT`]),
/// and whether its year matched the entry's.
fn rows() -> impl Strategy<Value = Vec<(usize, u32, bool)>> {
    proptest::collection::vec(
        (
            0..TITLES.len(),
            prop_oneof![0u32..8, Just(UNFIT)],
            any::<bool>(),
        ),
        0..8,
    )
}

fn hits(rows: &[(usize, u32, bool)]) -> Vec<BrowseHit> {
    rows.iter()
        .enumerate()
        .map(|(i, (t, _, _))| BrowseHit {
            slug: format!("hit-{i}"),
            title: TITLES[*t].0.to_string(),
            kind: None,
            jname: None,
        })
        .collect()
}

proptest! {
    /// The count stands alone exactly for a count above one with no
    /// year: never with a year, whatever the count, never for a count
    /// of one, and never for a count of zero — a count Kitsu has not
    /// filled in, which the pick went on scoring as it always had.
    #[test]
    fn the_count_stands_alone_without_a_year_and_above_one(
        expected in 0u32..2000,
        year in proptest::option::of(1900u32..2100),
    ) {
        prop_assert_eq!(count_stands_alone(expected, year), expected > 1 && year.is_none());
    }

    /// A dated entry, whatever its count, is left exactly as it was,
    /// and nothing is reported refused.
    #[test]
    fn a_dated_entry_is_untouched(rows in rows(), expected in 1u32..30, year in 1900u32..2100) {
        let hits = hits(&rows);
        let mut probed: Vec<_> = hits.iter().zip(&rows).map(|(h, (_, d, c))| (h, Vec::new(), *d, *c)).collect();
        let before: Vec<u32> = probed.iter().map(|r| r.2).collect();
        let refused = unvouched_counted(&mut probed, expected, NEEDLE, EntryTitles::new(&ENTRY), Some(year));
        prop_assert!(!refused);
        prop_assert_eq!(probed.iter().map(|r| r.2).collect::<Vec<_>>(), before);
    }

    /// An entry one video long is left to the single-video rule: this
    /// one touches nothing and reports nothing.
    #[test]
    fn a_single_video_entry_is_untouched(rows in rows()) {
        let hits = hits(&rows);
        let mut probed: Vec<_> = hits.iter().zip(&rows).map(|(h, (_, d, c))| (h, Vec::new(), *d, *c)).collect();
        let before: Vec<u32> = probed.iter().map(|r| r.2).collect();
        let refused = unvouched_counted(&mut probed, 1, NEEDLE, EntryTitles::new(&ENTRY), None);
        prop_assert!(!refused);
        prop_assert_eq!(probed.iter().map(|r| r.2).collect::<Vec<_>>(), before);
    }

    /// For an entry with a count above one and no year, a row keeps
    /// its distance exactly when its name vouches for it or it was
    /// already out; every other row is scored out, keeping its place,
    /// listing and year; and the refusal is reported exactly when a row
    /// scored out here sat within the count tolerance.
    #[test]
    fn an_undated_counted_entry_keeps_only_a_named_candidate(rows in rows(), expected in 2u32..30) {
        let hits = hits(&rows);
        let mut probed: Vec<_> = hits.iter().zip(&rows).map(|(h, (_, d, c))| (h, Vec::new(), *d, *c)).collect();
        let refused = unvouched_counted(&mut probed, expected, NEEDLE, EntryTitles::new(&ENTRY), None);
        let tolerance = crate::commands::play_native::ep_count_threshold(expected);
        let mut want_refused = false;
        for ((row, (t, d, confirmed)), h) in probed.iter().zip(&rows).zip(&hits) {
            prop_assert_eq!(row.0.slug.as_str(), h.slug.as_str());
            prop_assert_eq!(row.3, *confirmed);
            // A year-confirmed row is vouched for too; with no year
            // the walk never confirms one, but the rule reads the flag.
            let vouched = TITLES[*t].1 || *confirmed;
            if vouched || *d == UNFIT {
                prop_assert_eq!(row.2, *d, "{} kept", TITLES[*t].0);
            } else {
                prop_assert_eq!(row.2, UNFIT, "{} scored out", TITLES[*t].0);
                want_refused |= *d <= tolerance;
            }
        }
        prop_assert_eq!(refused, want_refused);
    }
}

/// A count of zero is no count to stand alone: an undated entry Kitsu
/// counts at zero is scored as before.
#[test]
fn a_count_of_zero_does_not_stand_alone() {
    assert!(!count_stands_alone(0, None));
}
