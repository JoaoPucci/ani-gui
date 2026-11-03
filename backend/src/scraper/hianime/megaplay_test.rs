//! The megaplay reader's pure halves: the media id off the embed page,
//! the sources response, and the language a track file names.

use super::megaplay::{media_id, sources_url};
use super::megaplay_sources::{lang_of_track, parse_sources, unread_languages};
use crate::error::AniError;
use crate::scraper::provider::{SubtitleTrack, SUBTITLE_URL_CAP};

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

/// The sources response as the site answers it now, captured for the
/// `bcdn` family: the tracks still in the clear beside the player's
/// own fields, and the stream behind `enc` — the site's own ciphertext
/// as it sent it.
const ENCRYPTED_SOURCES: &str = r#"{"tracks":[{"file":"https:\/\/xdw5v.qeltrix.top\/anime\/e3b259d82783ba2b947a2045336fd83f\/df9fb06b1a40fec2bff6d81935be43ac\/subtitles\/b70df979097c54ec51cee3d70a2caff8.vtt","label":"English","kind":"captions","default":true}],"t":1,"intro":{"start":0,"end":0},"outro":{"start":0,"end":0},"server":4,"enc":"wdeBruh3qqn_i5wUNnyaPbW9ZonPa4hISsp_E11QSCt-p7b4jKNpJTcgzfjSKpBqtWdiKZBjpuWdrtu6hfbb9xCln-QZ4nGg794RziNjbdkEwk5I-2LhL4KfxH1xwwyI_T75jGmJoQAJDTsVCwoYfmHZfWbJG6y5F7jJzrx3anE"}"#;

/// The stream [`ENCRYPTED_SOURCES`] carries, as the site's own
/// constants open it.
const ENCRYPTED_MASTER: &str = "https://ncdn.imgnex.top/anime/e3b259d82783ba2b947a2045336fd83f/df9fb06b1a40fec2bff6d81935be43ac/master.m3u8";

/// The same episode's answer for the `tcdn` family, captured beside
/// it: a different family, a different host, the same episode.
const ENCRYPTED_TCDN: &str = "wdeBruh3qqn_i5wUNnyaPT1vbSfhRv0ywMEUQK071B57U3YozZ3Fl0uKhDidhqiVpK-w8ngmjIO4TFza--XXzGU_Vow3XzwBzVltr8l6JDkuZKb0fG-R33MSYpqWYtlUAR6o9rqMYNPCpUGg2lt3HTLhtsLpuX28l0kEyKoNzyE";

/// And for the family a page names by naming none.
const ENCRYPTED_DEFAULT: &str = "wdeBruh3qqn_i5wUNnyaPcXqidp1UWP84FfPHzGyKXB9t8vPEIVC4pzNKaYSdv4CsPtBmgcAg6LHMDWWU1Fr1kCSYgRX2bXpI1D6vr0gIfkxncQu55cKclFUyTrpSt2LehXpciFyNV7zA38NDeSPRxP3K6n-1TelVwmNhH_xEaI";

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
        Some("https://megaplay.buzz/stream/getSourcesNew?id=179411&s=bcdn")
    );
    assert_eq!(
        sources_url("https://megaplay-1.buzz/stream/s-2/734292/sub", 7).as_deref(),
        Some("https://megaplay-1.buzz/stream/getSourcesNew?id=7&s=bcdn"),
        "a mirror's page asks the mirror"
    );
    assert_eq!(sources_url("not a url", 1), None);
    assert_eq!(sources_url("ftp://megaplay.buzz/x", 1), None);
}

/// The site streams an episode from more than one content delivery
/// network and picks between them with the `s` of the request, which
/// the endpoint honours for any media id: the id names the episode,
/// the selector names where it streams from. The listing hands out a
/// server per network and its player asks for the one its own page
/// was made for, but the client is not that player, and only one of
/// the networks serves it a stream it can play. So every request
/// asks for that one, whatever the page said — a page with no
/// selector, the selector for the network whose segments arrive
/// wrapped as images, or the right one already.
#[test]
fn every_sources_request_asks_for_the_one_family_the_client_can_play() {
    for page in [
        "https://megaplay.buzz/stream/s-2/474448/sub",
        "https://megaplay.buzz/stream/s-2/474448/sub?s=tcdn",
        "https://megaplay.buzz/stream/s-2/474448/sub?s=bcdn",
        "https://megaplay.buzz/stream/s-2/474448/sub?autoplay=1&s=tcdn&t=5",
    ] {
        assert_eq!(
            sources_url(page, 179_572).as_deref(),
            Some("https://megaplay.buzz/stream/getSourcesNew?id=179572&s=bcdn"),
            "{page}"
        );
    }
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
                lang: "en".into(),
                label: "English".into(),
                default: true,
                url: "https://fetch.nexabloom.top/anime/ba3d/c109/subtitles/track_0_eng.vtt".into(),
            },
            SubtitleTrack {
                lang: "es-419".into(),
                label: "Spanish (Latin American)".into(),
                default: false,
                url: "https://fetch.nexabloom.top/anime/ba3d/c109/subtitles/track_2_Latin_American_spa.vtt".into(),
            },
        ],
        "a thumbnails track is not a subtitle"
    );
}

/// The site stopped answering the stream in the clear: the response
/// carries it as ciphertext under `enc`, opened by two constants the
/// site's own player script holds. The tracks are still in the clear
/// and are read as they were, so an answer in the new shape reads
/// whole.
#[test]
fn an_encrypted_sources_answer_names_the_stream_and_keeps_its_tracks() {
    let payload = parse_sources(ENCRYPTED_SOURCES).expect("the encrypted answer");
    assert_eq!(payload.src, ENCRYPTED_MASTER);
    assert_eq!(
        payload.subtitles,
        vec![SubtitleTrack {
            lang: "en".into(),
            label: "English".into(),
            default: true,
            url: "https://xdw5v.qeltrix.top/anime/e3b259d82783ba2b947a2045336fd83f/df9fb06b1a40fec2bff6d81935be43ac/subtitles/b70df979097c54ec51cee3d70a2caff8.vtt".into(),
        }],
        "the tracks come back in the clear, as they always did"
    );
}

/// One episode, three families, three answers: each names the host
/// its own family streams from. The client reads whichever the page
/// it holds was served for.
#[test]
fn each_cdn_familys_answer_names_its_own_hosts_stream() {
    let answer = |enc: &str| format!(r#"{{"tracks":[],"t":1,"server":4,"enc":"{enc}"}}"#);
    assert_eq!(
        parse_sources(&answer(ENCRYPTED_TCDN)).expect("the tcdn answer").src,
        "https://megap.norami.top/e3b259d82783ba2b947a2045336fd83f/df9fb06b1a40fec2bff6d81935be43ac/master.m3u8"
    );
    assert_eq!(
        parse_sources(&answer(ENCRYPTED_DEFAULT)).expect("the default family's answer").src,
        "https://fetch.nexabloom.top/anime/e3b259d82783ba2b947a2045336fd83f/df9fb06b1a40fec2bff6d81935be43ac/master.m3u8"
    );
}

/// The plaintext takes the shapes the clear answer takes: one object
/// naming the file, or a list of them read by its first fetchable
/// row.
#[test]
fn a_decrypted_list_is_read_by_its_first_fetchable_file() {
    let listed = "4mimZJ24B5ja1Rhc0XNRstjtOP2YJb7BKi2VYwDJY1B1HmlYwZa4XN_3vNApc6Wie8Z6q7Yk3wmn1JP6krMBUR_5OiaR-q6qRmf_GZ5GvFa4LsFevF5zfulgpi82DHwt";
    let json = format!(r#"{{"tracks":[],"enc":"{listed}"}}"#);
    assert_eq!(
        parse_sources(&json).expect("the decrypted list").src,
        "https://cdn.example/a/master.m3u8"
    );
}

/// A blob the client cannot turn into a stream is the site having
/// changed what it hands the client — the constants rotated, the
/// shape moved, the file written some other way — and each failure
/// says which of those it was. None of them is an episode without a
/// stream.
#[test]
fn an_encrypted_sources_answer_the_client_cannot_open_is_a_parse_failure() {
    let refused = |enc: &str| {
        let json = format!(r#"{{"tracks":[],"enc":"{enc}"}}"#);
        match parse_sources(&json) {
            Err(AniError::ParseFailed { detail }) => detail,
            other => panic!("read {enc} as {other:?}"),
        }
    };
    assert!(
        refused("not base64 at all!!").contains("are not base64"),
        "the site wrote it in some other encoding"
    );
    assert!(
        refused("wdeBruh3qqn").contains("are not base64"),
        "a ciphertext cut short is not a whole symbol of the alphabet either"
    );
    assert!(
        refused("wdeBruh3qqk").contains("not whole cipher blocks"),
        "eight bytes is not a block"
    );
    assert!(
        refused("").contains("not whole cipher blocks"),
        "an empty blob is no blocks"
    );
    assert!(
        refused("Hdx9FqJlWeU_4IMNLDvBGq-ipwUHfyQJURkKmoza3uLGJ6rgOnVjqi9bgb-1PfXz")
            .contains("do not open under the site's key"),
        "whole blocks the constants do not open"
    );
    assert!(
        refused("bNZ7eKnXPGoGKh8IRRw3ZTNTgqbtQ6DTcrCdnjjlBNI").contains("are not the sources"),
        "opens to something that is not the sources"
    );
    assert!(
        refused("HaeVwccBGwqLg_DDjjUjy5uzm7W_G5-cAbRAx4HcmBQ")
            .contains("name no source the transport can fetch"),
        "opens to a file the transport cannot fetch"
    );
}

/// The clear shape wins where the site still answers in it: an
/// answer carrying both is read by its `sources`, and the ciphertext
/// beside them is never opened.
#[test]
fn a_sources_field_that_names_a_stream_is_read_over_the_ciphertext() {
    let json = format!(
        r#"{{"sources":{{"file":"https://cdn.example/clear/master.m3u8"}},"tracks":[],"enc":"{ENCRYPTED_TCDN}"}}"#
    );
    assert_eq!(
        parse_sources(&json).expect("the clear shape").src,
        "https://cdn.example/clear/master.m3u8"
    );
}

/// The ciphertext field is read row by row like the rest of the
/// response: one of a shape the reader does not know — a number, an
/// object, a list — costs the ciphertext and nothing else. An answer
/// naming a stream in the clear beside it is still read by its
/// `sources`, and one naming no stream anywhere is refused for the
/// stream it lacks.
#[test]
fn a_ciphertext_field_of_another_shape_costs_only_itself() {
    for enc in ["123", "true", r#"{"file":"x"}"#, r#"["abc"]"#] {
        let json = format!(
            r#"{{"sources":{{"file":"https://cdn.example/clear/master.m3u8"}},"tracks":[],"enc":{enc}}}"#
        );
        assert_eq!(
            parse_sources(&json)
                .unwrap_or_else(|e| panic!("the clear shape beside enc={enc}: {e}"))
                .src,
            "https://cdn.example/clear/master.m3u8"
        );
        let json = format!(r#"{{"tracks":[],"t":1,"enc":{enc}}}"#);
        let err = parse_sources(&json)
            .expect_err("no stream anywhere")
            .to_string();
        assert!(
            err.contains("no source the transport can fetch"),
            "enc={enc} refused for: {err}"
        );
    }
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
    // Nothing in the clear and a ciphertext that does not open: the
    // site changed what it hands the client, which is never an
    // episode without a stream.
    let unopenable = r#"{"tracks":[],"t":1,"intro":{"start":0,"end":0},"outro":{"start":0,"end":0},"server":4,"enc":"wdeBruh3qqn"}"#;
    assert!(matches!(
        parse_sources(unopenable),
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
    assert_eq!(payload.subtitles[0].lang, "de");
    assert_eq!(payload.subtitles[0].label, "German");
}

/// A track's file is held to the length the hand-offs can carry as
/// well as to the scheme the transport can fetch. Every track URL
/// rides on the external player's and Syncplay's command lines, and
/// the boundary the episode is held to afterwards counts tracks, not
/// their length: one file longer than any a CDN signs would fail the
/// hand-off for an episode whose stream is perfectly good. A file at
/// the bound is kept and one a byte past it is dropped, the row and
/// not the response, as the payload reader on the other host does.
#[test]
fn a_track_whose_file_runs_past_the_hand_off_bound_is_left_out() {
    let prefix = "https://cdn.example/subs/";
    let suffix = "_eng.vtt";
    let file = |len: usize| {
        format!(
            "{prefix}{}{suffix}",
            "a".repeat(len - prefix.len() - suffix.len())
        )
    };
    let at_cap = file(SUBTITLE_URL_CAP);
    let past_cap = file(SUBTITLE_URL_CAP + 1);
    assert_eq!(at_cap.len(), SUBTITLE_URL_CAP);
    assert_eq!(past_cap.len(), SUBTITLE_URL_CAP + 1);
    let json = format!(
        r#"{{"sources":{{"file":"https://cdn.example/master.m3u8"}},"tracks":[{{"file":"{at_cap}","label":"English","kind":"captions"}},{{"file":"{past_cap}","label":"German","kind":"subtitles"}}]}}"#
    );
    let payload = parse_sources(&json).expect("sources");
    let kept: Vec<&str> = payload.subtitles.iter().map(|t| t.url.as_str()).collect();
    assert_eq!(
        kept,
        vec![at_cap.as_str()],
        "the file a byte past the bound rode into the hand-offs"
    );
}

/// megaplay lists a track's `file`, `label` and `kind`, and no
/// language. The language is a BCP 47 tag read from the code the file
/// name carries, in either naming the site's CDNs have used, or else
/// from the label; a track neither names is `und` — never the label,
/// which is no tag and reached the player as `srclang="English"`.
#[test]
fn the_language_is_a_tag_from_the_files_code_or_else_the_label() {
    assert_eq!(
        lang_of_track("https://c.example/subtitles/track_0_eng.vtt", "English"),
        "en"
    );
    assert_eq!(
        lang_of_track(
            "https://c.example/subtitles/track_2_Latin_American_spa.vtt?x=1",
            "Spanish (Latin American)"
        ),
        "es-419"
    );
    assert_eq!(
        lang_of_track(
            "https://c.example/subtitles/track_3_European_fre.VTT",
            "French"
        ),
        "fr"
    );
    assert_eq!(
        lang_of_track("https://c.example/subtitles/english.vtt", "English"),
        "en",
        "no code in the file name: the label names the language"
    );
    assert_eq!(
        lang_of_track(
            "https://c.example/subtitles/b70df979097c54ec51cee3d70a2caff8.vtt",
            "Portuguese (Portuguese - Brazilian)"
        ),
        "pt-BR",
        "a hashed file name: the label names the language and the region"
    );
    assert_eq!(
        lang_of_track("https://c.example/subtitles/track_0_en.vtt", " Deutsch "),
        "und",
        "neither a code the reader knows nor an English name: unknown, not the label"
    );
    assert_eq!(lang_of_track("https://c.example/x.vtt", "   "), "und");
}

/// The site's other CDN names a track by its code and its place in
/// the listing (`eng-2.vtt`, `por-24.vtt`), with no `track_` prefix.
/// Steel Ball Run's first episode lists 33 tracks named that way; read
/// by the underscore rule alone, every one took its label as its
/// language, and the player was handed `srclang="Portuguese (- Brazil)"`.
#[test]
fn a_code_followed_by_the_tracks_place_is_the_language() {
    let at = |name: &str, label: &str| {
        lang_of_track(
            &format!("https://de3bv.example/anime/a/b/subtitles/{name}"),
            label,
        )
    };
    // A label the reader does not know, so the file name alone speaks.
    assert_eq!(at("eng-2.vtt", "Track"), "en");
    assert_eq!(at("por-24.vtt", "Track"), "pt");
    assert_eq!(at("ara-3.vtt?sig=abc", "Track"), "ar");
    assert_eq!(at("chi-4.VTT", "Track"), "zh");
    assert_eq!(at("track_0_eng-7.vtt", "Track"), "en");
    // Only a run of digits is a place in the listing.
    assert_eq!(at("eng-.vtt", "Track"), "und");
    assert_eq!(at("eng-2a.vtt", "Track"), "und");
    assert_eq!(at("eng-2-3.vtt", "Track"), "und");
    // And only three lowercase letters ahead of it are a code.
    assert_eq!(at("english-2.vtt", "Track"), "und");
    assert_eq!(at("ENG-2.vtt", "Track"), "und");
    assert_eq!(at("en-2.vtt", "Track"), "und");
    assert_eq!(at("-2.vtt", "   "), "und");
}

/// Every label the two listings sampled live carry, with its file
/// name: Steel Ball Run's first episode (33 tracks, `<code>-<n>.vtt`)
/// and Black Clover's (9). The region and the script come from the
/// label, which is the only place the listing says which Portuguese
/// or which Chinese a track is; `SDH` stays in the label the player
/// shows.
#[test]
fn every_label_the_sampled_listings_carry_reads_as_its_tag() {
    let listings = [
        ("ara-3.vtt", "Arabic", "ar"),
        ("chi-4.vtt", "Chinese (- Simplified)", "zh-Hans"),
        ("chi-5.vtt", "Chinese (- Traditional)", "zh-Hant"),
        ("hrv-6.vtt", "Croatian", "hr"),
        ("cze-7.vtt", "Czech", "cs"),
        ("dan-8.vtt", "Danish", "da"),
        ("dut-9.vtt", "Dutch", "nl"),
        ("eng-2.vtt", "English", "en"),
        ("fin-11.vtt", "Finnish", "fi"),
        ("fre-12.vtt", "French", "fr"),
        ("ger-13.vtt", "German", "de"),
        ("gre-14.vtt", "Greek", "el"),
        ("heb-15.vtt", "Hebrew", "he"),
        ("hun-16.vtt", "Hungarian", "hu"),
        ("ind-17.vtt", "Indonesian", "id"),
        ("ita-18.vtt", "Italian", "it"),
        ("jpn-19.vtt", "Japanese", "ja"),
        ("kor-20.vtt", "Korean", "ko"),
        ("may-21.vtt", "Malay", "ms"),
        ("nob-22.vtt", "Norwegian (Bokmål)", "nb"),
        ("pol-23.vtt", "Polish", "pl"),
        ("por-24.vtt", "Portuguese (- Brazil)", "pt-BR"),
        ("por-25.vtt", "Portuguese (- Portugal)", "pt-PT"),
        ("rum-26.vtt", "Romanian", "ro"),
        ("rus-27.vtt", "Russian", "ru"),
        ("spa-28.vtt", "Spanish (- Latin America)", "es-419"),
        ("spa-29.vtt", "Spanish (- Latin America | SDH)", "es-419"),
        ("spa-30.vtt", "Spanish (- Spain)", "es-ES"),
        ("swe-31.vtt", "Swedish", "sv"),
        ("tha-32.vtt", "Thai", "th"),
        ("tha-33.vtt", "Thai (- SDH)", "th"),
        ("tur-34.vtt", "Turkish", "tr"),
        ("ukr-35.vtt", "Ukrainian", "uk"),
        ("por-9.vtt", "Portuguese (Portuguese - Brazilian)", "pt-BR"),
        ("spa-7.vtt", "Spanish (- Spanish[ESP])", "es-ES"),
        ("spa-8.vtt", "Spanish (- Spanish[LAT])", "es-419"),
    ];
    for (file, label, tag) in listings {
        assert_eq!(
            lang_of_track(&format!("https://cdn.example/subtitles/{file}"), label),
            tag,
            "{file} under {label:?}"
        );
        assert_eq!(
            lang_of_track("https://cdn.example/subtitles/0f3a9c.vtt", label),
            tag,
            "{label:?} alone, its file naming no code"
        );
    }
}

/// The file's code names the language; a label naming another one
/// lends it no region, since it describes some other track.
#[test]
fn a_label_for_another_language_lends_no_region() {
    assert_eq!(
        lang_of_track(
            "https://c.example/subtitles/por-24.vtt",
            "Spanish (- Spain)"
        ),
        "pt"
    );
}

/// `American` names the United States only for English: a Spanish
/// label calling itself American is read as Spanish, no region.
#[test]
fn american_names_a_region_only_for_english() {
    let at = |label: &str| lang_of_track("https://c.example/subtitles/0f3a9c.vtt", label);
    assert_eq!(at("English (American)"), "en-US");
    assert_eq!(at("Spanish (American)"), "es");
}

/// A listing's tracks whose language the reader could not read are
/// reported together — file and label — so a third naming shows in
/// the log the first time it is played, not as tracks quietly without
/// a language: one warning per listing, logged from this report
/// (`warn_unread_languages`). A listing it reads in full reports
/// nothing. The report is checked rather than the log line: capturing
/// a log line in a test races every other test thread that reaches the
/// same call site, and lost the line under a loaded machine.
#[test]
fn a_listing_with_unread_languages_reports_them_together() {
    let listing = r#"{"sources":{"file":"https://cdn.example/master.m3u8"},"tracks":[
        {"file":"https://cdn.example/subs/eng-2.vtt","label":"English","kind":"captions","default":true},
        {"file":"https://cdn.example/subs/q7.vtt?sig=s3cr3t&e=99","label":"Klingon","kind":"captions"},
        {"file":"https://cdn.example/subs/zz-9.vtt","label":"Track 4","kind":"captions"}]}"#;
    let payload = parse_sources(listing).expect("listing");
    let langs: Vec<&str> = payload.subtitles.iter().map(|t| t.lang.as_str()).collect();
    assert_eq!(langs, ["en", "und", "und"]);
    let labels: Vec<&str> = payload.subtitles.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(
        labels,
        ["English", "Klingon", "Track 4"],
        "the label is kept for display"
    );
    let (count, report) = unread_languages(&payload.subtitles).expect("a report");
    assert_eq!(count, 2);
    for named in ["q7.vtt", "Klingon", "zz-9.vtt", "Track 4"] {
        assert!(report.contains(named), "{named} named in {report:?}");
    }
    assert!(!report.contains("eng-2.vtt"), "a read track is not named");
    assert!(
        !report.contains("s3cr3t"),
        "a signed file's query stays out of the log: {report:?}"
    );

    let read = r#"{"sources":{"file":"https://cdn.example/master.m3u8"},"tracks":[
        {"file":"https://cdn.example/subs/eng-2.vtt","label":"English","kind":"captions"}]}"#;
    let payload = parse_sources(read).expect("listing");
    assert_eq!(unread_languages(&payload.subtitles), None);
}

/// The tracks are a nicety beside the stream: the field being
/// `null`, absent or not a list, or holding rows in another shape,
/// costs those rows and nothing else. A lone megaplay server is not
/// skipped over its subtitle metadata having changed shape.
#[test]
fn a_tracks_field_the_reader_cannot_read_costs_only_those_rows() {
    let stream = "https://cdn.example/master.m3u8";
    for tracks in [
        r#""tracks":null"#,
        r#""tracks":"none""#,
        r#""tracks":7"#,
        r#""tracks":{"file":"https://cdn.example/subs/track_0_eng.vtt"}"#,
    ] {
        let json = format!(r#"{{"sources":{{"file":"{stream}"}},{tracks}}}"#);
        let payload = parse_sources(&json).unwrap_or_else(|e| panic!("{tracks}: {e:?}"));
        assert_eq!(payload.src, stream, "{tracks}");
        assert!(
            payload.subtitles.is_empty(),
            "{tracks}: {:?}",
            payload.subtitles
        );
    }
    let absent = format!(r#"{{"sources":{{"file":"{stream}"}}}}"#);
    let payload = parse_sources(&absent).expect("no tracks field");
    assert!(payload.subtitles.is_empty());
    // Readable rows survive rows of another shape beside them, in
    // the page's order.
    // A list positionally shaped like a track is not a track: serde
    // would read a sequence into the struct, and the row would come
    // through as a caption with a made-up label.
    let mixed = format!(
        r#"{{"sources":{{"file":"{stream}"}},"tracks":[{{"file":"https://cdn.example/subs/track_0_eng.vtt","label":"English","kind":"captions","default":true}},null,"junk",3,{{"label":"No file","kind":"captions"}},{{"file":["https://cdn.example/subs/track_9_x.vtt"],"label":"Wrong shape","kind":"captions"}},["https://cdn.example/subs/track_5_fre.vtt","Sequence","captions",true],{{"file":"https://cdn.example/subs/track_2_ger.vtt","label":"German","kind":"subtitles"}}]}}"#
    );
    let payload = parse_sources(&mixed).expect("readable rows");
    assert_eq!(
        payload.subtitles,
        vec![
            SubtitleTrack {
                lang: "en".into(),
                label: "English".into(),
                default: true,
                url: "https://cdn.example/subs/track_0_eng.vtt".into(),
            },
            SubtitleTrack {
                lang: "de".into(),
                label: "German".into(),
                default: false,
                url: "https://cdn.example/subs/track_2_ger.vtt".into(),
            },
        ],
        "exactly the readable captions rows, in order"
    );
}

/// The sources list is the one place the site hands the client a
/// choice, and the rows it does not read are the ones it has changed:
/// a listed fallback in a shape the reader does not know costs that
/// row and nothing else. Reading the list as a whole would let one
/// such row skip a server whose other rows name a playable stream.
#[test]
fn an_unreadable_source_row_costs_only_itself() {
    let first = "https://cdn.example/a/master.m3u8";
    let json = format!(
        r#"{{"sources":[{{"file":"{first}"}},null,"junk",3,["https://cdn.example/z/master.m3u8"],{{"label":"No file"}},{{"file":["https://cdn.example/y/master.m3u8"]}}],"tracks":[]}}"#
    );
    assert_eq!(
        parse_sources(&json).expect("the readable row's stream").src,
        first,
        "rows of another shape after the first do not cost the stream"
    );
}

/// With the first row unreadable the stream is the first row after it
/// the transport can fetch — the list's order stands, the rows that
/// name nothing fetchable are stepped over.
#[test]
fn a_sources_list_is_read_by_its_first_fetchable_file() {
    let wanted = "https://cdn.example/b/master.m3u8";
    for leading in [
        "null",
        r#""junk""#,
        "3",
        r#"{"label":"No file"}"#,
        r#"{"file":"/a/master.m3u8"}"#,
        r#"{"file":""}"#,
        r#"{"file":["https://cdn.example/a/master.m3u8"]}"#,
    ] {
        let json = format!(
            r#"{{"sources":[{leading},{{"file":"{wanted}"}},{{"file":"https://cdn.example/c/master.m3u8"}}],"tracks":[]}}"#
        );
        let payload = parse_sources(&json).unwrap_or_else(|e| panic!("{leading}: {e:?}"));
        assert_eq!(payload.src, wanted, "{leading}");
    }
}

/// A list naming nothing the transport can fetch is the site having
/// changed what it hands the client, exactly as the encrypted answer
/// and the empty list are.
#[test]
fn a_sources_list_of_rows_the_reader_cannot_use_is_a_parse_failure() {
    for listed in [
        "null",
        r#"null,"junk",3,["https://cdn.example/a/master.m3u8"]"#,
        r#"{"label":"No file"},{"kind":"hls"}"#,
        r#"{"file":"/a/master.m3u8"},{"file":"master.m3u8"},{"file":"ftp://x/y.m3u8"}"#,
    ] {
        let json = format!(r#"{{"sources":[{listed}],"tracks":[]}}"#);
        assert!(
            matches!(parse_sources(&json), Err(AniError::ParseFailed { .. })),
            "{listed}"
        );
    }
}

/// A provider that lists its own `lang` field — the embed payload's —
/// may write a tag, a code, a name or a locale with an underscore. A
/// tag-shaped field stands as listed; anything else is read as a name,
/// or else the label is, so the player's `srclang` is always a tag.
#[test]
fn a_listed_language_field_reads_as_a_tag() {
    use crate::scraper::subtitle_language::listed_tag;
    assert_eq!(listed_tag("en", "English"), "en");
    assert_eq!(listed_tag("pt-BR", "Português"), "pt-BR");
    assert_eq!(
        listed_tag("xx", "Something"),
        "xx",
        "a tag-shaped field stands"
    );
    assert_eq!(listed_tag("pt_BR", "Portuguese"), "pt-BR");
    assert_eq!(listed_tag("English", "English"), "en");
    assert_eq!(listed_tag("Portuguese (Brazil)", "x"), "pt-BR");
    assert_eq!(listed_tag("", "Spanish (- Latin America)"), "es-419");
    assert_eq!(listed_tag("Klingon", "Klingon"), "und");
    assert_eq!(listed_tag("", ""), "und");
}
