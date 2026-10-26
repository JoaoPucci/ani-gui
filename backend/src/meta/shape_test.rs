//! A metadata reply that is JSON but not the document asked for is no
//! answer: each parser fails it, so the caller treats it as unknown
//! rather than reading it as an empty answer and keeping that.
//!
//! Each case gives a parser bodies a service never sends as its answer
//! (an empty object, the envelope without the field the answer lives
//! in) and expects an error, beside the shapes that do mean "nothing
//! here", which must still read as such.

use super::*;

/// Bodies that are JSON and no document of any of these services.
const NOT_A_DOCUMENT: &[&str] = &["{}", r#"{"meta":{}}"#, r#"{"unexpected":true}"#];

fn fails<T: std::fmt::Debug>(what: &str, body: &str, got: crate::error::Result<T>) {
    assert!(got.is_err(), "{what} read {body} as an answer: {got:?}");
}

#[test]
fn kitsu_mappings_need_the_anime_document() {
    for body in NOT_A_DOCUMENT {
        fails(
            "external ids",
            body,
            kitsu::parse_external_ids_from_mappings(body.as_bytes()),
        );
        fails(
            "MAL id",
            body,
            kitsu::parse_mal_id_from_mappings(body.as_bytes()),
        );
    }
    // The document with no mappings is an answer: the show has none.
    let none = r#"{"data":{"id":"1","type":"anime"},"included":[]}"#;
    assert_eq!(
        kitsu::parse_external_ids_from_mappings(none.as_bytes()).expect("an answer"),
        kitsu::ExternalIds::default()
    );
}

#[test]
fn kitsu_relations_need_the_entries_they_name() {
    // A relation whose destination is not among the included entries
    // says nothing of what it is.
    let body = r#"{"data":[{"id":"1","type":"mediaRelationships","attributes":{"role":"sequel"},
        "relationships":{"destination":{"data":{"type":"anime","id":"2"}}}}],"meta":{"count":1}}"#;
    fails(
        "relations",
        body,
        kitsu::parse_relationships_response(body.as_bytes()),
    );
    // No relations at all needs no included entries.
    let none = r#"{"data":[],"meta":{"count":0}}"#;
    assert!(kitsu::parse_relationships_response(none.as_bytes()).is_ok());
}

#[test]
fn anilist_media_replies_need_the_media_field() {
    for body in [r#"{"data":{}}"#, r#"{"data":{"Media":{}}}"#] {
        fails(
            "idMal",
            body,
            anilist::parse_mal_id_response(body.as_bytes()),
        );
        fails(
            "banner",
            body,
            anilist::parse_banner_response(body.as_bytes()),
        );
    }
    fails(
        "media id",
        r#"{"data":{}}"#,
        anilist::parse_media_id_response(br#"{"data":{}}"#),
    );
    fails(
        "airing",
        r#"{"data":{}}"#,
        anilist_airing::parse_airing_response(br#"{"data":{}}"#),
    );
    for body in [r#"{"data":{}}"#, r#"{"data":{"Media":{"episodes":12}}}"#] {
        fails(
            "streaming episodes",
            body,
            anilist_streaming_eps::parse_streaming_episodes_response(body.as_bytes()),
        );
    }
    // Media null is AniList's answer for an id it does not know.
    let unknown = r#"{"data":{"Media":null}}"#;
    assert_eq!(
        anilist::parse_mal_id_response(unknown.as_bytes()).expect("an answer"),
        None
    );
    assert_eq!(
        anilist_airing::parse_airing_response(unknown.as_bytes()).expect("an answer"),
        None
    );
}

#[test]
fn anilist_batches_need_their_media_lists() {
    let body = r#"{"data":{"Page":{}}}"#;
    fails(
        "airing batch",
        body,
        anilist_airing::parse_airing_batch_response(body.as_bytes()),
    );
    let no_mal = r#"{"data":{"Page":{"media":[{"id":5}]}}}"#;
    fails(
        "media ids by MAL",
        no_mal,
        anilist::parse_media_ids_by_mal_response(no_mal.as_bytes()),
    );
}

#[test]
fn aniskip_needs_its_envelope() {
    for body in NOT_A_DOCUMENT {
        fails(
            "skip times",
            body,
            aniskip::parse_skip_times(body.as_bytes()),
        );
    }
    fails(
        "skip times",
        r#"{"found":true}"#,
        aniskip::parse_skip_times(br#"{"found":true}"#),
    );
    // Nothing catalogued is aniskip's answer.
    let none = r#"{"found":false,"results":[]}"#;
    assert!(aniskip::parse_skip_times(none.as_bytes())
        .expect("an answer")
        .is_empty());
}

#[test]
fn mal_list_status_needs_the_anime_document() {
    for body in NOT_A_DOCUMENT {
        fails(
            "MAL list status",
            body,
            mal_user_parse::parse_my_list_status_entry(body.as_bytes()),
        );
        fails(
            "MAL tracked media",
            body,
            mal_user_parse::parse_tracked_media(body.as_bytes()),
        );
    }
    // The anime with no list status is MAL's answer: not on the list.
    let off_list = r#"{"id":1,"title":"Show","num_episodes":12}"#;
    assert_eq!(
        mal_user_parse::parse_my_list_status_entry(off_list.as_bytes()).expect("an answer"),
        None
    );
}

#[test]
fn anilist_airing_needs_what_it_asked_for() {
    // Every airing query asks for the status, the episode count, the
    // next episode and the schedule; a Media without them is no answer
    // about the show's airing.
    let empty = r#"{"data":{"Media":{}}}"#;
    fails(
        "airing",
        empty,
        anilist_airing::parse_airing_response(empty.as_bytes()),
    );
    let node = r#"{"data":{"Page":{"media":[{"id":5}]}}}"#;
    fails(
        "airing batch",
        node,
        anilist_airing::parse_airing_batch_response(node.as_bytes()),
    );
    // Asked and answered null is AniList's answer: no schedule known.
    let known = r#"{"data":{"Media":{"status":"FINISHED","episodes":12,
        "nextAiringEpisode":null,"airingSchedule":{"nodes":[]}}}}"#;
    assert!(anilist_airing::parse_airing_response(known.as_bytes()).is_ok());
}
