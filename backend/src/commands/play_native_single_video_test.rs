//! Property coverage for the single-video filter: for an entry one
//! video long, a probed candidate survives only when its name or its
//! year vouches for it.

use super::unvouched_single_videos;
use crate::commands::play_native_title_marker::EntryTitles;
use crate::commands::play_native_wide_listing::UNFIT;
use crate::scraper::provider::BrowseHit;
use proptest::prelude::*;

const ENTRY: [&str; 2] = ["Show 2: The Movie", "Show Gekijouban"];
const NEEDLE: &str = "show 2: the movie";

/// Candidate titles with whether the entry's names vouch for them: the
/// title searched (as written, case aside), the same title with its
/// season written otherwise, the entry under its alias, and titles the
/// entry does not name — another show, the franchise's name alone, the
/// season the film is named for, and a numbered film that reads as
/// another one.
const TITLES: [(&str, bool); 8] = [
    ("Show 2: The Movie", true),
    ("SHOW 2: THE MOVIE", true),
    ("Show Second Season: The Movie", true),
    ("Show Gekijouban", true),
    ("Other Film", false),
    ("Show", false),
    ("Show 2", false),
    ("Show 2: The Movie 2", false),
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
    /// An entry whose count is not one is left exactly as it was, and
    /// nothing is reported refused.
    #[test]
    fn an_entry_longer_than_one_video_is_untouched(rows in rows(), expected in 2u32..30) {
        let hits = hits(&rows);
        let mut probed: Vec<_> = hits.iter().zip(&rows).map(|(h, (_, d, c))| (h, Vec::new(), *d, *c)).collect();
        let before: Vec<u32> = probed.iter().map(|r| r.2).collect();
        let refused = unvouched_single_videos(&mut probed, expected, NEEDLE, EntryTitles::new(&ENTRY));
        prop_assert!(!refused);
        prop_assert_eq!(probed.iter().map(|r| r.2).collect::<Vec<_>>(), before);
    }

    /// For an entry one video long, a row keeps its distance exactly
    /// when its name or its year vouches for it or it was already out;
    /// every other row is scored out, keeping its place, listing and
    /// year; and the refusal is reported exactly when a row scored out
    /// here sat within the count tolerance.
    #[test]
    fn a_single_video_survives_only_on_its_name_or_its_year(rows in rows()) {
        let hits = hits(&rows);
        let mut probed: Vec<_> = hits.iter().zip(&rows).map(|(h, (_, d, c))| (h, Vec::new(), *d, *c)).collect();
        let refused = unvouched_single_videos(&mut probed, 1, NEEDLE, EntryTitles::new(&ENTRY));
        let mut want_refused = false;
        for ((row, (t, d, confirmed)), h) in probed.iter().zip(&rows).zip(&hits) {
            prop_assert_eq!(row.0.slug.as_str(), h.slug.as_str());
            prop_assert_eq!(row.3, *confirmed);
            let vouched = TITLES[*t].1 || *confirmed;
            if vouched || *d == UNFIT {
                prop_assert_eq!(row.2, *d, "{} kept", TITLES[*t].0);
            } else {
                prop_assert_eq!(row.2, UNFIT, "{} scored out", TITLES[*t].0);
                want_refused |= *d <= 3;
            }
        }
        prop_assert_eq!(refused, want_refused);
    }
}
