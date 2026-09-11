//! Property coverage for the megaplay reader's pure halves.

use super::megaplay::{lang_of_track, media_id, parse_sources, sources_url};
use crate::error::AniError;
use proptest::prelude::*;

/// The player element with its attributes in any order, the media id
/// among them.
fn player_element(id: u64, before: &[String], after: &[String]) -> String {
    let mut attrs: Vec<String> = before.to_vec();
    attrs.push(format!("data-id=\"{id}\""));
    attrs.extend(after.iter().cloned());
    format!(
        "<div class=\"fix-area\" id=\"megaplay-player\" {}>",
        attrs.join(" ")
    )
}

fn other_attr() -> impl Strategy<Value = String> {
    (
        "(data-realid|data-mediaid|data-fileversion|data-domain)",
        "[a-z0-9.-]{0,12}",
    )
        .prop_map(|(name, value)| format!("{name}=\"{value}\""))
}

proptest! {
    /// The media id comes back whatever surrounds the element and
    /// whatever order its attributes come in.
    #[test]
    fn the_media_id_round_trips_whatever_the_attribute_order(
        id in 1u64..1_000_000_000,
        before in proptest::collection::vec(other_attr(), 0..3),
        after in proptest::collection::vec(other_attr(), 0..3),
        head in "[^<>\"]{0,40}",
        tail in "[^<>\"]{0,40}",
    ) {
        let page = format!(
            "<html><head><title>File {id} - MegaPlay</title></head><body><div class=\"x\">{head}</div>{}<div>{tail}</div></body></html>",
            player_element(id, &before, &after)
        );
        prop_assert_eq!(media_id(&page), Some(id));
    }

    /// A page without the player element — the site's title and a
    /// script, other hosts' pages — names no media.
    #[test]
    fn a_page_without_the_player_element_names_no_media(body in "[^<>]{0,80}", n in 0u64..100_000) {
        let page = format!("<html><head><title>File {n} - MegaPlay</title></head><body><div id=\"player\" data-id=\"{n}\">{body}</div></body></html>");
        prop_assert_eq!(media_id(&page), None);
    }

    /// The endpoint is the embed page's origin, scheme and host,
    /// with the path and query left behind.
    #[test]
    fn the_sources_endpoint_is_the_pages_origin(
        scheme in "(http|https)",
        host in "[a-z]{2,10}(-[0-9]{1,2})?\\.(buzz|site|to)",
        path in "/[a-z0-9/_-]{0,30}",
        query in "(\\?[a-z]=[a-z0-9]{1,6})?",
        id in 1u64..1_000_000_000,
    ) {
        let embed = format!("{scheme}://{host}{path}{query}");
        prop_assert_eq!(
            sources_url(&embed, id),
            Some(format!("{scheme}://{host}/stream/getSourcesNew?id={id}"))
        );
    }

    /// The response round-trips: the master as given, and exactly
    /// the captions tracks with a fetchable file, in order, each with
    /// the language its file name ends in.
    #[test]
    fn the_sources_response_round_trips_the_master_and_the_captions(
        master in "https://[a-z]{2,8}\\.example/[a-z0-9/]{1,20}/master\\.m3u8",
        tracks in proptest::collection::vec(
            (
                "[a-z]{3}",
                "[A-Za-z ()]{1,20}",
                proptest::bool::ANY,
                "(captions|subtitles|thumbnails)",
                proptest::bool::ANY,
            ),
            0..5,
        ),
    ) {
        let listed: Vec<String> = tracks
            .iter()
            .enumerate()
            .map(|(i, (code, label, default, kind, fetchable))| {
                let file = if *fetchable {
                    format!("https://c.example/subs/track_{i}_{code}.vtt")
                } else {
                    format!("/subs/track_{i}_{code}.vtt")
                };
                serde_json::json!({"file": file, "label": label, "kind": kind, "default": default}).to_string()
            })
            .collect();
        let listed = listed.join(",");
        let json = format!(r#"{{"sources":{{"file":"{master}"}},"tracks":[{listed}],"t":1}}"#);
        let payload = parse_sources(&json).expect("the response");
        prop_assert_eq!(&payload.src, &master);
        let expected: Vec<(String, String, bool)> = tracks
            .iter()
            .filter(|(_, _, _, kind, fetchable)| *fetchable && kind != "thumbnails")
            .map(|(code, label, default, _, _)| (code.clone(), label.clone(), *default))
            .collect();
        let got: Vec<(String, String, bool)> = payload
            .subtitles
            .iter()
            .map(|t| (t.lang.clone(), t.label.clone(), t.default))
            .collect();
        prop_assert_eq!(got, expected);
    }

    /// A response whose sources name nothing the transport fetches
    /// is refused, whatever else it carries.
    #[test]
    fn a_response_without_a_fetchable_source_is_refused(
        src in "(|/[a-z/]{1,20}\\.m3u8|ftp://x/y\\.m3u8|master\\.m3u8)",
        tracks in 0usize..3,
        enc in "[A-Za-z0-9_-]{0,20}",
    ) {
        let listed = vec![r#"{"file":"https://c.example/t.vtt","label":"x","kind":"captions"}"#; tracks].join(",");
        let json = format!(r#"{{"sources":{{"file":"{src}"}},"tracks":[{listed}],"enc":"{enc}"}}"#);
        let refused = matches!(parse_sources(&json), Err(AniError::ParseFailed { .. }));
        prop_assert!(refused, "a source the transport cannot fetch: {json}");
        let null = format!(r#"{{"sources":null,"tracks":[{listed}],"enc":"{enc}"}}"#);
        let refused = matches!(parse_sources(&null), Err(AniError::ParseFailed { .. }));
        prop_assert!(refused, "null sources in the clear: {null}");
    }

    /// The language is the three-letter tail of the file's stem when
    /// it has one, whatever precedes it and whatever query follows;
    /// otherwise the trimmed label, or `und` when the label is blank.
    #[test]
    fn the_language_is_the_stems_tail_or_the_label(
        prefix in "[a-z0-9]{1,8}(_[A-Za-z]{1,10}){0,2}",
        code in "[a-z]{3}",
        query in "(\\?[a-z]=[0-9]{1,3})?",
        label in "[A-Za-z ()]{0,16}",
    ) {
        let file = format!("https://c.example/subs/{prefix}_{code}.vtt{query}");
        prop_assert_eq!(lang_of_track(&file, &label), code);
        let plain = format!("https://c.example/subs/{prefix}.vtt{query}");
        let expected = if label.trim().is_empty() { "und".to_string() } else { label.trim().to_string() };
        // A stem whose own tail happens to be three lowercase letters
        // reads as a code; the property covers the other stems.
        let tail = prefix.rsplit('_').next().unwrap_or("");
        prop_assume!(!(tail.len() == 3 && tail.bytes().all(|b| b.is_ascii_lowercase())));
        prop_assert_eq!(lang_of_track(&plain, &label), expected);
    }
}
