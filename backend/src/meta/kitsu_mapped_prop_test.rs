//! Property coverage for Kitsu's mappings page: which entries it
//! reads, in what order, and when it holds them all.

use super::parse_mapped_anime_response;
use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;

/// An id from a pool small enough that pointers and inlined resources
/// name the same entries often.
fn pooled_id() -> impl Strategy<Value = String> {
    (0u8..6).prop_map(|i| i.to_string())
}

/// What an inlined resource reads as: its subtype, its count, and
/// whether its attributes read at all.
#[derive(Debug, Clone)]
struct Inlined {
    id: String,
    kind: &'static str,
    subtype: Option<&'static str>,
    episodes: Option<u32>,
    readable: bool,
}

fn inlined() -> impl Strategy<Value = Inlined> {
    (
        pooled_id(),
        prop::sample::select(&["anime", "anime", "manga"][..]),
        proptest::option::of(prop::sample::select(&["TV", "movie", "music", "Music"][..])),
        proptest::option::of(0u32..30),
        prop::bool::weighted(0.9),
    )
        .prop_map(|(id, kind, subtype, episodes, readable)| Inlined {
            id,
            kind,
            subtype,
            episodes,
            readable,
        })
}

fn inlined_json(r: &Inlined) -> Value {
    let attributes = if r.readable {
        json!({ "subtype": r.subtype, "episodeCount": r.episodes })
    } else {
        json!({ "episodeCount": "many" })
    };
    json!({ "id": r.id, "type": r.kind, "attributes": attributes })
}

/// What a mapping says of its item.
#[derive(Debug, Clone)]
enum Said {
    /// No relationships, or no item: the mapping is no answer.
    Nothing,
    /// An item given as null: the mapping maps to nothing.
    Null,
    /// A pointer to `(type, id)`.
    To(&'static str, String),
}

/// A mapping's pointer to its item, or the shapes with nothing on the
/// other end: no relationships, no item, a null item.
fn mapping() -> impl Strategy<Value = (Value, Said)> {
    prop_oneof![
        1 => Just((json!({}), Said::Nothing)),
        1 => Just((json!({ "relationships": null }), Said::Nothing)),
        1 => Just((json!({ "relationships": { "item": null } }), Said::Nothing)),
        1 => Just((json!({ "relationships": { "item": { "data": null } } }), Said::Null)),
        8 => (prop::sample::select(&["anime", "anime", "anime", "manga"][..]), pooled_id())
            .prop_map(|(kind, id)| {
                let json = json!({ "relationships": { "item": { "data": { "type": kind, "id": id } } } });
                (json, Said::To(kind, id))
            }),
    ]
}

/// `meta` as Kitsu could send it: absent, null, with no count, or a
/// count near the number of mappings on the page.
fn meta() -> impl Strategy<Value = Option<Value>> {
    prop_oneof![
        Just(None),
        Just(Some(Value::Null)),
        Just(Some(json!({ "count": null }))),
        (0usize..8).prop_map(|n| Some(json!({ "count": n }))),
    ]
}

type Page = (Vec<(Value, Said)>, Vec<Inlined>, Option<Value>);

fn page() -> impl Strategy<Value = Page> {
    (
        prop::collection::vec(mapping(), 0..7),
        prop::collection::vec(inlined(), 0..7),
        meta(),
    )
}

fn page_body((mappings, included, meta): &Page) -> Vec<u8> {
    let mut body = json!({
        "data": mappings.iter().map(|(m, _)| m.clone()).collect::<Vec<_>>(),
        "included": included.iter().map(inlined_json).collect::<Vec<_>>(),
    });
    if let Some(meta) = meta {
        body["meta"] = meta.clone();
    }
    serde_json::to_vec(&body).expect("serialise")
}

/// The entries a page is specified to read as, `(id, count)` in the
/// order of the mappings, or `None` where the page is no answer. A
/// mapping with no item is no answer; so is an anime pointer whose
/// resource is not inlined, or does not read, since the page then cannot
/// say what is mapped. A null item maps to nothing, a pointer to
/// another type is not an anime, an id pointed to again is read once,
/// the last inlined resource for an id is the one read, and a music
/// entry is dropped.
fn model((mappings, included, _): &Page) -> Option<Vec<(String, Option<u32>)>> {
    // The last readable resource for an id is the one read; an id with
    // no readable resource cannot be shown.
    let mut inlined: HashMap<&str, &Inlined> = HashMap::new();
    for r in included.iter().filter(|r| r.kind == "anime" && r.readable) {
        inlined.insert(&r.id, r);
    }
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for (_, said) in mappings {
        match said {
            Said::Nothing => return None,
            Said::Null => {}
            Said::To(kind, _) if *kind != "anime" => {}
            Said::To(_, id) => {
                if seen.contains(&id.as_str()) {
                    continue;
                }
                seen.push(id);
                let r = inlined.get(id.as_str())?;
                if !r.subtype.is_some_and(|s| s.eq_ignore_ascii_case("music")) {
                    out.push((r.id.clone(), r.episodes));
                }
            }
        }
    }
    Some(out)
}

proptest! {
    /// Any body reads as a page or an error, never a panic.
    #[test]
    fn the_mappings_parser_answers_any_body(
        body in prop_oneof![
            prop::collection::vec(any::<u8>(), 0..64),
            "[\\[\\]{}\":, 0-9a-z.-]{0,80}".prop_map(String::into_bytes),
            Just(br#"{"data":[{"relationships":{"item":{"data":{"type":"anime","id":"1"}}}}],"included":[{"id":"1","type":"anime","attributes":null}]}"#.to_vec()),
        ]
    ) {
        let _ = parse_mapped_anime_response(&body);
    }

    /// A page holds every mapping exactly when Kitsu's count is the
    /// number of mappings on it; a page with no count is not known to.
    #[test]
    fn a_page_is_complete_exactly_when_it_holds_kitsu_s_count(page in page()) {
        let Ok(got) = parse_mapped_anime_response(&page_body(&page)) else {
            return Ok(());
        };
        let count = page.2.as_ref().and_then(|m| m.get("count")).and_then(Value::as_u64);
        prop_assert_eq!(got.complete, count == u64::try_from(page.0.len()).ok());
    }

    /// The entries are the anime the mappings point to, in the
    /// mappings' order, each once: a null item or a pointer to another
    /// type is skipped, a music entry is dropped, and a page with a
    /// mapping that has no item, or points to an anime it does not
    /// include readably, is no answer. So the entries are never more
    /// than the mappings, and always a subsequence of what they point to.
    #[test]
    fn the_entries_are_the_pointed_anime_in_order(page in page()) {
        let parsed = parse_mapped_anime_response(&page_body(&page));
        let Some(want) = model(&page) else {
            prop_assert!(parsed.is_err(), "a page that cannot say what is mapped read as an answer");
            return Ok(());
        };
        let got = parsed.expect("a page");
        prop_assert!(got.entries.len() <= page.0.len());
        let pointed: Vec<&str> = page
            .0
            .iter()
            .filter_map(|(_, said)| match said {
                Said::To(_, id) => Some(id.as_str()),
                _ => None,
            })
            .collect();
        let mut rest = pointed.iter();
        for entry in &got.entries {
            prop_assert!(rest.any(|id| *id == entry.id), "{} out of order", entry.id);
        }
        let read: Vec<(String, Option<u32>)> =
            got.entries.iter().map(|a| (a.id.clone(), a.episode_count)).collect();
        prop_assert_eq!(read, want);
    }
}
