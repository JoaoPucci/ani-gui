//! Properties of the availability stamp for a listing whose rows past
//! Kitsu's count no reading reached: a count is written only where it
//! is a count, and an unread one never displaces a counted row.
//!
//! Each strategy returns a `BoxedStrategy`: the unboxed combinator
//! types run deep enough that the Windows linker's debug-symbol
//! records overflow on them.

use super::*;
use crate::commands::play_native_numbering::{extra_episode_tags, kitsu_episode_cap};
use crate::scraper::provider::{EpisodeRef, ProviderId};
use proptest::prelude::*;

/// A listing numbered by position, some rows tagged as recaps.
fn listing() -> BoxedStrategy<Vec<EpisodeRef>> {
    proptest::collection::vec(any::<bool>(), 0..30)
        .prop_map(|recaps| {
            recaps
                .into_iter()
                .zip(1u32..)
                .map(|(recap, n)| EpisodeRef {
                    id: u64::from(n),
                    number: n,
                    number2: recap.then(|| format!("{n}.5")),
                    title: None,
                    former_tag: None,
                })
                .collect()
        })
        .boxed()
}

fn pick(
    episodes: Vec<EpisodeRef>,
    unread_past: Option<u32>,
) -> crate::commands::play_native::PickedShow {
    crate::commands::play_native::PickedShow {
        hit: crate::scraper::provider::BrowseHit {
            slug: "show-1".into(),
            title: "Show".into(),
            kind: None,
            jname: None,
        },
        episodes,
        stitched: false,
        versions: Vec::new(),
        unread_past,
    }
}

proptest! {
    /// The stamp against its model: a count only from a sub cap whose
    /// rows past Kitsu's count were read; a counted row that stands
    /// is left as it is exactly when the count is unread; the row
    /// with no count otherwise.
    #[test]
    fn a_count_is_written_only_where_it_is_one(
        cap in proptest::option::of(any::<u32>()),
        dub in any::<bool>(),
        count_unread in any::<bool>(),
        standing_counted in any::<bool>(),
    ) {
        let got = served_stamp(cap, dub, count_unread, standing_counted);
        let want = match (count_unread, standing_counted, cap) {
            (true, true, _) => ServedStamp::Standing,
            (false, _, Some(c)) if !dub => ServedStamp::Count(c),
            _ => ServedStamp::NoCount,
        };
        prop_assert_eq!(got, want);
        // Whatever stands, an unread count writes none.
        if count_unread {
            prop_assert!(!matches!(got, ServedStamp::Count(_)));
        }
        // What stands weighs nothing for a count that was read.
        if !count_unread {
            prop_assert_eq!(got, served_stamp(cap, dub, false, !standing_counted));
        }
    }

    /// The probe's counts are the listing's, unless the rows past
    /// Kitsu's count were not read, which gives neither.
    #[test]
    fn a_probe_counts_only_a_read_listing(
        rows in listing(),
        unread_past in proptest::option::of(0u32..40),
    ) {
        let got = probe_counts(&pick(rows.clone(), unread_past));
        if unread_past.is_some() {
            prop_assert_eq!(got, (None, Vec::new()));
        } else {
            prop_assert_eq!(got, (kitsu_episode_cap(&rows), extra_episode_tags(&rows)));
        }
    }

    /// A row stands for an unread count exactly when it is positive,
    /// counted, exact, and names the provider that served; a row naming
    /// no provider stands for none.
    #[test]
    fn a_count_stands_only_for_its_own_provider(
        available in any::<bool>(),
        count in proptest::option::of(any::<u32>()),
        approximate in any::<bool>(),
        named in proptest::option::of(provider()),
        served in proptest::option::of(provider()),
    ) {
        let stood = crate::commands::availability::AvailabilityResponse {
            available,
            episode_count: count,
            extra_episodes: Vec::new(),
            episode_count_approximate: approximate,
            gate_refused: false,
            unsettled: None,
            provider: named,
            versions: Vec::new(),
        };
        let want = available
            && count.is_some()
            && !approximate
            && matches!((named, served), (Some(a), Some(b)) if a == b);
        prop_assert_eq!(counts_for(&stood, served), want);
        prop_assert!(!counts_for(&stood, None));
    }
}

fn provider() -> BoxedStrategy<ProviderId> {
    prop_oneof![Just(ProviderId::TestA), Just(ProviderId::TestB)].boxed()
}
