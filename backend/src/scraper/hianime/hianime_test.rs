use super::*;
use crate::error::AniError;
use crate::scraper::provider::{BrowseHit, EpisodeRef};

// ── search page ─────────────────────────────────────────────────────

/// A search page carries result cards inside `film_list-wrap` and
/// unrelated cards elsewhere — the top-10 widget, the sidebar — in
/// the same `film-detail` markup. Only the result list is the answer.
const SEARCH_PAGE: &str = r##"<html><body>
<section class="block_area block_area_sidebar hianime-top10-widget">
  <div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/one-piece-100" title="One Piece">One Piece</a></h3></div>
</section>
<div class="tab-content"><div class="block_area-content block_area-list film_list film_list-grid film_list-wfeature">
<div class="film_list-wrap">
  <div class="flw-item">
    <div class="film-poster"><div class="tick ltr"><div class="tick-item tick-sub">26</div><div class="tick-item tick-dub">26</div></div></div>
    <div class="film-detail">
      <h3 class="film-name"><a href="https://hianime.at/cowboy-bebop-1281" title="Cowboy Bebop" class="dynamic-name" data-jname="Cowboy Bebop">Cowboy Bebop</a></h3>
      <div class="fd-infor"><span class="fdi-item">TV</span><span class="dot"></span><span class="fdi-item fdi-duration">24m</span></div>
    </div>
  </div>
  <div class="flw-item">
    <div class="film-detail">
      <h3 class="film-name"><a href="https://hianime.at/cowboy-bebop-the-movie-1282" title="Cowboy Bebop: The Movie &amp; Tengoku no Tobira &#039;01" class="dynamic-name">Cowboy Bebop: The Movie</a></h3>
      <div class="fd-infor"><span class="fdi-item">Movie</span><span class="dot"></span><span class="fdi-item fdi-duration">115m</span></div>
    </div>
  </div>
</div>
</div></div>
<div id="main-sidebar">
  <div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/naruto-5" title="Naruto">Naruto</a></h3></div>
</div>
</body></html>"##;

#[test]
fn search_cards_inside_the_result_list_become_hits() {
    let hits = parse_search(SEARCH_PAGE).expect("parsed");
    assert_eq!(
        hits,
        vec![
            BrowseHit {
                slug: "cowboy-bebop-1281".into(),
                title: "Cowboy Bebop".into(),
                kind: Some("TV".into()),
            },
            BrowseHit {
                slug: "cowboy-bebop-the-movie-1282".into(),
                title: "Cowboy Bebop: The Movie & Tengoku no Tobira '01".into(),
                kind: Some("Movie".into()),
            },
        ],
        "the widget and sidebar cards share the markup and are not results"
    );
}

#[test]
fn a_search_page_that_says_no_animes_found_is_an_empty_answer() {
    let page = r#"<html><body><div class="tab-content"><div class="block_area-content block_area-list film_list film_list-grid film_list-wfeature"><p>No animes found.</p></div></div><div id="main-sidebar"></div></body></html>"#;
    assert_eq!(parse_search(page).expect("parsed"), Vec::<BrowseHit>::new());
}

#[test]
fn a_page_without_the_search_shape_is_a_parse_failure() {
    // Zero hits are only an answer when the page shows the search
    // shape; a throttling page, a redirect landing, or a redesign
    // must never read as absence.
    let err =
        parse_search("<html><body><h1>Something else</h1></body></html>").expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[test]
fn slug_id_is_the_trailing_number() {
    assert_eq!(slug_id("cowboy-bebop-1281"), Some(1281));
    assert_eq!(slug_id("cowboy-bebop"), None);
    assert_eq!(slug_id(""), None);
}

// ── entry page ──────────────────────────────────────────────────────

#[test]
fn the_entry_page_year_is_the_aired_start() {
    let page = r#"<div class="item item-title">
        <span class="item-head">Aired:</span>
        <span class="name">Apr 3, 1998 to Apr 24, 1999</span>
    </div>
    <div class="item item-title"><span class="item-head">Duration:</span><span class="name">24m</span></div>"#;
    assert_eq!(parse_detail_year(page), Some(1998));
    assert_eq!(parse_detail_year("<div>Duration: 24m</div>"), None);
    assert_eq!(
        parse_detail_year(r#"<span class="item-head">Aired:</span><span class="name">?</span>"#),
        None,
        "an unannounced date is no hint, not a parse failure"
    );
}

// ── episode listing ─────────────────────────────────────────────────

const EPISODE_LIST: &str = r#"{"status":true,"totalItems":2,"html":"<div class=\"ss-list\">\n<a title=\"Episode 1\"\n   class=\"ssl-item ep-item\"\n   data-number=\"1\"\n   data-id=\"21418\"\n   href=\"https://hianime.at/watch/cowboy-bebop-1281?ep=21418\"><div class=\"ssli-order\">1</div></a>\n<a title=\"Episode 2\" class=\"ssl-item ep-item\" data-number=\"2\" data-id=\"21419\" href=\"https://hianime.at/watch/cowboy-bebop-1281?ep=21419\"></a>\n</div>"}"#;

#[test]
fn an_episode_listing_reads_number_and_id_off_each_ep_item() {
    let eps = parse_episode_list(EPISODE_LIST).expect("parsed");
    assert_eq!(
        eps,
        vec![
            EpisodeRef {
                id: 21418,
                number: 1,
                number2: None,
            },
            EpisodeRef {
                id: 21419,
                number: 2,
                number2: None,
            },
        ]
    );
}

#[test]
fn a_listing_envelope_answering_false_is_the_provider_saying_not_found() {
    // A stale or foreign id: the provider answered, and what it said
    // is "no such entry" — the same not-found shape a dead slug's
    // 404 carries elsewhere, so the picker drops the candidate rather
    // than the walk.
    let err = parse_episode_list(r#"{"status":false}"#).expect_err("refused");
    assert!(matches!(err, AniError::Upstream { status: 404 }), "{err:?}");
}

#[test]
fn a_listing_that_is_not_the_envelope_is_a_parse_failure() {
    let err = parse_episode_list("<html>Just a page</html>").expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

// ── servers ─────────────────────────────────────────────────────────

const SERVERS: &str = r#"{"status":true,"html":"<div class=\"ps_-block ps_-block-sub servers-sub\">\n<div class=\"item server-item\" data-type=\"sub\"\n     data-server-name=\"HD-1\"\n     data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"><a href=\"javascript:;\" class=\"btn\">HD-1</a></div>\n<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9tYWwvMS8xL3N1Yg==\"></div>\n</div>\n<div class=\"ps_-block servers-dub\"><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div></div>"}"#;

#[test]
fn servers_decode_their_embed_urls_and_keep_type_and_name() {
    let servers = parse_servers(SERVERS).expect("parsed");
    assert_eq!(
        servers,
        vec![
            ServerEmbed {
                mode: "sub".into(),
                name: "HD-1".into(),
                embed_url: "https://zokoanime.video/stream/mal/1/1/sub".into(),
            },
            ServerEmbed {
                mode: "sub".into(),
                name: "HD-2".into(),
                embed_url: "https://megaplay.buzz/stream/mal/1/1/sub".into(),
            },
            ServerEmbed {
                mode: "dub".into(),
                name: "HD-1".into(),
                embed_url: "https://zokoanime.video/stream/mal/1/1/dub".into(),
            },
        ]
    );
}

#[test]
fn a_server_whose_hash_is_not_an_embed_url_is_skipped() {
    let json = r#"{"status":true,"html":"<div class=\"server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"!!!\"></div><div class=\"server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"bm90IGEgdXJs\"></div>"}"#;
    assert_eq!(
        parse_servers(json).expect("parsed"),
        Vec::<ServerEmbed>::new()
    );
}

#[test]
fn the_preferred_server_is_hd1_of_the_mode_else_the_first_of_the_mode() {
    let servers = parse_servers(SERVERS).expect("parsed");
    assert_eq!(
        preferred_server(&servers, "sub").map(|s| s.name.as_str()),
        Some("HD-1")
    );
    let without_hd1: Vec<ServerEmbed> = servers
        .iter()
        .filter(|s| s.name != "HD-1")
        .cloned()
        .collect();
    assert_eq!(
        preferred_server(&without_hd1, "sub").map(|s| s.embed_url.as_str()),
        Some("https://megaplay.buzz/stream/mal/1/1/sub")
    );
    assert!(preferred_server(&without_hd1, "dub").is_none());
}

// ── embed page ──────────────────────────────────────────────────────

/// `{"src":"https://hls.example/v/master.m3u8","subtitles":[{"lang":"en",
/// "label":"English","default":true,"src":"https://hls.example/v/subs/en.vtt"}],
/// "skip":null}` under the page's XOR key, base64'd — computed once,
/// outside this crate, so the test pins the scheme instead of
/// restating it.
const EMBED_BLOB: &str = "FFYSGRYPX08KERBdBQtAWwkHBgMAFQMIFEETHhlbDAoGWQAfTAhXWE4TQ1YSHhdZDBkOABcPTGoUVg0KG0pHV0AACg9aEwMVAw4ZD19PJwsDQR9CB1ZNSRFIAwwXCRAPTEUdAQRHV14XDkBfRkUCRR8HW0RaRQkeTAAcTBtBAxFOHVpeEA8RSgFDWEcbAEMWKAFHHgkMFA9MXxoYDRY=";

#[test]
fn the_embed_payload_decodes_from_the_page_blob() {
    let page = format!(
        r#"<!doctype html><html><body><div id="player"></div><script>window.__P="{EMBED_BLOB}"</script><script type="module" src="/player.js?v=13"></script></body></html>"#
    );
    let payload = decode_embed(&page).expect("decoded");
    assert_eq!(payload.src, "https://hls.example/v/master.m3u8");
    assert_eq!(
        payload.subtitles,
        vec![SubtitleTrack {
            lang: "en".into(),
            label: "English".into(),
            default: true,
            src: "https://hls.example/v/subs/en.vtt".into(),
        }]
    );
}

#[test]
fn an_embed_page_without_the_blob_is_no_payload() {
    assert!(decode_embed("<html><body>Player</body></html>").is_none());
    assert!(
        decode_embed(r#"<script>window.__P="not base64 at all!"</script>"#).is_none(),
        "garbage under the marker is a miss, not a panic"
    );
}

#[test]
fn the_embed_origin_is_the_referer_the_cdn_wants() {
    assert_eq!(
        embed_origin("https://zokoanime.video/stream/mal/1/1/sub").as_deref(),
        Some("https://zokoanime.video/")
    );
    assert_eq!(embed_origin("not a url"), None);
}
