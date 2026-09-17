//! Property coverage for the megaplay reader's pure halves.

use super::megaplay::{media_id, sources_url};
use super::megaplay_sources::{lang_of_track, parse_sources};
use crate::error::AniError;
use crate::scraper::provider::SUBTITLE_URL_CAP;
use base64::Engine as _;
use proptest::prelude::*;

/// The constants the site's player script opens its sources answer
/// with, written out here rather than read from the reader: the
/// property drives the site's own cipher from the outside, so a
/// reader that opened its answers under some other key would fail it.
const SOURCES_KEY: &[u8; 16] = b"i?LMTAx0Q6,:}50U";
const SOURCES_IV: &[u8; 16] = b"W0;27ToaUpl_P%'c";

/// `plain` as the site hands it back: AES-256-CBC under the site's
/// constants, the key zero-padded to the cipher's width, base64url
/// with the padding left off.
fn encrypted(plain: &str) -> String {
    use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
    let mut key = [0u8; 32];
    key[..SOURCES_KEY.len()].copy_from_slice(SOURCES_KEY);
    let cipher = cbc::Encryptor::<aes::Aes256>::new(&key.into(), SOURCES_IV.into());
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(cipher.encrypt_padded_vec_mut::<Pkcs7>(plain.as_bytes()))
}

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

/// A sources row naming no stream the transport can fetch: not an
/// object at all, or an object whose `file` is missing, blank, not a
/// string, or not an absolute http(s) URL.
fn unfetchable_source_row(n: u8) -> String {
    match n % 8 {
        0 => "null".to_string(),
        1 => "\"https://c.example/junk/master.m3u8\"".to_string(),
        2 => "3".to_string(),
        3 => "[\"https://c.example/junk/master.m3u8\"]".to_string(),
        4 => r#"{"label":"No file"}"#.to_string(),
        5 => r#"{"file":["https://c.example/junk/master.m3u8"]}"#.to_string(),
        6 => r#"{"file":"/junk/master.m3u8"}"#.to_string(),
        _ => r#"{"file":""}"#.to_string(),
    }
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
            Some(format!("{scheme}://{host}/stream/getSourcesNew?id={id}&s=bcdn"))
        );
    }

    /// The endpoint's query is the media id and the one delivery
    /// network the client can play a stream from, whatever network
    /// the page named and whatever else its query held. The endpoint
    /// honours the selector for any id, so the page's own choice is
    /// the site player's business and not this client's.
    #[test]
    fn the_sources_endpoint_asks_for_the_played_family_whatever_the_page_named(
        scheme in "(http|https)",
        host in "[a-z]{2,10}(-[0-9]{1,2})?\\.(buzz|site|to)",
        path in "/[a-z0-9/_-]{0,30}",
        // The networks the site names, and the ones it does not: a
        // generator of bare letters would name `tcdn` about never.
        family in proptest::option::of(prop_oneof![
            Just("tcdn".to_string()),
            Just("bcdn".to_string()),
            "[a-z0-9_-]{1,8}",
        ]),
        others in proptest::collection::vec(("[a-rt-z]{1,4}", "[a-z0-9]{0,6}"), 0..3),
        id in 1u64..1_000_000_000,
    ) {
        let mut pairs: Vec<String> = others.iter().map(|(k, v)| format!("{k}={v}")).collect();
        if let Some(family) = &family {
            pairs.push(format!("s={family}"));
        }
        let query = if pairs.is_empty() {
            String::new()
        } else {
            format!("?{}", pairs.join("&"))
        };
        let embed = format!("{scheme}://{host}{path}{query}");
        let built = sources_url(&embed, id).expect("an http(s) embed URL");
        let asked = url::Url::parse(&built).expect("the endpoint is a URL");
        prop_assert_eq!(
            asked.origin().ascii_serialization(),
            format!("{scheme}://{host}"),
            "the endpoint is the page's origin: {}",
            built
        );
        let got: Vec<(String, String)> = asked
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        prop_assert_eq!(
            got,
            vec![("id".to_string(), id.to_string()), ("s".to_string(), "bcdn".to_string())],
            "the query of {}",
            built
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

    /// Every track the response yields has a file within the bound
    /// the hand-offs need, and a file at the bound is kept while one
    /// a byte past it is not: the bound is on the URL as the
    /// response names it, in bytes, and the rows that survive keep
    /// their order.
    #[test]
    fn every_kept_track_file_is_within_the_hand_off_bound(
        master in "https://[a-z]{2,8}\\.example/[a-z0-9/]{1,20}/master\\.m3u8",
        lens in proptest::collection::vec(1usize..2 * SUBTITLE_URL_CAP, 0..6),
    ) {
        let prefix = "https://c.example/subs/";
        let files: Vec<String> = lens
            .iter()
            .map(|&len| format!("{prefix}{}", "a".repeat(len.max(prefix.len() + 1) - prefix.len())))
            .collect();
        let listed: Vec<String> = files
            .iter()
            .map(|file| {
                serde_json::json!({"file": file, "label": "X", "kind": "captions", "default": false})
                    .to_string()
            })
            .collect();
        let json = format!(
            r#"{{"sources":{{"file":"{master}"}},"tracks":[{}]}}"#,
            listed.join(",")
        );
        let payload = parse_sources(&json).expect("the response");
        let kept: Vec<&String> = files.iter().filter(|f| f.len() <= SUBTITLE_URL_CAP).collect();
        prop_assert_eq!(payload.subtitles.len(), kept.len());
        for (got, want) in payload.subtitles.iter().zip(kept) {
            prop_assert!(got.url.len() <= SUBTITLE_URL_CAP, "{} bytes rode into the hand-offs", got.url.len());
            prop_assert_eq!(&got.url, want);
        }
    }

    /// Rows of another shape beside readable ones — `null`, a
    /// string, a number, a list, an object without a file — cost
    /// themselves and nothing else; the readable rows survive in
    /// order, and a tracks field that is not a list at all yields no
    /// tracks and still the stream.
    #[test]
    fn unreadable_track_rows_cost_only_themselves(
        master in "https://[a-z]{2,8}\\.example/[a-z0-9/]{1,20}/master\\.m3u8",
        rows in proptest::collection::vec(
            prop_oneof![
                "[a-z]{3}".prop_map(|code| (Some(code), true)),
                Just((None, false)),
            ],
            0..6,
        ),
        junk in 0u8..5,
    ) {
        let listed: Vec<String> = rows
            .iter()
            .enumerate()
            .map(|(i, (code, _))| match code {
                Some(code) => serde_json::json!({
                    "file": format!("https://c.example/subs/track_{i}_{code}.vtt"),
                    "label": format!("Track {i}"),
                    "kind": "captions",
                })
                .to_string(),
                None => match (i + usize::from(junk)) % 5 {
                    0 => "null".to_string(),
                    1 => "\"junk\"".to_string(),
                    2 => "3".to_string(),
                    3 => "[\"https://c.example/x.vtt\"]".to_string(),
                    _ => r#"{"label":"No file","kind":"captions"}"#.to_string(),
                },
            })
            .collect();
        let json = format!(r#"{{"sources":{{"file":"{master}"}},"tracks":[{}]}}"#, listed.join(","));
        let payload = parse_sources(&json).expect("the stream with its readable rows");
        prop_assert_eq!(&payload.src, &master);
        let expected: Vec<String> = rows
            .iter()
            .filter_map(|(code, _)| code.clone())
            .collect();
        let got: Vec<String> = payload.subtitles.iter().map(|t| t.lang.clone()).collect();
        prop_assert_eq!(got, expected);
        for not_a_list in ["null", "\"none\"", "7", r#"{"file":"https://c.example/x.vtt"}"#] {
            let json = format!(r#"{{"sources":{{"file":"{master}"}},"tracks":{not_a_list}}}"#);
            let payload = parse_sources(&json).expect("the stream without tracks");
            prop_assert_eq!(&payload.src, &master);
            prop_assert!(payload.subtitles.is_empty(), "{not_a_list}");
        }
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

    /// The sources list is read row by row: the stream is the first
    /// row naming a file the transport can fetch, and rows of another
    /// shape around it — `null`, a string, a number, a list, an
    /// object without a file — cost themselves and nothing else. A
    /// list naming no such file is refused, as the encrypted answer
    /// is.
    #[test]
    fn unreadable_source_rows_cost_only_themselves(
        master in "https://[a-z]{2,8}\\.example/[a-z0-9/]{1,20}/master\\.m3u8",
        before in proptest::collection::vec(0u8..8, 0..4),
        after in proptest::collection::vec(
            prop_oneof![(0u8..8).prop_map(Some), Just(None)],
            0..6,
        ),
    ) {
        let mut rows: Vec<String> = before.iter().map(|n| unfetchable_source_row(*n)).collect();
        rows.push(serde_json::json!({"file": master.clone()}).to_string());
        for (i, row) in after.iter().enumerate() {
            rows.push(match row {
                Some(n) => unfetchable_source_row(*n),
                None => serde_json::json!({
                    "file": format!("https://c.example/later/{i}/master.m3u8"),
                })
                .to_string(),
            });
        }
        let json = format!(r#"{{"sources":[{}],"tracks":[]}}"#, rows.join(","));
        let payload = parse_sources(&json).expect("the first fetchable row");
        prop_assert_eq!(&payload.src, &master);

        let junk: Vec<String> = before
            .iter()
            .chain(after.iter().filter_map(Option::as_ref))
            .map(|n| unfetchable_source_row(*n))
            .collect();
        let json = format!(r#"{{"sources":[{}],"tracks":[]}}"#, junk.join(","));
        let refused = matches!(parse_sources(&json), Err(AniError::ParseFailed { .. }));
        prop_assert!(refused, "a list naming no fetchable file: {json}");
    }

    /// Whatever stream the site names, the answer it encrypts comes
    /// back naming exactly that: the response carries nothing in the
    /// clear but its tracks, and the file is read out of the
    /// ciphertext.
    #[test]
    fn an_encrypted_answer_round_trips_the_stream_it_names(
        host in "[a-z]{2,10}\\.[a-z]{2,5}",
        path in "(/[a-z0-9]{1,8}){1,4}",
        tracks in 0usize..3,
    ) {
        let master = format!("https://{host}{path}/master.m3u8");
        let enc = encrypted(&serde_json::json!({"file": master}).to_string());
        let listed = vec![
            r#"{"file":"https://c.example/subs/track_0_eng.vtt","label":"English","kind":"captions"}"#;
            tracks
        ]
        .join(",");
        let json = format!(r#"{{"tracks":[{listed}],"t":1,"enc":"{enc}"}}"#);
        let payload = parse_sources(&json).expect("the encrypted answer");
        prop_assert_eq!(&payload.src, &master);
        prop_assert_eq!(payload.subtitles.len(), tracks);
    }

    /// A blob that is not the site's ciphertext names no stream and
    /// is refused, whatever it is made of — the client reads the
    /// site's answer, and anything else is the site having changed.
    #[test]
    fn a_blob_that_is_not_the_sites_ciphertext_is_refused(enc in "[A-Za-z0-9_=-]{0,200}") {
        let json = format!(r#"{{"tracks":[],"t":1,"enc":"{enc}"}}"#);
        let refused = matches!(parse_sources(&json), Err(AniError::ParseFailed { .. }));
        prop_assert!(refused, "read as a stream: {}", json);
    }
}
