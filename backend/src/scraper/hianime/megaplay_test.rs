//! The megaplay reader's pure halves: the media id off the embed page,
//! the sources response, and the language a track file names.

use super::megaplay::{lang_of_track, media_id, parse_sources, sources_url};
use crate::error::AniError;
use crate::scraper::provider::SubtitleTrack;

/// megaplay's embed page as captured on 2026-09-12: no payload in the
/// markup, a player element whose `data-id` is the media id the
/// sources endpoint is keyed on, and the site's other ids beside it.
const MEGAPLAY_PAGE: &str = r#"<!DOCTYPE html><html><head><title>File 179411 - MegaPlay</title>
<script src="https://megaplay.buzz/lib/newclient.min.js?v=4.7"></script></head>
<body><div class="mg-3mb3d"><div class="mg3-player">
<div class="fix-area" id="megaplay-player"
     data-id="179411"
     data-realid="734292"
     data-mediaid="8879"
     data-fileversion="0"><div class="content-center"></div></div>
</div></div></body></html>"#;

/// The sources response as captured on 2026-09-12: one master, six
/// captions tracks whose file names carry the language code, and the
/// player's own fields beside them.
const SOURCES: &str = r#"{"sources":{"file":"https://ncdn.imgnex.top/anime/ba3d/c109/master.m3u8"},"tracks":[{"file":"https://fetch.nexabloom.top/anime/ba3d/c109/subtitles/track_0_eng.vtt","label":"English","kind":"captions","default":true},{"file":"https://fetch.nexabloom.top/anime/ba3d/c109/subtitles/track_2_Latin_American_spa.vtt","label":"Spanish (Latin American)","kind":"captions"},{"file":"https://fetch.nexabloom.top/anime/ba3d/c109/thumbnails.vtt","kind":"thumbnails"}],"t":1,"intro":{"start":0,"end":0},"outro":{"start":0,"end":0},"server":4}"#;

#[test]
fn the_media_id_is_the_player_elements_data_id() {
    assert_eq!(media_id(MEGAPLAY_PAGE), Some(179_411));
}

#[test]
fn the_media_id_is_read_whatever_the_attribute_order() {
    let page =
        r#"<div data-realid="734292" data-id="42" id="megaplay-player" data-mediaid="8879">"#;
    assert_eq!(media_id(page), Some(42));
}

#[test]
fn a_page_without_the_player_element_carries_no_media_id() {
    // Another host's page — a title and a player script, no element
    // the reader knows — or a page whose element lost its id.
    assert_eq!(
        media_id(
            r#"<html><head><title>File 143764 - MegaPlay</title></head><body><div id="player"></div></body></html>"#
        ),
        None
    );
    assert_eq!(
        media_id(r#"<div id="megaplay-player" data-realid="734292" data-mediaid="8879">"#),
        None,
        "the element without its data-id names no media"
    );
    assert_eq!(
        media_id(r#"<div id="megaplay-player" data-id="">"#),
        None,
        "a blank id names no media"
    );
    assert_eq!(media_id(""), None);
}

#[test]
fn the_sources_endpoint_is_on_the_embed_pages_origin() {
    assert_eq!(
        sources_url(
            "https://megaplay.buzz/stream/s-2/734292/sub?s=bcdn",
            179_411
        )
        .as_deref(),
        Some("https://megaplay.buzz/stream/getSourcesNew?id=179411")
    );
    assert_eq!(
        sources_url("https://megaplay-1.buzz/stream/s-2/734292/sub", 7).as_deref(),
        Some("https://megaplay-1.buzz/stream/getSourcesNew?id=7"),
        "a mirror's page asks the mirror"
    );
    assert_eq!(sources_url("not a url", 1), None);
    assert_eq!(sources_url("ftp://megaplay.buzz/x", 1), None);
}

#[test]
fn the_sources_response_yields_the_master_and_the_captions_tracks() {
    let payload = parse_sources(SOURCES).expect("sources");
    assert_eq!(
        payload.src,
        "https://ncdn.imgnex.top/anime/ba3d/c109/master.m3u8"
    );
    assert_eq!(
        payload.subtitles,
        vec![
            SubtitleTrack {
                lang: "eng".into(),
                label: "English".into(),
                default: true,
                url: "https://fetch.nexabloom.top/anime/ba3d/c109/subtitles/track_0_eng.vtt".into(),
            },
            SubtitleTrack {
                lang: "spa".into(),
                label: "Spanish (Latin American)".into(),
                default: false,
                url: "https://fetch.nexabloom.top/anime/ba3d/c109/subtitles/track_2_Latin_American_spa.vtt".into(),
            },
        ],
        "a thumbnails track is not a subtitle"
    );
}

#[test]
fn a_sources_list_is_read_by_its_first_file() {
    let json = r#"{"sources":[{"file":"https://cdn.example/a/master.m3u8"},{"file":"https://cdn.example/b/master.m3u8"}],"tracks":[]}"#;
    assert_eq!(
        parse_sources(json).expect("sources").src,
        "https://cdn.example/a/master.m3u8"
    );
}

#[test]
fn a_response_without_a_usable_source_is_a_parse_failure() {
    // The older endpoint answers the sources encrypted and `null`
    // in the clear: the site changed what it hands the client, which
    // is never an episode without a stream.
    let old = r#"{"tracks":[],"t":1,"intro":{"start":0,"end":0},"outro":{"start":0,"end":0},"server":4,"enc":"wdeBruh3qqn"}"#;
    assert!(matches!(
        parse_sources(old),
        Err(AniError::ParseFailed { .. })
    ));
    let null = r#"{"sources":null,"tracks":[]}"#;
    assert!(matches!(
        parse_sources(null),
        Err(AniError::ParseFailed { .. })
    ));
    let relative = r#"{"sources":{"file":"/anime/x/master.m3u8"},"tracks":[]}"#;
    assert!(matches!(
        parse_sources(relative),
        Err(AniError::ParseFailed { .. })
    ));
    let blank = r#"{"sources":{"file":""},"tracks":[]}"#;
    assert!(matches!(
        parse_sources(blank),
        Err(AniError::ParseFailed { .. })
    ));
    let empty_list = r#"{"sources":[],"tracks":[]}"#;
    assert!(matches!(
        parse_sources(empty_list),
        Err(AniError::ParseFailed { .. })
    ));
    assert!(matches!(
        parse_sources("<html>Just a moment...</html>"),
        Err(AniError::ParseFailed { .. })
    ));
}

#[test]
fn a_track_the_transport_cannot_fetch_is_left_out() {
    let json = r#"{"sources":{"file":"https://cdn.example/master.m3u8"},"tracks":[{"file":"/subs/track_0_eng.vtt","label":"English","kind":"captions"},{"file":"https://cdn.example/subs/track_1_ger.vtt","label":"German","kind":"subtitles"}]}"#;
    let payload = parse_sources(json).expect("sources");
    assert_eq!(payload.subtitles.len(), 1);
    assert_eq!(payload.subtitles[0].lang, "ger");
    assert_eq!(payload.subtitles[0].label, "German");
}

#[test]
fn the_language_is_the_files_trailing_code_or_else_the_label() {
    assert_eq!(
        lang_of_track("https://c.example/subtitles/track_0_eng.vtt", "English"),
        "eng"
    );
    assert_eq!(
        lang_of_track(
            "https://c.example/subtitles/track_2_Latin_American_spa.vtt?x=1",
            "Spanish"
        ),
        "spa"
    );
    assert_eq!(
        lang_of_track(
            "https://c.example/subtitles/track_3_European_fre.VTT",
            "French"
        ),
        "fre"
    );
    assert_eq!(
        lang_of_track("https://c.example/subtitles/english.vtt", "English"),
        "English",
        "no trailing code: the label stands in"
    );
    assert_eq!(
        lang_of_track("https://c.example/subtitles/track_0_en.vtt", " Deutsch "),
        "Deutsch",
        "a two-letter tail is not the site's code"
    );
    assert_eq!(lang_of_track("https://c.example/x.vtt", "   "), "und");
}
