//! Property coverage for the pure parts of the airing batch, beside
//! `anilist_airing_batch.rs`: the partition of a page's nodes by key,
//! and the page parser.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use super::{parse_airing_batch_response, parse_batch, BatchPage};
use crate::meta::anilist_airing::AiringStatus;

/// A status told apart by its `aired`, so a test can see which node a
/// key kept.
fn status(aired: u32) -> AiringStatus {
    AiringStatus {
        aired: Some(aired),
        ..Default::default()
    }
}

/// Up to 40 nodes over a few keys, so repeats are common.
fn nodes() -> impl Strategy<Value = Vec<(u32, u32)>> {
    prop::collection::vec((0u32..8, any::<u32>()), 0..40)
}

proptest! {
    /// Every key lands in exactly one bucket. A key one node answers
    /// keeps that node's status. A key several nodes answer is in
    /// `several` once and never collapsed into `once`.
    #[test]
    fn each_key_lands_in_exactly_one_bucket(nodes in nodes()) {
        let mut count: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (key, aired) in &nodes {
            count.entry(*key).or_default().push(*aired);
        }
        let page = BatchPage {
            media: nodes.iter().map(|(k, a)| (*k, status(*a))).collect(),
            cut: false,
        };
        let (once, several) = page.by_key();
        let several_set: BTreeSet<u32> = several.iter().copied().collect();
        prop_assert_eq!(several_set.len(), several.len(), "a key in several twice");
        for (key, aired) in &count {
            match aired.as_slice() {
                [one] => {
                    prop_assert_eq!(once.get(key).cloned(), Some(status(*one)));
                    prop_assert!(!several_set.contains(key));
                }
                _ => {
                    prop_assert!(several_set.contains(key));
                    prop_assert!(!once.contains_key(key));
                }
            }
        }
        let answered: BTreeSet<u32> = once.keys().copied().chain(several.iter().copied()).collect();
        prop_assert_eq!(answered, count.keys().copied().collect::<BTreeSet<_>>());
    }

    /// The partition does not depend on the order the page lists its
    /// nodes in.
    #[test]
    fn the_partition_does_not_depend_on_the_page_s_order(
        nodes in nodes(),
        seed in any::<u64>(),
    ) {
        let mut shuffled = nodes.clone();
        // A deterministic shuffle from the seed.
        let mut state = seed | 1;
        for i in (1..shuffled.len()).rev() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let j = (state % (i as u64 + 1)) as usize;
            shuffled.swap(i, j);
        }
        let split = |n: &[(u32, u32)]| {
            let (once, several) = BatchPage {
                media: n.iter().map(|(k, a)| (*k, status(*a))).collect(),
                cut: false,
            }
            .by_key();
            let once: BTreeMap<u32, AiringStatus> = once.into_iter().collect();
            let several: BTreeSet<u32> = several.into_iter().collect();
            (once, several)
        };
        prop_assert_eq!(split(&nodes), split(&shuffled));
    }

    /// The parser keeps every node with a key, in page order, keyed by
    /// the id the batch asked by, and reads the page as cut exactly when
    /// AniList says so.
    #[test]
    fn the_parser_keeps_each_keyed_node_in_order_and_reads_the_cut(
        nodes in prop::collection::vec(
            (proptest::option::of(0u32..1000), proptest::option::of(0u32..1000), 0u32..60),
            0..20,
        ),
        cut in proptest::option::of(any::<bool>()),
        by_mal in any::<bool>(),
    ) {
        let rendered: Vec<String> = nodes
            .iter()
            .map(|(id, mal, eps)| {
                let id = id.map_or("null".into(), |v| v.to_string());
                let mal = mal.map_or("null".into(), |v| v.to_string());
                format!(
                    r#"{{"id":{id},"idMal":{mal},"status":"FINISHED","episodes":{eps},"nextAiringEpisode":null,"airingSchedule":{{"nodes":[]}}}}"#
                )
            })
            .collect();
        let info = match cut {
            Some(c) => format!(r#""pageInfo":{{"hasNextPage":{c}}},"#),
            None => String::new(),
        };
        let body = format!(r#"{{"data":{{"Page":{{{info}"media":[{}]}}}}}}"#, rendered.join(","));
        let page = parse_batch(body.as_bytes(), by_mal).expect("a well-formed page");
        prop_assert_eq!(page.cut, cut.unwrap_or(false));
        let want: Vec<(u32, Option<u32>)> = nodes
            .iter()
            .filter_map(|(id, mal, eps)| if by_mal { *mal } else { *id }.map(|k| (k, Some(*eps))))
            .collect();
        let got: Vec<(u32, Option<u32>)> = page.media.iter().map(|(k, s)| (*k, s.aired)).collect();
        prop_assert_eq!(got, want);
    }

    /// Whatever bytes arrive, the parsers answer or refuse; they never
    /// panic, and the id-keyed public parser answers exactly what the
    /// page parser does, keyed by id.
    #[test]
    fn the_parser_never_panics(body in prop::collection::vec(any::<u8>(), 0..200), by_mal in any::<bool>()) {
        let _ = parse_batch(&body, by_mal);
        let public = parse_airing_batch_response(&body).ok();
        let page = parse_batch(&body, false)
            .ok()
            .map(|p| p.media.into_iter().collect::<std::collections::HashMap<_, _>>());
        prop_assert_eq!(public, page);
    }

    /// For a well-formed page, the public parser keeps every node with
    /// an id: the page parser's nodes, keyed by id.
    #[test]
    fn the_public_parser_is_the_page_parser_keyed_by_id(
        ids in prop::collection::vec(0u32..50, 0..20),
    ) {
        let nodes: Vec<String> = ids
            .iter()
            .map(|id| format!(r#"{{"id":{id},"status":"FINISHED","episodes":{id},"nextAiringEpisode":null}}"#))
            .collect();
        let body = format!(r#"{{"data":{{"Page":{{"media":[{}]}}}}}}"#, nodes.join(","));
        let public = parse_airing_batch_response(body.as_bytes()).expect("well formed");
        let page: std::collections::HashMap<_, _> =
            parse_batch(body.as_bytes(), false).expect("well formed").media.into_iter().collect();
        prop_assert_eq!(public, page);
    }
}

#[test]
fn an_empty_page_answers_nothing_and_needs_no_single_lookup() {
    let (once, several) = BatchPage {
        media: Vec::new(),
        cut: false,
    }
    .by_key();
    assert!(once.is_empty());
    assert!(several.is_empty());
}
