//! Properties of the batched streaming-listing lookup ([`resolve`])
//! against an in-process stand-in for AniList ([`FakeAniList`]): every
//! show reads what its own single lookup would read; a show asked in a
//! page that failed reads the failure, never an empty listing; and a
//! set of shows no two media share costs one request per id space and
//! page of fifty.
//!
//! Every strategy function returns a `BoxedStrategy` (see
//! `anilist_pages_prop_test.rs` for why).

use std::cell::RefCell;

use super::*;
use crate::meta::anilist_pages::by_key;
use crate::meta::anilist_streaming_eps_fake::{Asked, FakeAniList, FakeMedia};
use proptest::prelude::*;

/// Media with AniList ids `1..=n`, each recording a MAL id drawn from a
/// small range so that several media sometimes record one.
fn fake() -> BoxedStrategy<FakeAniList> {
    prop::collection::vec(prop::option::weighted(0.8, 1u32..40), 0..70)
        .prop_map(|mals| FakeAniList {
            media: mals
                .into_iter()
                .zip(1u32..)
                .map(|(mal, id)| FakeMedia {
                    id,
                    mal,
                    listed: id % 3,
                })
                .collect(),
        })
        .boxed()
}

/// Shows as Kitsu maps them: ids drawn past the stand-in's range too, so
/// that some name media AniList does not have.
fn wants() -> BoxedStrategy<Vec<Want>> {
    prop::collection::vec(
        (
            prop::option::weighted(0.7, 1u32..50),
            prop::option::weighted(0.6, 1u32..90),
        )
            .prop_map(|(mal, anilist)| Want { mal, anilist }),
        0..80,
    )
    .boxed()
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(f)
}

/// The stand-in's page for `ids` in `space`, as the real client reads it.
fn page_of(
    fake: &FakeAniList,
    space: Space,
    ids: &[u32],
) -> Page<(std::collections::HashMap<u32, StreamingListing>, Vec<u32>)> {
    let hit = |m: &FakeMedia| match space {
        Space::Mal => m.mal.filter(|x| ids.contains(x)),
        Space::AniList => Some(m.id).filter(|x| ids.contains(x)),
    };
    let found: Vec<(u32, StreamingListing)> = fake
        .media
        .iter()
        .filter_map(|m| hit(m).map(|k| (k, m.listing())))
        .collect();
    if found.len() > PAGE_CAP {
        Page::Cut
    } else {
        Page::Whole(by_key(found))
    }
}

/// Run [`resolve`] against `fake`, a page in `failing` failing; the
/// answers, and what was asked.
fn run(
    fake: &FakeAniList,
    wants: &[Want],
    failing: Option<Space>,
) -> (Vec<crate::error::Result<StreamingListing>>, Vec<Asked>) {
    let asked = RefCell::new(Vec::new());
    let got = block_on(resolve(
        wants,
        |space, ids: Vec<u32>| {
            asked.borrow_mut().push(match space {
                Space::Mal => Asked::PageByMal(ids.clone()),
                Space::AniList => Asked::PageById(ids.clone()),
            });
            let answer = if failing == Some(space) {
                Err(AniError::Upstream { status: 500 })
            } else {
                Ok(page_of(fake, space, &ids))
            };
            async move { answer }
        },
        |want: Want| {
            asked.borrow_mut().push(match want.mal {
                Some(mal) => Asked::ByMal(mal),
                None => Asked::ById(want.anilist.unwrap_or_default()),
            });
            let answer = Ok(fake.single_path(want.mal, want.anilist));
            async move { answer }
        },
    ));
    (got, asked.into_inner())
}

proptest! {
    #[test]
    fn every_show_reads_what_its_single_lookup_reads(fake in fake(), wants in wants()) {
        let (got, _) = run(&fake, &wants, None);
        prop_assert_eq!(got.len(), wants.len());
        for (w, answer) in wants.iter().zip(got) {
            prop_assert_eq!(answer.ok(), Some(fake.single_path(w.mal, w.anilist)), "{:?}", w);
        }
    }

    #[test]
    fn a_failed_page_fails_its_shows_and_only_them(
        fake in fake(),
        wants in wants(),
        by_mal in any::<bool>(),
    ) {
        let failing = if by_mal { Space::Mal } else { Space::AniList };
        let (got, _) = run(&fake, &wants, Some(failing));
        for (w, answer) in wants.iter().zip(got) {
            // A show is asked in the AniList-id page when it has no MAL
            // id, or AniList answered that it records none for it.
            let mal_listed = w.mal.is_some_and(|x| fake.media.iter().any(|m| m.mal == Some(x)));
            let in_failed = match failing {
                Space::Mal => w.mal.is_some(),
                Space::AniList => w.anilist.is_some() && !mal_listed,
            };
            if in_failed {
                prop_assert!(answer.is_err(), "{:?} read {:?}", w, answer);
            } else {
                prop_assert_eq!(answer.ok(), Some(fake.single_path(w.mal, w.anilist)), "{:?}", w);
            }
        }
    }

    #[test]
    fn shows_no_two_media_share_cost_one_request_per_space_and_page(
        fake in fake(),
        wants in wants(),
    ) {
        let mut seen = std::collections::HashSet::new();
        let unshared = fake.media.iter().all(|m| m.mal.is_none_or(|x| seen.insert(x)));
        prop_assume!(unshared);
        let (_, asked) = run(&fake, &wants, None);
        let pages = |n: usize| n.div_ceil(PAGE_CAP);
        let mals: std::collections::BTreeSet<u32> = wants.iter().filter_map(|w| w.mal).collect();
        let ids: std::collections::BTreeSet<u32> = wants.iter().filter_map(|w| w.anilist).collect();
        prop_assert!(
            asked.len() <= pages(mals.len()) + pages(ids.len()),
            "{} requests for {} MAL and {} AniList ids: {:?}", asked.len(), mals.len(), ids.len(), asked
        );
        prop_assert!(asked.iter().all(|a| matches!(a, Asked::PageByMal(_) | Asked::PageById(_))));
    }
}
