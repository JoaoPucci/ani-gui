//! Properties of the walk over AniList's pages, against a stand-in for
//! AniList that answers each id with some number of media and cuts a
//! page holding more than [`PAGE_CAP`]: every id is answered by exactly
//! one whole page or handed over as cut alone, in the order given; no
//! chunk asks for more than a page holds; and an error ends the walk.
//!
//! Every strategy function returns a `BoxedStrategy`. Composed without
//! boxing, the nested combinator types grow into symbol names tens of
//! thousands of characters long, and the Windows test build's debug info
//! hits the linker's PDB limit (LNK4319). Box each new strategy too.

use std::cell::RefCell;

use super::*;
use crate::error::AniError;
use proptest::prelude::*;

/// How many media answer each id: mostly none to three, sometimes more
/// than a page holds by itself.
fn media_per_id() -> BoxedStrategy<Vec<u32>> {
    prop::collection::vec(
        prop_oneof![12 => 0u32..=3, 1 => 51u32..=80, 1 => 20u32..=50],
        0..=160,
    )
    .boxed()
}

fn block_on<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(f)
}

/// The walk over ids `0..counts.len()`, AniList answering id `i` with
/// `counts[i]` media; every chunk asked, the whole pages, and the ids cut
/// alone.
fn walk(counts: &[u32]) -> (Vec<Vec<u32>>, Vec<Vec<u32>>, Vec<u32>) {
    let ids: Vec<u32> = (0..u32::try_from(counts.len()).unwrap_or(0)).collect();
    let asked = RefCell::new(Vec::new());
    let alone = RefCell::new(Vec::new());
    let pages = block_on(whole_pages(
        &ids,
        |chunk| {
            asked.borrow_mut().push(chunk.to_vec());
            let held: u32 = chunk.iter().map(|i| counts[*i as usize]).sum();
            let page = if held as usize > PAGE_CAP {
                Page::Cut
            } else {
                Page::Whole(chunk.to_vec())
            };
            async move { Ok(page) }
        },
        |one| {
            alone.borrow_mut().push(one);
            Ok(())
        },
    ))
    .expect("no error");
    (asked.into_inner(), pages, alone.into_inner())
}

proptest! {
    /// Every id is answered by exactly one whole page or handed over as
    /// cut alone, and taken in order the two together are the ids given.
    #[test]
    fn every_id_is_answered_once_in_order(counts in media_per_id()) {
        let (_, pages, alone) = walk(&counts);
        let mut answered: Vec<u32> = pages.iter().flatten().copied().collect();
        answered.extend(&alone);
        answered.sort_unstable();
        let ids: Vec<u32> = (0..u32::try_from(counts.len()).unwrap_or(0)).collect();
        prop_assert_eq!(&answered, &ids);
        // The whole pages come back in the order their ids were given.
        let in_pages: Vec<u32> = pages.iter().flatten().copied().collect();
        prop_assert!(in_pages.windows(2).all(|w| w[0] < w[1]));
        // An id is cut alone exactly when it fills more than a page.
        for i in &alone {
            prop_assert!(counts[*i as usize] as usize > PAGE_CAP);
        }
        for i in &in_pages {
            prop_assert!(counts[*i as usize] as usize <= PAGE_CAP);
        }
    }

    /// No chunk asks for more ids than a page holds, every whole page
    /// holds what AniList answered for its ids, and a chunk is asked
    /// again only as one of the two halves of a cut one.
    #[test]
    fn a_chunk_is_a_page_or_half_a_cut_one(counts in media_per_id()) {
        let (asked, pages, _) = walk(&counts);
        prop_assert!(asked.iter().all(|c| !c.is_empty() && c.len() <= PAGE_CAP));
        for page in &pages {
            let held: u32 = page.iter().map(|i| counts[*i as usize]).sum();
            prop_assert!(held as usize <= PAGE_CAP);
        }
        // Each id is asked once in the first chunks, and once more in
        // each cut chunk it was part of: at most once per halving.
        let first_chunks = counts.len().div_ceil(PAGE_CAP);
        let cuts = asked.len() - pages.len();
        prop_assert!(asked.len() <= first_chunks + 2 * cuts);
        prop_assert!(
            asked.iter().take(first_chunks.min(1)).all(|c| c.first() == Some(&0)),
            "the walk starts at the first id"
        );
    }

    /// An error asking ends the walk with it: nothing is asked after.
    #[test]
    fn an_error_ends_the_walk(len in 1usize..=160, fail_at in 0usize..6) {
        let ids: Vec<u32> = (0..u32::try_from(len).unwrap_or(0)).collect();
        let asked = RefCell::new(0usize);
        let got = block_on(whole_pages(
            &ids,
            |_| {
                let n = {
                    let mut asked = asked.borrow_mut();
                    *asked += 1;
                    *asked
                };
                async move {
                    if n > fail_at {
                        Err(AniError::Network)
                    } else {
                        Ok(Page::Whole(()))
                    }
                }
            },
            |_| Ok(()),
        ));
        let chunks = len.div_ceil(PAGE_CAP);
        if fail_at < chunks {
            prop_assert!(matches!(got, Err(AniError::Network)));
            prop_assert_eq!(asked.into_inner(), fail_at + 1);
        } else {
            prop_assert_eq!(got.map(|p| p.len()).ok(), Some(chunks));
        }
    }

    /// An error handing over an id cut alone ends the walk with it.
    #[test]
    fn an_error_for_an_id_cut_alone_ends_the_walk(len in 1usize..=60) {
        let ids: Vec<u32> = (0..u32::try_from(len).unwrap_or(0)).collect();
        let asked = RefCell::new(0usize);
        let got: Result<Vec<()>> = block_on(whole_pages(
            &ids,
            |chunk| {
                *asked.borrow_mut() += 1;
                // Id 0 fills more than a page by itself.
                let cut = chunk.contains(&0);
                async move { Ok(if cut { Page::Cut } else { Page::Whole(()) }) }
            },
            |one| Err(AniError::ParseFailed { detail: format!("{one}") }),
        ));
        let cut_zero = matches!(&got, Err(AniError::ParseFailed { detail }) if detail == "0");
        prop_assert!(cut_zero, "{:?}", got);
        // Halving down to id 0 alone, and nothing asked after it.
        let halvings = usize::try_from(len.min(PAGE_CAP).ilog2()).unwrap_or(0);
        prop_assert!(asked.into_inner() <= halvings + 2);
    }
}
