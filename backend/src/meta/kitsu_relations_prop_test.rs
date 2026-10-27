//! Property coverage for Kitsu's relationships page: which relations it
//! reads, and when it is no answer.
//!
//! Every strategy function returns a `BoxedStrategy`. Composed without
//! boxing, the nested combinator types grow into symbol names tens of
//! thousands of characters long, and the Windows test build's debug info
//! hits the linker's PDB limit (LNK4319). Box each new strategy too.

use proptest::prelude::*;
use serde_json::{json, Value};

use super::parse_relationships_response;

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

/// An included anime: its id, its count, and whether it carries its
/// attributes at all.
fn node() -> BoxedStrategy<Node> {
    (
        0u8..5,
        proptest::option::of(0u32..30),
        prop::bool::weighted(0.9),
    )
        .boxed()
}

/// An included anime: its id, its count, whether it has attributes.
type Node = (u8, Option<u32>, bool);

/// A page: its relations' destinations, its included anime, its count.
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
        .map(|(id, count, attrs)| {
            if *attrs {
                json!({ "type": "anime", "id": id.to_string(), "attributes": { "subtype": "TV", "status": "finished", "episodeCount": count } })
            } else {
                json!({ "type": "anime", "id": id.to_string() })
            }
        })
        .collect();
    let mut doc = json!({ "data": edges, "included": included });
    if let Some(count) = count {
        doc["meta"] = json!({ "count": count });
    }
    serde_json::to_vec(&doc).expect("serialise")
}

/// The relations a page is specified to read as, `(id, count)` in order,
/// or `None` where it is no answer: a relation with no destination, or
/// one without its data key, or one whose destination is not included;
/// or an included entry without attributes. A null destination is no
/// relation.
fn model(dests: &[Dest], nodes: &[Node]) -> Option<Vec<(String, Option<u32>)>> {
    if nodes.iter().any(|(_, _, attrs)| !attrs) {
        return None;
    }
    let mut out = Vec::new();
    for d in dests {
        match d {
            Dest::Absent | Dest::NoData => return None,
            Dest::Null => {}
            Dest::To(id) => {
                let (_, count, _) = nodes.iter().find(|(n, _, _)| n == id)?;
                out.push((id.to_string(), *count));
            }
        }
    }
    Some(out)
}

proptest! {
    /// The relations are the included destinations, in order, and a page
    /// that leaves out what it is asked for is no answer.
    #[test]
    fn the_relations_are_the_included_destinations_or_no_answer((dests, nodes, count) in page()) {
        let parsed = parse_relationships_response(&body(&dests, &nodes, count));
        match model(&dests, &nodes) {
            None => prop_assert!(parsed.is_err(), "a page missing what was asked for read as an answer"),
            Some(want) => {
                let got = parsed.expect("an answer");
                let read: Vec<(String, Option<u32>)> =
                    got.entries.iter().map(|r| (r.id.clone(), r.episode_count)).collect();
                prop_assert_eq!(read, want);
                prop_assert_eq!(got.complete, count == Some(dests.len()));
            }
        }
    }

    /// Any body reads as a page or an error, never a panic.
    #[test]
    fn the_relationships_parser_answers_any_body(body in prop::collection::vec(any::<u8>(), 0..64)) {
        let _ = parse_relationships_response(&body);
    }
}
