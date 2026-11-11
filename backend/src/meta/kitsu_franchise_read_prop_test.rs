//! Property coverage for the franchise relations page: which relations
//! it reads, when it is no answer, and that paging stops.
//!
//! Every strategy returns a `BoxedStrategy`, for the Windows test
//! build's PDB limit (see `kitsu_relations_prop_test.rs`).

use proptest::prelude::*;
use serde_json::{json, Value};

use super::{listing_complete, more_pages, parse_franchise_page, FRANCHISE_MAX_PAGES};

/// What a relation says of its destination.
#[derive(Debug, Clone)]
enum Dest {
    /// No `destination` at all.
    Absent,
    /// A destination without its `data` key.
    NoData,
    /// `data: null`: the relation points nowhere.
    Null,
    /// A pointer to the anime with this id.
    To(u8),
}

fn dest() -> BoxedStrategy<Dest> {
    prop_oneof![
        1 => Just(Dest::Absent),
        1 => Just(Dest::NoData),
        2 => Just(Dest::Null),
        8 => (0u8..5).prop_map(Dest::To),
    ]
    .boxed()
}

/// An included anime: its id, whether it carries attributes, whether
/// it carries its title, and whether it carries its start.
type Node = (u8, bool, bool, bool);

fn node() -> BoxedStrategy<Node> {
    (
        0u8..5,
        prop::bool::weighted(0.9),
        prop::bool::weighted(0.9),
        prop::bool::weighted(0.9),
    )
        .boxed()
}

type Page = (Vec<Dest>, Vec<Node>, Option<usize>);

fn page() -> BoxedStrategy<Page> {
    (
        prop::collection::vec(dest(), 0..5),
        prop::collection::vec(node(), 0..5),
        proptest::option::of(0usize..6),
    )
        .boxed()
}

fn body(dests: &[Dest], nodes: &[Node], count: Option<usize>) -> Vec<u8> {
    let edges: Vec<Value> = dests
        .iter()
        .map(|d| {
            let links = match d {
                Dest::Absent => json!({}),
                Dest::NoData => json!({ "destination": { "links": {} } }),
                Dest::Null => json!({ "destination": { "data": null } }),
                Dest::To(id) => {
                    json!({ "destination": { "data": { "type": "anime", "id": id.to_string() } } })
                }
            };
            json!({ "type": "mediaRelationships", "attributes": { "role": "sequel" }, "relationships": links })
        })
        .collect();
    let included: Vec<Value> = nodes
        .iter()
        .map(|(id, attrs, title, start)| {
            if !*attrs {
                return json!({ "type": "anime", "id": id.to_string() });
            }
            let mut a = json!({ "subtype": "TV", "status": "finished", "episodeCount": 3 });
            if *title {
                a["canonicalTitle"] = json!(format!("T{id}"));
            }
            if *start {
                a["startDate"] = json!("2020-01-01");
            }
            json!({ "type": "anime", "id": id.to_string(), "attributes": a })
        })
        .collect();
    let mut doc = json!({ "data": edges, "included": included });
    if let Some(count) = count {
        doc["meta"] = json!({ "count": count });
    }
    serde_json::to_vec(&doc).expect("serialise")
}

/// The ids a page is specified to read as, in order, or `None` where it
/// is no answer.
fn model(dests: &[Dest], nodes: &[Node]) -> Option<Vec<String>> {
    if nodes.iter().any(|(_, attrs, _, _)| !attrs) {
        return None;
    }
    let mut out = Vec::new();
    for d in dests {
        match d {
            Dest::Absent | Dest::NoData => return None,
            Dest::Null => {}
            Dest::To(id) => {
                let (_, _, title, start) = nodes.iter().find(|(n, _, _, _)| n == id)?;
                if !title || !start {
                    return None;
                }
                out.push(id.to_string());
            }
        }
    }
    Some(out)
}

proptest! {
    /// The relations are the included destinations, in order, and a
    /// page that leaves out what it is asked for is no answer.
    #[test]
    fn the_relations_are_the_included_destinations_or_no_answer((dests, nodes, count) in page()) {
        let parsed = parse_franchise_page(&body(&dests, &nodes, count));
        match model(&dests, &nodes) {
            None => prop_assert!(parsed.is_err(), "a page missing what was asked for read as an answer"),
            Some(want) => {
                let got = parsed.expect("an answer");
                let read: Vec<String> = got.relations.iter().map(|r| r.id.clone()).collect();
                prop_assert_eq!(read, want);
                prop_assert_eq!(got.read, dests.len());
                prop_assert_eq!(got.count, count);
            }
        }
    }

    /// Any body reads as a page or an error, never a panic.
    #[test]
    fn the_franchise_parser_answers_any_body(body in prop::collection::vec(any::<u8>(), 0..64)) {
        let _ = parse_franchise_page(&body);
    }

    /// Paging never passes the cap, never asks past an empty page, and
    /// a listing it stops short of Kitsu's count is incomplete.
    #[test]
    fn paging_is_bounded(pages in 0usize..10, rows in 0usize..200, last in 0usize..21, count in proptest::option::of(0usize..200)) {
        let more = more_pages(pages, rows, last, count);
        if pages >= FRANCHISE_MAX_PAGES || last == 0 {
            prop_assert!(!more);
        }
        if !more && count.is_some_and(|c| rows < c) {
            prop_assert!(!listing_complete(rows, count));
        }
    }
}
