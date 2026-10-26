//! Property coverage for AniList's batched by-MAL `Page` replies: the
//! cut-page flag, and every media each MAL id is recorded by.

use super::{page_has_next, parse_media_ids_recording_mal, parse_media_listed_by_mal_response};
use proptest::prelude::*;
use std::collections::HashMap;

/// A body as the wire could hand one over: arbitrary bytes, or text
/// made of JSON's punctuation and the envelope's own words.
fn any_body() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..64),
        "[\\[\\]{}\":, 0-9a-zA-Z.-]{0,80}".prop_map(String::into_bytes),
        Just(br#"{"data":{"Page":{"media":[{"id":1,"idMal":null}]}}}"#.to_vec()),
        Just(br#"{"data":{"Page":null}}"#.to_vec()),
    ]
}

/// The `pageInfo` a page carries: absent, null, empty, or with its
/// flag null, true or false.
fn page_info() -> impl Strategy<Value = (Option<serde_json::Value>, Option<bool>)> {
    prop_oneof![
        Just((None, None)),
        Just((Some(serde_json::Value::Null), None)),
        Just((Some(serde_json::json!({})), None)),
        Just((Some(serde_json::json!({ "hasNextPage": null })), None)),
        any::<bool>().prop_map(|b| (Some(serde_json::json!({ "hasNextPage": b })), Some(b))),
    ]
}

/// The media on a page, over ids small enough that one media is listed
/// twice and one MAL id is recorded by several.
fn page_media() -> impl Strategy<Value = Vec<(u32, Option<u32>)>> {
    prop::collection::vec(
        (
            prop_oneof![0u32..8, any::<u32>()],
            proptest::option::of(prop_oneof![0u32..4, any::<u32>()]),
        ),
        0..10,
    )
}

fn media_body(media: &[(u32, Option<u32>)]) -> Vec<u8> {
    let media: Vec<serde_json::Value> = media
        .iter()
        .map(|(id, mal)| serde_json::json!({ "id": id, "idMal": mal }))
        .collect();
    serde_json::to_vec(&serde_json::json!({ "data": { "Page": { "media": media } } }))
        .expect("serialise")
}

/// The listing stated without the parser: each MAL id's media ids,
/// sorted and each once.
fn model(media: &[(u32, Option<u32>)]) -> HashMap<u32, Vec<u32>> {
    let mut want: HashMap<u32, Vec<u32>> = HashMap::new();
    for (id, mal) in media {
        if let Some(mal) = mal {
            let ids = want.entry(*mal).or_default();
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
    }
    for ids in want.values_mut() {
        ids.sort_unstable();
    }
    want
}

proptest! {
    /// Any body reads as a flag or an error, never a panic.
    #[test]
    fn page_has_next_answers_any_body(body in any_body()) {
        let _ = page_has_next(&body);
    }

    /// A page is cut exactly when AniList says `hasNextPage: true`; a
    /// page that says false, null or nothing at all is whole.
    #[test]
    fn a_page_is_cut_exactly_when_it_says_so((info, flag) in page_info()) {
        let mut page = serde_json::json!({ "media": [] });
        if let Some(info) = info {
            page["pageInfo"] = info;
        }
        let body = serde_json::to_vec(&serde_json::json!({ "data": { "Page": page } }))
            .expect("serialise");
        prop_assert_eq!(page_has_next(&body).ok(), Some(flag == Some(true)));
    }

    /// Any body reads as a listing or an error, never a panic, and
    /// the two by-MAL readers agree on which bodies are errors.
    #[test]
    fn the_by_mal_readers_answer_any_body_alike(body in any_body(), mal: u32) {
        let listed = parse_media_listed_by_mal_response(&body);
        let recording = parse_media_ids_recording_mal(&body, mal);
        prop_assert_eq!(listed.is_err(), recording.is_err());
    }

    /// Every MAL id the page records maps to the media recording it,
    /// in AniList id order and each once; media without one are not
    /// listed under any.
    #[test]
    fn the_listing_is_each_mal_id_s_media_sorted_and_once(media in page_media()) {
        let got = parse_media_listed_by_mal_response(&media_body(&media)).expect("a page");
        prop_assert_eq!(got, model(&media));
    }

    /// The order AniList lists the media in does not change the
    /// listing.
    #[test]
    fn the_listing_does_not_depend_on_page_order(
        (media, shuffled) in page_media().prop_flat_map(|m| (Just(m.clone()), Just(m).prop_shuffle()))
    ) {
        prop_assert_eq!(
            parse_media_listed_by_mal_response(&media_body(&media)).expect("a page"),
            parse_media_listed_by_mal_response(&media_body(&shuffled)).expect("a page"),
        );
    }

    /// The media recording one MAL id are that id's row of the full
    /// listing, and none for an id the page does not record.
    #[test]
    fn one_mal_id_s_media_are_its_row_of_the_listing(media in page_media(), probe in 0u32..6) {
        let body = media_body(&media);
        let listed = parse_media_listed_by_mal_response(&body).expect("a page");
        for mal in listed.keys().copied().chain(std::iter::once(probe)) {
            prop_assert_eq!(
                parse_media_ids_recording_mal(&body, mal).expect("a page"),
                listed.get(&mal).cloned().unwrap_or_default(),
            );
        }
    }

    /// The media recording a MAL id come back strictly increasing,
    /// and are exactly the ids of the media that record it.
    #[test]
    fn one_mal_id_s_media_are_strictly_increasing(media in page_media(), mal in 0u32..4) {
        let got = parse_media_ids_recording_mal(&media_body(&media), mal).expect("a page");
        prop_assert!(got.windows(2).all(|w| w[0] < w[1]));
        let mut want: Vec<u32> = media
            .iter()
            .filter(|(_, m)| *m == Some(mal))
            .map(|(id, _)| *id)
            .collect();
        want.sort_unstable();
        want.dedup();
        prop_assert_eq!(got, want);
    }
}
