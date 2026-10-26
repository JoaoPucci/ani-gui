//! Properties of the viewer's AniList replies, as `shape_prop_test.rs`
//! states them for the other metadata parsers: a document reads as the
//! answer it gives, a field the answer lives in taken out fails, and
//! AniList's own "nothing here" reads as nothing; with fields no
//! parser reads put into every object, as there.

use super::*;
use crate::meta::shape_prop_tests::{extras, without};
use proptest::prelude::*;
use serde_json::{json, Value};

fn status() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("CURRENT"),
        Just("PLANNING"),
        Just("COMPLETED"),
        Just("PAUSED"),
        Just("DROPPED"),
        Just("REPEATING"),
    ]
}

fn body(doc: &Value, extra: &[(String, Value)]) -> Vec<u8> {
    crate::meta::shape_prop_tests::decorated(doc.clone(), extra)
}

proptest! {
    #[test]
    fn the_viewer_s_entry_reads_its_progress_or_off_the_list(
        st in status(),
        progress in 0u32..2000,
        extra in extras(),
    ) {
        let doc = json!({ "data": { "Media": {
            "mediaListEntry": { "status": st, "progress": progress }, "episodes": 12 } } });
        let read = parse_media_entry_response(&body(&doc, &extra)).expect("an answer").expect("listed");
        prop_assert_eq!(read.progress_episodes, progress);
        for field in ["progress", "status"] {
            let missing = without(doc.clone(), &["data", "Media", "mediaListEntry", field]);
            prop_assert!(parse_media_entry_response(&body(&missing, &extra)).is_err());
        }
        let off = json!({ "data": { "Media": { "mediaListEntry": null, "episodes": 12 } } });
        prop_assert_eq!(parse_media_entry_response(&body(&off, &extra)).expect("an answer"), None);
        for missing in [
            without(doc.clone(), &["data", "Media", "mediaListEntry"]),
            without(doc, &["data", "Media"]),
        ] {
            prop_assert!(parse_media_entry_response(&body(&missing, &extra)).is_err());
        }
        let unknown = json!({ "data": { "Media": null } });
        prop_assert_eq!(parse_media_entry_response(&body(&unknown, &extra)).expect("an answer"), None);
    }

    #[test]
    fn a_list_page_reads_each_entry_or_none(
        rows in proptest::collection::vec((1u32..1_000_000, status(), 0u32..500), 0..5),
        extra in extras(),
    ) {
        let entries: Vec<Value> = rows
            .iter()
            .map(|(id, st, progress)| json!({
                "mediaId": id, "status": st, "progress": progress, "score": 0, "updatedAt": 0,
                "media": { "idMal": null, "title": { "romaji": "x", "english": null, "userPreferred": "x" } },
            }))
            .collect();
        let doc = json!({ "data": { "MediaListCollection": {
            "hasNextChunk": false, "lists": [{ "entries": entries }] } } });
        let page = parse_media_list_page(&body(&doc, &extra)).expect("an answer");
        prop_assert_eq!(page.entries.len(), rows.len());
        for (entry, (id, _, progress)) in page.entries.iter().zip(&rows) {
            prop_assert_eq!(entry.media_id.0, *id);
            prop_assert_eq!(entry.progress_episodes, *progress);
        }
        for missing in [
            without(doc.clone(), &["data", "MediaListCollection", "lists", "0", "entries"]),
            without(doc, &["data", "MediaListCollection", "lists"]),
        ] {
            prop_assert!(parse_media_list_page(&body(&missing, &extra)).is_err());
        }
        let empty = json!({ "data": { "MediaListCollection": { "hasNextChunk": false, "lists": [] } } });
        prop_assert!(parse_media_list_page(&body(&empty, &extra)).expect("an answer").entries.is_empty());
    }
}
