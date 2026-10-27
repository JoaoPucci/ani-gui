//! Properties of the metadata parsers' reading of a reply: for any
//! document a service could send, with fields the parser does not read
//! scattered through every object of it, the parser reads the answer
//! the document gives; with a field the answer lives in taken out, it
//! fails; and the service's own "nothing here" reads as nothing.
//!
//! The examples in `shape_test.rs` pin particular bodies; these state
//! the same three rules over generated documents, so a parser that
//! reads one shape right by accident does not pass.

use super::*;
use proptest::prelude::*;
use serde_json::{json, Value};

/// Fields no parser reads, put into every object of a document.
pub(super) fn extras() -> impl Strategy<Value = Vec<(String, Value)>> {
    let value = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i32>().prop_map(Value::from),
        "[a-z ]{0,8}".prop_map(Value::from),
        Just(json!({ "x_nested": [1, null] })),
    ];
    proptest::collection::vec(("x_[a-z]{1,4}", value), 0..3)
}

/// `doc` with `extras` added to each of its objects, at every depth.
/// No generated document carries a map read as such (Kitsu's `titles`),
/// where the extras would read as entries.
pub(super) fn decorated(mut doc: Value, extras: &[(String, Value)]) -> Vec<u8> {
    fn walk(v: &mut Value, extras: &[(String, Value)]) {
        match v {
            Value::Object(map) => {
                for child in map.values_mut() {
                    walk(child, extras);
                }
                for (k, x) in extras {
                    map.entry(k.clone()).or_insert_with(|| x.clone());
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|i| walk(i, extras)),
            _ => {}
        }
    }
    walk(&mut doc, extras);
    serde_json::to_vec(&doc).expect("json")
}

/// `doc` without the field at `path` (object keys, or array indices).
pub(super) fn without(mut doc: Value, path: &[&str]) -> Value {
    let (last, parents) = path.split_last().expect("a path");
    let mut at = &mut doc;
    for p in parents {
        at = match p.parse::<usize>() {
            Ok(i) => &mut at[i],
            Err(_) => &mut at[*p],
        };
    }
    at.as_object_mut().expect("an object").remove(*last);
    doc
}

/// `doc` with the field at `path` set to `value`.
fn with(mut doc: Value, path: &[&str], value: Value) -> Value {
    let mut at = &mut doc;
    for p in path {
        at = match p.parse::<usize>() {
            Ok(i) => &mut at[i],
            Err(_) => &mut at[*p],
        };
    }
    *at = value;
    doc
}

fn site() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("myanimelist/anime"),
        Just("anilist/anime"),
        Just("anilist/manga"),
        Just("anidb"),
    ]
}

// ── Kitsu ──────────────────────────────────────────────────────────

fn mappings_doc(sites: &[(&str, u32)]) -> Value {
    let included: Vec<Value> = sites
        .iter()
        .enumerate()
        .map(|(i, (site, id))| {
            json!({ "id": i.to_string(), "type": "mappings",
                    "attributes": { "externalSite": site, "externalId": id.to_string() } })
        })
        .collect();
    json!({ "data": { "id": "7", "type": "anime", "attributes": { "canonicalTitle": "x" } },
            "included": included })
}

fn related_doc(edges: &[(String, bool, u32, Option<u32>)]) -> Value {
    let data: Vec<Value> = edges
        .iter()
        .enumerate()
        .map(|(i, (role, anime, id, _))| {
            json!({ "id": i.to_string(), "type": "mediaRelationships",
                    "attributes": { "role": role },
                    "relationships": { "destination": { "data": {
                        "type": if *anime { "anime" } else { "manga" }, "id": id.to_string() } } } })
        })
        .collect();
    let included: Vec<Value> = edges
        .iter()
        .map(|(_, anime, id, count)| {
            json!({ "id": id.to_string(), "type": if *anime { "anime" } else { "manga" },
                    "attributes": { "subtype": "TV", "status": "finished", "episodeCount": count } })
        })
        .collect();
    json!({ "data": data, "included": included, "meta": { "count": edges.len() } })
}

fn edges() -> impl Strategy<Value = Vec<(String, bool, u32, Option<u32>)>> {
    proptest::collection::btree_map(
        1u32..100_000,
        (
            "[a-z_]{3,10}",
            any::<bool>(),
            proptest::option::of(1u32..200),
        ),
        0..5,
    )
    .prop_map(|m| {
        m.into_iter()
            .map(|(id, (role, anime, count))| (role, anime, id, count))
            .collect()
    })
}

proptest! {
    #[test]
    fn kitsu_mappings_read_the_ids_they_carry(
        sites in proptest::collection::vec((site(), 1u32..1_000_000), 0..5),
        extra in extras(),
    ) {
        let doc = mappings_doc(&sites);
        let first = |want: &str| sites.iter().find(|(s, _)| *s == want).map(|(_, id)| *id);
        let ids = kitsu::parse_external_ids_from_mappings(&decorated(doc.clone(), &extra))
            .expect("the document");
        prop_assert_eq!(ids.mal, first("myanimelist/anime"));
        prop_assert_eq!(ids.anilist, first("anilist/anime"));
        prop_assert!(kitsu::parse_external_ids_from_mappings(
            &decorated(without(doc, &["data"]), &extra)).is_err());
    }

    #[test]
    fn a_kitsu_mapping_lookup_reads_its_anime_or_no_mapping(
        id in 1u32..1_000_000,
        title in "[A-Za-z ]{1,12}",
        extra in extras(),
    ) {
        let doc = json!({
            "data": [{ "id": "1", "type": "mappings",
                       "relationships": { "item": { "data": { "type": "anime", "id": id.to_string() } } } }],
            "included": [{ "id": id.to_string(), "type": "anime",
                           "attributes": { "canonicalTitle": title, "subtype": "TV" } }],
        });
        let read = kitsu::parse_mappings_response(&decorated(doc.clone(), &extra))
            .expect("the document")
            .expect("its anime");
        prop_assert_eq!(read.id, id.to_string());
        prop_assert_eq!(read.canonical_title, title);
        for missing in [
            without(doc.clone(), &["data", "0", "relationships", "item"]),
            without(doc.clone(), &["data", "0", "relationships", "item", "data"]),
            without(doc.clone(), &["data", "0", "relationships"]),
            without(doc.clone(), &["included"]),
        ] {
            prop_assert!(kitsu::parse_mappings_response(&decorated(missing, &extra)).is_err());
        }
        let nothing = with(doc, &["data", "0", "relationships", "item", "data"], Value::Null);
        prop_assert_eq!(kitsu::parse_mappings_response(&decorated(nothing, &extra)).expect("an answer"), None);
        let none = json!({ "data": [] });
        prop_assert_eq!(kitsu::parse_mappings_response(&decorated(none, &extra)).expect("an answer"), None);
    }

    #[test]
    fn kitsu_relations_read_every_entry_they_include(edges in edges(), extra in extras()) {
        let doc = related_doc(&edges);
        let read = kitsu::parse_relationships_response(&decorated(doc.clone(), &extra))
            .expect("the document");
        prop_assert!(read.complete);
        prop_assert_eq!(read.entries.len(), edges.len());
        for (entry, (role, anime, id, count)) in read.entries.iter().zip(&edges) {
            prop_assert_eq!(&entry.role, role);
            prop_assert_eq!(entry.kind.as_str(), if *anime { "anime" } else { "manga" });
            prop_assert_eq!(&entry.id, &id.to_string());
            prop_assert_eq!(entry.episode_count, *count);
            prop_assert_eq!(entry.subtype.as_deref(), Some("TV"));
        }
        if !edges.is_empty() {
            for missing in [
                without(doc.clone(), &["data", "0", "relationships", "destination", "data"]),
                without(doc.clone(), &["included", "0", "attributes"]),
                without(doc.clone(), &["included"]),
            ] {
                prop_assert!(kitsu::parse_relationships_response(&decorated(missing, &extra)).is_err());
            }
            // A destination pointing nowhere is an answer: that edge is
            // skipped, the rest read.
            let dangling = with(doc.clone(), &["data", "0", "relationships", "destination", "data"], Value::Null);
            let read = kitsu::parse_relationships_response(&decorated(dangling, &extra)).expect("an answer");
            prop_assert_eq!(read.entries.len(), edges.len() - 1);
        }
        if edges.len() >= 2 {
            // One destination left out while the others are included.
            let mut one_missing = doc;
            one_missing["included"].as_array_mut().expect("included").remove(0);
            prop_assert!(kitsu::parse_relationships_response(&decorated(one_missing, &extra)).is_err());
        }
    }
}

// ── AniList ────────────────────────────────────────────────────────

fn media(fields: Value) -> Value {
    json!({ "data": { "Media": fields } })
}

proptest! {
    #[test]
    fn anilist_media_replies_read_their_field_or_none(
        id_mal in proptest::option::of(1u32..1_000_000),
        banner in proptest::option::of("https://a/[a-z]{1,8}\\.jpg"),
        id in 1u32..1_000_000,
        extra in extras(),
    ) {
        let doc = media(json!({ "idMal": id_mal, "bannerImage": banner, "id": id }));
        let body = decorated(doc.clone(), &extra);
        prop_assert_eq!(anilist::parse_mal_id_response(&body).expect("an answer"), id_mal);
        prop_assert_eq!(anilist::parse_banner_response(&body).expect("an answer"), banner);
        prop_assert_eq!(anilist::parse_media_id_response(&body).expect("an answer"), Some(id));
        prop_assert!(anilist::parse_mal_id_response(
            &decorated(without(doc.clone(), &["data", "Media", "idMal"]), &extra)).is_err());
        prop_assert!(anilist::parse_banner_response(
            &decorated(without(doc.clone(), &["data", "Media", "bannerImage"]), &extra)).is_err());
        let no_media = decorated(without(doc, &["data", "Media"]), &extra);
        prop_assert!(anilist::parse_media_id_response(&no_media).is_err());
        prop_assert!(anilist::parse_mal_id_response(&no_media).is_err());
        prop_assert!(anilist::parse_banner_response(&no_media).is_err());
        let unknown = decorated(media(Value::Null), &extra);
        prop_assert_eq!(anilist::parse_mal_id_response(&unknown).expect("an answer"), None);
        prop_assert_eq!(anilist::parse_banner_response(&unknown).expect("an answer"), None);
        prop_assert_eq!(anilist::parse_media_id_response(&unknown).expect("an answer"), None);
    }

    #[test]
    fn anilist_airing_reads_its_status_or_none(
        status in prop_oneof![Just("FINISHED"), Just("RELEASING"), Just("NOT_YET_RELEASED")],
        episodes in proptest::option::of(1u32..2000),
        extra in extras(),
    ) {
        let doc = media(json!({ "status": status, "episodes": episodes,
                                "nextAiringEpisode": null, "airingSchedule": { "nodes": [] } }));
        let read = anilist_airing::parse_airing_response(&decorated(doc.clone(), &extra))
            .expect("an answer")
            .expect("known");
        let aired = match status {
            "FINISHED" => episodes,
            "NOT_YET_RELEASED" => Some(0),
            _ => None,
        };
        prop_assert_eq!(read.aired, aired);
        for field in ["status", "episodes", "nextAiringEpisode", "airingSchedule"] {
            prop_assert!(anilist_airing::parse_airing_response(
                &decorated(without(doc.clone(), &["data", "Media", field]), &extra)).is_err(),
                "without {}", field);
        }
        prop_assert!(anilist_airing::parse_airing_response(
            &decorated(without(doc.clone(), &["data", "Media", "airingSchedule", "nodes"]), &extra)).is_err());
        prop_assert!(anilist_airing::parse_airing_response(
            &decorated(without(doc, &["data", "Media"]), &extra)).is_err());
        prop_assert!(anilist_airing::parse_airing_response(&decorated(media(Value::Null), &extra))
            .expect("an answer")
            .is_none());
    }

    #[test]
    fn an_anilist_by_mal_batch_reads_each_node_s_ids(
        nodes in proptest::collection::btree_map(1u32..100_000, proptest::option::of(1u32..100_000), 0..6),
        extra in extras(),
    ) {
        let list: Vec<Value> = nodes.iter().map(|(id, mal)| json!({ "id": id, "idMal": mal })).collect();
        let doc = json!({ "data": { "Page": { "media": list } } });
        let read = anilist::parse_media_listed_by_mal_response(&decorated(doc.clone(), &extra))
            .expect("an answer");
        // Each MAL id AniList listed maps to every node recording it, in
        // id order; a node whose idMal is null records none.
        let mut expected: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
        for (id, mal) in &nodes {
            if let Some(mal) = mal {
                expected.entry(*mal).or_default().push(*id);
            }
        }
        prop_assert_eq!(read, expected);
        if !nodes.is_empty() {
            prop_assert!(anilist::parse_media_listed_by_mal_response(
                &decorated(without(doc.clone(), &["data", "Page", "media", "0", "idMal"]), &extra)).is_err());
            prop_assert!(anilist::parse_media_listed_by_mal_response(
                &decorated(without(doc, &["data", "Page", "media", "0", "id"]), &extra)).is_err());
        }
    }

    #[test]
    fn anilist_streaming_episodes_read_each_numbered_entry_or_nothing(
        numbers in proptest::collection::btree_set(1u32..500, 0..6),
        extra in extras(),
    ) {
        let list: Vec<Value> = numbers
            .iter()
            .map(|n| json!({ "title": format!("Episode {n} - Story {n}"), "thumbnail": null }))
            .collect();
        let doc = media(json!({ "episodes": 12, "status": "FINISHED", "streamingEpisodes": list }));
        let read = anilist_streaming_eps::parse_streaming_episodes_response(&decorated(doc.clone(), &extra))
            .expect("an answer");
        prop_assert_eq!(read.entries.keys().copied().collect::<Vec<_>>(), numbers.iter().copied().collect::<Vec<_>>());
        prop_assert!(anilist_streaming_eps::parse_streaming_episodes_response(
            &decorated(without(doc.clone(), &["data", "Media", "streamingEpisodes"]), &extra)).is_err());
        prop_assert!(anilist_streaming_eps::parse_streaming_episodes_response(
            &decorated(without(doc, &["data", "Media"]), &extra)).is_err());
        let unknown = anilist_streaming_eps::parse_streaming_episodes_response(
            &decorated(media(Value::Null), &extra)).expect("an answer");
        prop_assert!(unknown.entries.is_empty());
    }
}

proptest! {
    #[test]
    fn an_anilist_airing_batch_reads_each_node_or_none(
        ids in proptest::collection::btree_set(1u32..100_000, 0..5),
        cut in any::<bool>(),
        extra in extras(),
    ) {
        let list: Vec<Value> = ids
            .iter()
            .map(|id| json!({ "id": id, "status": "FINISHED", "episodes": 12,
                              "nextAiringEpisode": null, "airingSchedule": { "nodes": [] } }))
            .collect();
        let doc = json!({ "data": { "Page": {
            "pageInfo": { "hasNextPage": cut }, "media": list } } });
        let read = anilist_airing::parse_airing_batch_response(&decorated(doc.clone(), &extra))
            .expect("an answer");
        prop_assert_eq!(read.keys().copied().collect::<std::collections::BTreeSet<_>>(), ids.clone());
        for path in [&["data", "Page", "media"][..], &["data", "Page", "pageInfo"],
                     &["data", "Page", "pageInfo", "hasNextPage"]] {
            prop_assert!(anilist_airing::parse_airing_batch_response(
                &decorated(without(doc.clone(), path), &extra)).is_err(), "without {:?}", path);
        }
        if !ids.is_empty() {
            for field in ["id", "status", "episodes", "nextAiringEpisode", "airingSchedule"] {
                prop_assert!(anilist_airing::parse_airing_batch_response(
                    &decorated(without(doc.clone(), &["data", "Page", "media", "0", field]), &extra))
                    .is_err(), "without {}", field);
            }
            prop_assert!(anilist_airing::parse_airing_batch_response(
                &decorated(without(doc, &["data", "Page", "media", "0", "airingSchedule", "nodes"]), &extra))
                .is_err());
        }
    }
}

// ── aniskip, MAL ───────────────────────────────────────────────────

fn intervals() -> impl Strategy<Value = Vec<(bool, u16, u16)>> {
    proptest::collection::vec((any::<bool>(), 0u16..2000, 1u16..300), 0..4)
}

fn mal_status() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("watching"),
        Just("completed"),
        Just("on_hold"),
        Just("dropped"),
        Just("plan_to_watch"),
    ]
}

proptest! {
    #[test]
    fn aniskip_reads_its_intervals_or_nothing_catalogued(spans in intervals(), extra in extras()) {
        let results: Vec<Value> = spans
            .iter()
            .map(|(op, start, len)| json!({
                "skipType": if *op { "op" } else { "ed" },
                "interval": { "startTime": f32::from(*start), "endTime": f32::from(*start + *len) },
            }))
            .collect();
        let doc = json!({ "found": true, "results": results });
        let read = aniskip::parse_skip_times(&decorated(doc.clone(), &extra)).expect("an answer");
        prop_assert_eq!(read.len(), spans.len());
        for (got, (op, start, len)) in read.iter().zip(&spans) {
            prop_assert_eq!(got.skip_type.as_str(), if *op { "op" } else { "ed" });
            prop_assert_eq!(got.start_time, f32::from(*start));
            prop_assert_eq!(got.end_time, f32::from(*start + *len));
        }
        prop_assert!(aniskip::parse_skip_times(&decorated(without(doc.clone(), &["results"]), &extra)).is_err());
        prop_assert!(aniskip::parse_skip_times(&decorated(without(doc.clone(), &["found"]), &extra)).is_err());
        let none = json!({ "found": false, "results": [] });
        prop_assert!(aniskip::parse_skip_times(&decorated(none, &extra)).expect("an answer").is_empty());
        // Not found is aniskip's answer whatever the results say.
        let not_found = with(doc, &["found"], Value::Bool(false));
        prop_assert!(aniskip::parse_skip_times(&decorated(not_found, &extra)).expect("an answer").is_empty());
    }

    #[test]
    fn mal_list_statuses_read_their_count_or_off_the_list(
        status in mal_status(),
        watched in 0u32..2000,
        id in 1u32..1_000_000,
        extra in extras(),
    ) {
        let doc = json!({ "id": id, "num_episodes": 12,
                          "my_list_status": { "status": status, "num_episodes_watched": watched } });
        let read = mal_user_parse::parse_my_list_status_entry(&decorated(doc.clone(), &extra))
            .expect("an answer")
            .expect("on the list");
        prop_assert_eq!(read.progress_episodes, watched);
        for field in ["num_episodes_watched", "status"] {
            prop_assert!(mal_user_parse::parse_my_list_status_entry(
                &decorated(without(doc.clone(), &["my_list_status", field]), &extra)).is_err());
        }
        prop_assert!(mal_user_parse::parse_my_list_status_entry(
            &decorated(without(doc.clone(), &["id"]), &extra)).is_err());
        prop_assert!(mal_user_parse::parse_my_list_status_entry(
            &decorated(without(doc, &["my_list_status"]), &extra)).expect("an answer").is_none());

        let saved = json!({ "status": status, "num_episodes_watched": watched });
        let entry = mal_user_parse::parse_list_status_response(&decorated(saved.clone(), &extra), id)
            .expect("an answer");
        prop_assert_eq!(entry.progress_episodes, watched);
        prop_assert!(mal_user_parse::parse_list_status_response(
            &decorated(without(saved, &["num_episodes_watched"]), &extra), id).is_err());
    }

    #[test]
    fn a_mal_list_page_reads_each_row_or_none(
        rows in proptest::collection::vec((1u32..1_000_000, mal_status(), 0u32..500), 0..5),
        extra in extras(),
    ) {
        let data: Vec<Value> = rows
            .iter()
            .map(|(id, status, watched)| json!({
                "node": { "id": id, "title": "Show" },
                "list_status": { "status": status, "num_episodes_watched": watched },
            }))
            .collect();
        let doc = json!({ "data": data, "paging": {} });
        let page = mal_user_parse::parse_list_page(&decorated(doc.clone(), &extra)).expect("an answer");
        prop_assert_eq!(page.entries.len(), rows.len());
        for (entry, (id, _, watched)) in page.entries.iter().zip(&rows) {
            prop_assert_eq!(entry.media_id.0, *id);
            prop_assert_eq!(entry.progress_episodes, *watched);
        }
        if !rows.is_empty() {
            prop_assert!(mal_user_parse::parse_list_page(&decorated(
                without(doc, &["data", "0", "list_status", "num_episodes_watched"]), &extra)).is_err());
        }
    }
}
