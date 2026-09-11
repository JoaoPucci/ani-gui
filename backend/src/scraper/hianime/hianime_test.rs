use super::*;
use crate::error::AniError;
use crate::scraper::provider::{BrowseHit, EpisodeRef, SubtitleTrack};

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

/// Each result card leads with its poster link — `href` and `title`
/// of its own — ahead of the detail block the parser reads.
const SEARCH_PAGE_WITH_POSTER_LINKS: &str = r##"<html><body>
<div class="film_list-wrap">
  <div class="flw-item">
    <div class="film-poster"><a href="https://hianime.at/watch/cowboy-bebop-1281" class="film-poster-ahref item-qtip" title="Cowboy Bebop"></a></div>
    <div class="film-detail">
      <h3 class="film-name"><a href="https://hianime.at/cowboy-bebop-1281" title="Cowboy Bebop" class="dynamic-name">Cowboy Bebop</a></h3>
      <div class="fd-infor"><span class="fdi-item">TV</span></div>
    </div>
  </div>
  <div class="flw-item">
    <div class="film-poster"><a href="https://hianime.at/watch/unreadable-7" class="film-poster-ahref item-qtip" title="Unreadable"></a></div>
    <div class="film-detail">
      <h3 class="film-name"><span data-slug="unreadable-7">Unreadable</span></h3>
    </div>
  </div>
  <div class="flw-item">
    <div class="film-poster"><a href="https://hianime.at/watch/cowboy-bebop-the-movie-1282" class="film-poster-ahref item-qtip" title="Cowboy Bebop: The Movie"></a></div>
    <div class="film-detail">
      <h3 class="film-name"><a href="https://hianime.at/cowboy-bebop-the-movie-1282" title="Cowboy Bebop: The Movie" class="dynamic-name">Cowboy Bebop: The Movie</a></h3>
      <div class="fd-infor"><span class="fdi-item">Movie</span></div>
    </div>
  </div>
</div>
<div id="main-sidebar"></div>
</body></html>"##;

#[test]
fn a_card_the_parser_cannot_read_is_skipped_and_its_neighbour_is_read_once() {
    // A card whose detail block has no anchor must not borrow the
    // next card's poster link as its identity, and the next card must
    // not come back twice: the hits are exactly the readable cards,
    // each with its own slug and title.
    let hits = parse_search(SEARCH_PAGE_WITH_POSTER_LINKS).expect("parsed");
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
                title: "Cowboy Bebop: The Movie".into(),
                kind: Some("Movie".into()),
            },
        ]
    );
}

/// A card whose title is blank names nothing: picked, it would put
/// an empty title on the resolve, the download's file name and the
/// progress copy, and match no title the user typed. Skipped like an
/// unreadable card; a listing of only such cards is refused.
#[test]
fn a_card_whose_title_is_blank_is_skipped_and_a_listing_of_such_cards_is_refused() {
    let card = |slug: &str, title: &str| {
        format!(
            r#"<div class="flw-item"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/{slug}" title="{title}" class="dynamic-name">x</a></h3><div class="fd-infor"><span class="fdi-item">TV</span></div></div></div>"#
        )
    };
    let page = |cards: String| {
        format!(r#"<html><body><div class="film_list-wrap">{cards}</div></body></html>"#)
    };
    let mixed = page(format!(
        "{}{}",
        card("nameless-1", "   "),
        card("cowboy-bebop-1281", "Cowboy Bebop")
    ));
    let hits = parse_search(&mixed).expect("the readable card");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].slug, "cowboy-bebop-1281");
    let all_blank = page(format!(
        "{}{}",
        card("nameless-1", ""),
        card("nameless-2", " \t ")
    ));
    let err = parse_search(&all_blank).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}
#[test]
fn a_card_whose_slug_carries_no_id_is_skipped_and_a_listing_of_such_cards_is_refused() {
    // The episode listing is keyed on the decimal tail of a slug; a
    // card without one cannot be resolved, and picked — as the
    // count-less pick would — it fails the walk where a later card
    // would have played. It is skipped like an unreadable card, and a
    // listing of nothing else is a changed shape.
    let mixed = r#"<html><body><div class="film_list-wrap"><div class="flw-item"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/cowboy-bebop" title="Cowboy Bebop">Cowboy Bebop</a></h3></div></div><div class="flw-item"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/cowboy-bebop-1281" title="Cowboy Bebop">Cowboy Bebop</a></h3></div></div></div><div id="main-sidebar"></div></body></html>"#;
    let hits = parse_search(mixed).expect("parsed");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].slug, "cowboy-bebop-1281");
    let unresolvable_only = r#"<html><body><div class="film_list-wrap"><div class="flw-item"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/cowboy-bebop" title="Cowboy Bebop">Cowboy Bebop</a></h3></div></div></div><div id="main-sidebar"></div></body></html>"#;
    let err = parse_search(unresolvable_only).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
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
fn a_result_list_whose_cards_all_fail_to_parse_is_a_parse_failure() {
    // The cards are there and the title anchor is not what the parser
    // knows: read as "no results", every title searched would be
    // persisted as absent.
    let page = r#"<html><body><div class="film_list-wrap"><div class="flw-item"><div class="film-detail"><h3 class="film-name"><span data-slug="cowboy-bebop-1281" data-title="Cowboy Bebop">Cowboy Bebop</span></h3></div></div></div><div id="main-sidebar"></div></body></html>"#;
    let err = parse_search(page).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let some_readable = r#"<html><body><div class="film_list-wrap"><div class="flw-item"><div class="film-detail"><h3 class="film-name"><span data-slug="x">X</span></h3></div></div><div class="flw-item"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/cowboy-bebop-1281" title="Cowboy Bebop">Cowboy Bebop</a></h3></div></div></div><div id="main-sidebar"></div></body></html>"#;
    assert_eq!(
        parse_search(some_readable).expect("parsed").len(),
        1,
        "a readable card beside an unreadable one is kept"
    );
    let no_cards = r#"<html><body><div class="film_list-wrap"></div><div id="main-sidebar"></div></body></html>"#;
    assert_eq!(
        parse_search(no_cards).expect("parsed"),
        Vec::<BrowseHit>::new(),
        "a result region with no cards at all is the empty answer"
    );
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
fn a_server_whose_hash_is_not_an_embed_url_is_skipped_beside_one_that_is() {
    let json = r#"{"status":true,"html":"<div class=\"server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"!!!\"></div><div class=\"server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9tYWwvMS8xL3N1Yg==\"></div>"}"#;
    let servers = parse_servers(json).expect("parsed");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "HD-2");
}

/// A listing can be readable for one mode and not the other: a dub
/// row that decodes beside sub rows whose hashes no longer do. Read as
/// "no sub", the mode probe would persist an absence over the sub
/// playback the site still lists, so the listing keeps, per mode,
/// whether a row was present but unreadable, and a mode with no
/// readable server answers a parse failure when one was.
#[test]
fn a_mode_whose_rows_were_all_unreadable_is_uncertain_not_absent() {
    let json = r#"{"status":true,"html":"<div class=\"server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"!!!\"></div><div class=\"server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(json).expect("the dub row is readable");
    assert_eq!(listing.servers.len(), 1);
    assert_eq!(listing.servers[0].mode, "dub");
    assert_eq!(listing.unreadable_modes, vec!["sub".to_string()]);
    assert!(
        matches!(
            listing.mode_readable("sub"),
            Err(AniError::ParseFailed { .. })
        ),
        "a mode with rows the client could not read is not absent"
    );
    assert!(listing.mode_readable("dub").expect("read"));
    assert!(parse_server_listing(SERVERS)
        .expect("parsed")
        .mode_readable("dub")
        .expect("read"));
    let sub_only = parse_server_listing(
        r#"{"status":true,"html":"<div class=\"server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#,
    )
    .expect("parsed");
    assert!(
        !sub_only.mode_readable("dub").expect("read"),
        "a mode the listing carries no row of is absent"
    );
}

#[test]
fn a_listing_whose_rows_all_fail_to_parse_is_a_parse_failure() {
    // The marker survived a redesign but the attributes did not: zero
    // rows out of a nonempty listing is the site having changed shape,
    // and read as "none" it hides a playable show exactly as a missing
    // marker did.
    let servers = r#"{"status":true,"html":"<div class=\"server-item\" data-kind=\"sub\" data-name=\"HD-1\" data-ref=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#;
    let err = parse_servers(servers).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let hashes = r#"{"status":true,"html":"<div class=\"server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"!!!\"></div><div class=\"server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"bm90IGEgdXJs\"></div>"}"#;
    let err = parse_servers(hashes).expect_err("refused");
    assert!(
        matches!(err, AniError::ParseFailed { .. }),
        "hashes that all fail to decode are a changed format: {err:?}"
    );
    let episodes = r#"{"status":true,"html":"<a class=\"ssl-item ep-item\" data-num=\"1\" data-ref=\"21418\"></a>"}"#;
    let err = parse_episode_list(episodes).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[test]
fn an_empty_server_list_is_the_provider_answering_no_servers() {
    assert_eq!(
        parse_servers(r#"{"status":true,"html":""}"#).expect("answered"),
        Vec::<ServerEmbed>::new()
    );
}

#[test]
fn a_nonempty_server_list_with_no_recognizable_rows_is_a_parse_failure() {
    // The site changed shape: read as "no servers", has_mode says
    // false and the mode probe persists an absence that hides a
    // playable show for the negative TTL. Weather, not a verdict.
    let json = r#"{"status":true,"html":"<div class=\"ps_-block\"><div class=\"item srv\" data-kind=\"sub\" data-ref=\"x\"></div></div>"}"#;
    let err = parse_servers(json).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[test]
fn a_server_row_with_a_blank_mode_is_not_a_usable_row() {
    // A row that keeps its type attribute but empties it would pass
    // as parsed, has_mode would find no sub or dub, and the probe
    // would persist an absence over a listing it did not understand.
    let blank_only = r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"  \" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9tYWwvMS8xL3N1Yg==\"></div>"}"#;
    let err = parse_servers(blank_only).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let mixed = r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\" \" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9tYWwvMS8xL3N1Yg==\"></div>"}"#;
    let servers = parse_servers(mixed).expect("the usable row");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].mode, "sub");
    assert_eq!(servers[0].name, "HD-2");
}

#[test]
fn a_server_row_with_a_mode_the_client_does_not_know_is_not_a_usable_row() {
    // The site types its servers sub or dub, and the mode probe asks
    // for exactly those. A row typed some other way — the values
    // renamed — would pass as parsed while has_mode found neither,
    // and the probe would persist an absence over a listing it did
    // not understand. Such a row is not usable; a listing of nothing
    // else is a changed shape.
    let unsupported_only = r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"softsub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"raw\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#;
    let err = parse_servers(unsupported_only).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let mixed = r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"softsub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#;
    let servers = parse_servers(mixed).expect("the usable row");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].mode, "sub");
    assert_eq!(servers[0].name, "HD-2");
}

#[test]
fn a_nonempty_episode_list_with_no_recognizable_rows_is_a_parse_failure() {
    let json = r#"{"status":true,"html":"<div class=\"ss-list\"><a class=\"item\" data-num=\"1\" data-ref=\"9\"></a></div>"}"#;
    let err = parse_episode_list(json).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    assert_eq!(
        parse_episode_list(r#"{"status":true,"html":""}"#).expect("answered"),
        Vec::<EpisodeRef>::new(),
        "an empty listing is the provider answering no episodes"
    );
}

/// The listing as the site renders it for an entry it has announced
/// but not started serving, captured on 2026-09-09 for
/// `demon-slayer-kimetsu-no-yaiba-infinity-castle-part-2-7215`: the
/// listing's chrome and its container, with nothing inside.
const BLANK_EPISODE_LIST: &str = r#"{"status":true,"totalItems":0,"html":"<div class=\"seasons-block \">\n    <div id=\"detail-ss-list\" class=\"detail-seasons\">\n        <div class=\"detail-infor-content\">\n            <div class=\"ss-choice\">\n                <div class=\"ssc-list\">\n                    <div id=\"ssc-list\" class=\"ssc-button\">\n                        <div class=\"ssc-label\">List of episodes:</div>\n                    </div>\n                </div>\n                <div class=\"ssc-quick\">\n                    <input id=\"search-ep\" class=\"form-control\" type=\"text\" placeholder=\"Number of Ep\" autocomplete=\"off\">\n                </div>\n            </div>\n            <div class=\"ss-list\">\n            </div>\n        </div>\n    </div>\n</div>"}"#;

#[test]
fn a_listing_whose_container_holds_nothing_is_the_provider_answering_no_episodes() {
    // Refusing this shape made the probe weather, and weather made
    // the whole hianime attempt fail over — so the play surfaced the
    // other provider's outage instead of this show's own verdict.
    assert_eq!(
        parse_episode_list(BLANK_EPISODE_LIST).expect("answered"),
        Vec::<EpisodeRef>::new()
    );
}

/// The site's server list as captured on 2026-09-08, two days after
/// the first capture: `HD-1` and `HD-2` are megaplay.buzz now, and
/// the zokoanime server — the one whose page carries the payload —
/// is listed under its own name. The names rotate; the page shape
/// is what the client can read.
const SERVERS_RENAMED: &str = r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9zdWI/cz10Y2Ru\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9zdWI/cz1iY2Ru\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xNzM1LzM5MS9zdWI=\"></div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9kdWI/cz10Y2Ru\"></div>"}"#;

/// megaplay's embed page as captured on 2026-09-12: no payload in the
/// markup, the player element naming the media.
const MEGAPLAY_PAGE: &str = r#"<!DOCTYPE html><html><head><title>File 179411 - MegaPlay</title></head><body><div class="mg3-player"><div class="fix-area" id="megaplay-player" data-id="179411" data-realid="734292" data-mediaid="8879" data-fileversion="0"><div class="content-center"></div></div></div></body></html>"#;

/// megaplay's sources response as captured on 2026-09-12, the master
/// and the tracks pointed at stub hosts.
const MEGAPLAY_SOURCES: &str = r#"{"sources":{"file":"https://mp.example/v/master.m3u8"},"tracks":[{"file":"https://mp.example/v/subs/track_0_eng.vtt","label":"English","kind":"captions","default":true},{"file":"https://mp.example/v/subs/track_2_Latin_American_spa.vtt","label":"Spanish (Latin American)","kind":"captions"}],"t":1,"intro":{"start":0,"end":0},"outro":{"start":0,"end":0},"server":4}"#;

/// The servers to try for a mode, in order: the ones on a host whose
/// embed page the client can read first, then the site's own order.
/// A name is not a shape — `HD-1` moved hosts between two captures.
#[test]
fn the_servers_the_client_can_read_come_first_then_the_sites_order() {
    let servers = parse_servers(SERVERS).expect("parsed");
    assert_eq!(
        servers_for(&servers, "sub")
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["HD-1", "HD-2"],
        "zokoanime first, as the site lists it"
    );
    let renamed = parse_servers(SERVERS_RENAMED).expect("parsed");
    assert_eq!(
        renamed.len(),
        4,
        "the captured list parses whole: {renamed:?}"
    );
    assert_eq!(
        servers_for(&renamed, "sub")
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["ZokoAnime", "HD-1", "HD-2"],
        "zokoanime first though the site lists it last"
    );
    assert_eq!(
        servers_for(&renamed, "dub")
            .iter()
            .map(|s| s.embed_url.as_str())
            .collect::<Vec<_>>(),
        vec!["https://megaplay.buzz/stream/s-2/8272/dub?s=tcdn"],
        "a mode with no readable host still lists what the site has"
    );
    assert!(servers_for(&renamed, "raw").is_empty());
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
            url: "https://hls.example/v/subs/en.vtt".into(),
        }]
    );
}

#[test]
fn an_embed_page_without_the_blob_carries_no_playlist() {
    let err = decode_embed("<html><body>Player</body></html>").expect_err("no marker");
    assert!(matches!(err, AniError::NoResults), "{err:?}");
}

#[test]
fn a_payload_assignment_in_another_form_is_a_parse_failure() {
    // The marker is there; the assignment is not the one the parser
    // extracts. That is the site having reformatted its page, and it
    // must not read as an episode without a playlist.
    for page in [
        r#"<script>window.__P = "AAAA"</script>"#,
        r#"<script>window.__P='AAAA'</script>"#,
        r#"<script>window.__P=BLOB</script>"#,
    ] {
        let err = decode_embed(page).expect_err("refused");
        assert!(
            matches!(err, AniError::ParseFailed { .. }),
            "{page}: {err:?}"
        );
    }
}

#[test]
fn a_blob_that_no_longer_decodes_is_a_parse_failure() {
    // The marker is there and the bytes are not what the key opens:
    // the versioned key rotated, or the payload changed shape. That
    // is the site having changed, never "this episode has no stream".
    for page in [
        r#"<script>window.__P="not base64 at all!"</script>"#,
        r#"<script>window.__P="AAAA"</script>"#,
    ] {
        let err = decode_embed(page).expect_err("refused");
        assert!(
            matches!(err, AniError::ParseFailed { .. }),
            "{page}: {err:?}"
        );
    }
}

#[test]
fn the_embed_origin_is_the_referer_the_cdn_wants() {
    assert_eq!(
        embed_origin("https://zokoanime.video/stream/mal/1/1/sub").as_deref(),
        Some("https://zokoanime.video/")
    );
    assert_eq!(embed_origin("not a url"), None);
}

// ── the client over the seam ────────────────────────────────────────

use crate::scraper::fetch::{Fetch, FetchRequest, FetchResponse};
use crate::scraper::provider::{Provider, ProviderId, StreamSource};
use std::sync::Mutex;

const BASE: &str = "http://stub";

/// The site as the client sees it — every endpoint of the 2026-09-06
/// capture, refusing what the real site refuses: the AJAX listings
/// without `X-Requested-With`, the embed without the site's origin as
/// `Referer`, the playlists without the embed host's.
struct Site {
    log: Mutex<Vec<FetchRequest>>,
}

impl Site {
    fn new() -> Self {
        Self {
            log: Mutex::new(Vec::new()),
        }
    }
    fn requests(&self) -> Vec<FetchRequest> {
        self.log.lock().expect("log").clone()
    }
}

fn header<'a>(req: &'a FetchRequest, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn ok(body: impl Into<String>) -> crate::error::Result<FetchResponse> {
    Ok(FetchResponse {
        status: 200,
        body: body.into(),
    })
}

fn refused(status: u16) -> crate::error::Result<FetchResponse> {
    Ok(FetchResponse {
        status,
        body: "<html>Forbidden</html>".into(),
    })
}

#[async_trait::async_trait]
impl Fetch for Site {
    async fn fetch(&self, req: &FetchRequest) -> crate::error::Result<FetchResponse> {
        self.log.lock().expect("log").push(req.clone());
        let url = req.url.as_str();
        let ajax = header(req, "X-Requested-With") == Some("XMLHttpRequest");
        match url {
            u if u == format!("{BASE}/search?keyword=cowboy+bebop") => ok(SEARCH_PAGE),
            u if u == format!("{BASE}/search?keyword=zqxjvwkpltmb") => ok(
                r#"<html><body><div class="film_list film_list-grid"><p>No animes found.</p></div><div id="main-sidebar"></div></body></html>"#,
            ),
            u if u == format!("{BASE}/api/theme/episode/list/1281") => {
                if ajax {
                    ok(EPISODE_LIST)
                } else {
                    refused(403)
                }
            }
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21418") => {
                if ajax {
                    ok(SERVERS)
                } else {
                    refused(403)
                }
            }
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21419") => {
                if ajax {
                    ok(r#"{"status":true,"html":""}"#)
                } else {
                    refused(403)
                }
            }
            // A dub server the client reads beside a sub row whose hash
            // no longer decodes.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21429") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"!!!\"></div><div class=\"server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21420") => {
                if ajax {
                    ok(SERVERS_RENAMED)
                } else {
                    refused(403)
                }
            }
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21421") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9zdWI/cz10Y2Ru\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A readable host whose payload the key does not open,
            // then a host without a payload at all.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21422") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85Lzkvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9zdWI/cz1iY2Ru\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // The same undecodable page first, then a host whose page
            // decodes.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21423") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85Lzkvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A host that answers not-found for its page, then one
            // that refuses outright.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21424") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85LzQwNC9zdWI=\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85LzQwMy9zdWI=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A megaplay server alone: its page carries no payload and
            // names its media; the sources come from the site.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21430") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0MjkyL3N1Yj9zPWJjZG4=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A megaplay server whose sources come back encrypted,
            // null in the clear.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21431") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0MjkzL3N1Yg==\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // megaplay's pages as captured: no payload, the player
            // element naming the media. The sources endpoint wants the
            // page's own origin as the referer, like its player sends.
            "https://megaplay.buzz/stream/s-2/734292/sub?s=bcdn" => {
                if header(req, "Referer") == Some(&format!("{BASE}/")) {
                    ok(MEGAPLAY_PAGE)
                } else {
                    refused(403)
                }
            }
            "https://megaplay.buzz/stream/s-2/734293/sub" => {
                ok(MEGAPLAY_PAGE.replace("179411", "5"))
            }
            "https://megaplay.buzz/stream/getSourcesNew?id=179411" => {
                if header(req, "Referer") == Some("https://megaplay.buzz/")
                    && header(req, "X-Requested-With") == Some("XMLHttpRequest")
                {
                    ok(MEGAPLAY_SOURCES)
                } else {
                    refused(403)
                }
            }
            "https://megaplay.buzz/stream/getSourcesNew?id=5" => ok(
                r#"{"tracks":[],"t":1,"intro":{"start":0,"end":0},"outro":{"start":0,"end":0},"server":4,"enc":"wdeBruh3qqn"}"#,
            ),
            "https://mp.example/v/master.m3u8" => {
                if header(req, "Referer") == Some("https://megaplay.buzz/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1920x1080,NAME=\"1080p\"\nindex-f1.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://zokoanime.video/stream/mal/9/403/sub" => refused(403),
            // A host that refuses outright, then one that rate-limits.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21425") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85LzQwMy9zdWI=\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85LzQyOS9zdWI=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            "https://zokoanime.video/stream/mal/9/429/sub" => refused(429),
            // A host whose payload decodes to a source the client
            // cannot fetch — a relative path — then one whose page
            // decodes to a stream.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21426") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3JlbGF0aXZlL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A host whose payload the key does not open, then one
            // that rate-limits.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21428") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85Lzkvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85LzQyOS9zdWI=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A lone host whose payload decodes to a blank source.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21427") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L2JsYW5rL3N1Yg==\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // The payload decodes; its source is a relative path, or
            // nothing at all.
            "https://zokoanime.video/stream/mal/9/relative/sub" => ok(
                r#"<html><body><script>window.__P="FFYSGRYPX09NFwFBF0UGAgREGEwWGQcXSkBFRFdWTUkGWAcZCxEISAUTVS88Fg=="</script></body></html>"#,
            ),
            "https://zokoanime.video/stream/mal/9/blank/sub" => ok(
                r#"<html><body><script>window.__P="FFYSGRYPX09ASUZeA1MbHRUHEF5HVzk4GQ=="</script></body></html>"#,
            ),
            // megaplay's player page: no payload in the markup, the
            // sources come from a call its script makes.
            "https://megaplay.buzz/stream/s-2/8272/sub?s=tcdn"
            | "https://megaplay.buzz/stream/s-2/8272/sub?s=bcdn" => ok(
                r#"<html><head><title>File 143764 - MegaPlay</title></head><body><div id="player"></div><script src="/assets/player.js"></script></body></html>"#,
            ),
            // The payload marker is there; the blob is not one the
            // key opens.
            "https://zokoanime.video/stream/mal/9/9/sub" => {
                if header(req, "Referer") == Some(&format!("{BASE}/")) {
                    ok(r#"<html><body><script>window.__P="AAAA"</script></body></html>"#)
                } else {
                    refused(403)
                }
            }
            "https://zokoanime.video/stream/mal/1735/391/sub" => {
                if header(req, "Referer") == Some(&format!("{BASE}/")) {
                    ok(format!(
                        r#"<html><body><script>window.__P="{EMBED_BLOB}"</script></body></html>"#
                    ))
                } else {
                    refused(403)
                }
            }
            "https://zokoanime.video/stream/mal/1/1/sub" => {
                if header(req, "Referer") == Some(&format!("{BASE}/")) {
                    ok(format!(
                        r#"<html><body><script>window.__P="{EMBED_BLOB}"</script></body></html>"#
                    ))
                } else {
                    refused(403)
                }
            }
            "https://hls.example/v/master.m3u8" => {
                if header(req, "Referer") == Some("https://zokoanime.video/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720\n720/index.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://hls.example/v/720/index.m3u8" => {
                if header(req, "Referer") == Some("https://zokoanime.video/") {
                    ok("#EXTM3U\n")
                } else {
                    refused(403)
                }
            }
            u if u == format!("{BASE}/cowboy-bebop-1281") => ok(
                r#"<div class="item item-title"><span class="item-head">Aired:</span><span class="name">Apr 3, 1998 to Apr 24, 1999</span></div>"#,
            ),
            u if u == format!("{BASE}/search?keyword=blocked") => Ok(FetchResponse {
                status: 403,
                body: "<title>Just a moment...</title>".into(),
            }),
            _ => refused(404),
        }
    }
}

fn client() -> HianimeClient<Site> {
    HianimeClient::with_base(Site::new(), BASE)
}

#[test]
fn the_client_names_itself() {
    let c = client();
    assert_eq!(c.id(), ProviderId::Hianime);
    assert_eq!(c.label(), "hianime");
    assert_eq!(HIANIME_BASE, "https://hianime.at");
}

#[tokio::test]
async fn search_asks_the_search_page_with_the_encoded_query() {
    let c = client();
    let hits = c.search("cowboy bebop").await.expect("hits");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].slug, "cowboy-bebop-1281");
    assert_eq!(
        c.transport().requests()[0].url,
        format!("{BASE}/search?keyword=cowboy+bebop")
    );
    assert_eq!(
        c.search("zqxjvwkpltmb").await.expect("answered"),
        Vec::new(),
        "the no-results page is the provider answering absence"
    );
}

#[tokio::test]
async fn an_interstitial_is_an_upstream_refusal() {
    let err = client().search("blocked").await.expect_err("refused");
    assert!(matches!(err, AniError::Upstream { status: 403 }), "{err:?}");
}

#[tokio::test]
async fn episodes_are_keyed_on_the_slugs_id_and_asked_as_ajax() {
    let c = client();
    let eps = c.episodes("cowboy-bebop-1281").await.expect("listing");
    assert_eq!(eps.iter().map(|e| e.id).collect::<Vec<_>>(), [21418, 21419]);
    let err = c.episodes("cowboy-bebop").await.expect_err("no id");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

/// The client answers a mode from its listing: a readable server is
/// the mode present, no row of it is the mode absent, and rows the
/// client could not read are a parse failure — never an absence the
/// mode probe would persist.
#[tokio::test]
async fn a_mode_with_only_unreadable_rows_is_a_parse_failure_to_the_client() {
    let c = client();
    let err = c.has_mode(21429, "sub").await.expect_err("uncertain");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    assert!(c.has_mode(21429, "dub").await.expect("read"));
    let err = c
        .master_playlist_url(21429, "sub")
        .await
        .expect_err("no stream can be read");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[tokio::test]
async fn has_mode_reads_the_episodes_server_list() {
    let c = client();
    assert!(c.has_mode(21418, "sub").await.expect("answered"));
    assert!(c.has_mode(21418, "dub").await.expect("answered"));
    assert!(
        !c.has_mode(21419, "dub").await.expect("answered"),
        "an episode with no servers has no mode"
    );
}

#[tokio::test]
async fn the_master_is_the_decoded_embed_src_with_the_embed_origin_as_referer() {
    let c = client();
    let source = c.master_playlist_url(21418, "sub").await.expect("resolved");
    assert_eq!(
        source,
        StreamSource {
            master_url: "https://hls.example/v/master.m3u8".into(),
            referer: Some("https://zokoanime.video/".into()),
            subtitles: vec![SubtitleTrack {
                lang: "en".into(),
                label: "English".into(),
                default: true,
                url: "https://hls.example/v/subs/en.vtt".into(),
            }],
        },
        "the embed's sidecar tracks ride out with the master — a sub stream is raw video without them"
    );
    let err = c
        .master_playlist_url(21419, "sub")
        .await
        .expect_err("no servers");
    assert!(matches!(err, AniError::NoResults), "{err:?}");
}

#[tokio::test]
async fn quality_selection_fetches_playlists_with_the_embed_referer() {
    let c = client();
    let source = c.master_playlist_url(21418, "sub").await.expect("resolved");
    let chosen = c
        .quality_stream_url(&source, "720")
        .await
        .expect("selected");
    assert_eq!(chosen, "https://hls.example/v/720/index.m3u8");
}

#[tokio::test]
async fn detail_year_reads_the_entry_page_and_soft_misses() {
    let c = client();
    assert_eq!(
        c.detail_year("cowboy-bebop-1281").await.expect("year"),
        Some(1998)
    );
    assert_eq!(
        c.detail_year("no-such-entry-9").await.expect("soft"),
        None,
        "a missing page is no hint, not a failure"
    );
}

/// The site lists several servers per episode and names them by
/// slot, and the slots move between hosts; only one host's page
/// carries the payload the client reads. The master comes from the
/// first server whose page it can read, whatever the site calls it.
#[tokio::test]
async fn the_master_comes_from_the_first_server_whose_page_the_client_reads() {
    let c = client();
    let source = c.master_playlist_url(21420, "sub").await.expect("resolved");
    assert_eq!(source.master_url, "https://hls.example/v/master.m3u8");
    assert_eq!(
        source.referer.as_deref(),
        Some("https://zokoanime.video/"),
        "the referer is the host that served the payload"
    );
    assert!(c.has_mode(21420, "sub").await.expect("asked"));
}

#[tokio::test]
async fn an_episode_whose_servers_all_serve_unreadable_pages_has_no_stream() {
    let c = client();
    let err = c
        .master_playlist_url(21421, "sub")
        .await
        .expect_err("nothing readable");
    assert!(matches!(err, AniError::NoResults), "{err:?}");
}

#[tokio::test]
async fn a_payload_the_client_cannot_decode_is_a_parse_failure_once_every_server_is_tried() {
    // The marker is on the page and the blob does not open: the key
    // rotated or the shape moved. Read as "no stream" that is an
    // answered verdict — health to the breaker, an absence the mode
    // probe may persist — for what is the client no longer reading
    // the site.
    let c = client();
    let err = c
        .master_playlist_url(21422, "sub")
        .await
        .expect_err("nothing decoded");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[tokio::test]
async fn the_weather_kept_across_servers_is_the_block_not_the_first_miss() {
    // The first host answers not-found for its page and the next
    // refuses. Surfacing the first — an answered status — reads as a
    // dead end, and the breaker records health on a provider that
    // just blocked the client; the refusal is what the walk needs.
    let c = client();
    let err = c
        .master_playlist_url(21424, "sub")
        .await
        .expect_err("no server served a stream");
    assert!(matches!(err, AniError::Upstream { status: 403 }), "{err:?}");
    assert!(err.is_provider_block());
}

#[tokio::test]
async fn the_weather_kept_across_servers_is_the_rate_limit_over_a_generic_block() {
    // The first host refuses and the next rate-limits. Both are
    // provider blocks, but only the rate limit opens the breaker's
    // advertised pause at once; keeping the refusal lets background
    // traffic go on hitting a provider that is rate-limiting.
    let c = client();
    let err = c
        .master_playlist_url(21425, "sub")
        .await
        .expect_err("no server served a stream");
    assert!(matches!(err, AniError::Upstream { status: 429 }), "{err:?}");
}

#[tokio::test]
async fn a_payload_whose_source_the_client_cannot_fetch_is_stepped_over_for_a_later_server() {
    // The payload decoded, but its source is a relative path: nothing
    // the transport can fetch. Taking it ends the walk on an operand
    // that fails, while the next server had the stream.
    let c = client();
    let source = c.master_playlist_url(21426, "sub").await.expect("resolved");
    assert_eq!(source.master_url, "https://hls.example/v/master.m3u8");
}

#[tokio::test]
async fn a_payload_whose_source_is_blank_is_a_parse_failure_when_no_server_serves_a_stream() {
    // Decoded, and not the shape the client can use: the site changed
    // what it puts in the payload, which is a parse failure and never
    // an episode without a stream.
    let c = client();
    let err = c
        .master_playlist_url(21427, "sub")
        .await
        .expect_err("no usable source");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

#[tokio::test]
async fn a_rate_limit_from_a_later_server_outranks_an_earlier_parse_failure() {
    // One host's payload the key does not open; the next host
    // rate-limits. The parse failure says the client no longer reads
    // the site, but the rate limit is the one verdict the breaker
    // acts on at once — the advertised pause — and surfacing the
    // parse failure instead lets background traffic go on hitting a
    // provider that is throttling.
    let c = client();
    let err = c
        .master_playlist_url(21428, "sub")
        .await
        .expect_err("no server served a stream");
    assert!(matches!(err, AniError::Upstream { status: 429 }), "{err:?}");
}

#[tokio::test]
async fn a_payload_the_client_cannot_decode_is_stepped_over_when_a_later_server_decodes() {
    let c = client();
    let source = c.master_playlist_url(21423, "sub").await.expect("resolved");
    assert_eq!(source.master_url, "https://hls.example/v/master.m3u8");
}

/// The site lists megaplay servers beside zokoanime's, and their
/// pages carry no payload: the player names its media, and asks the
/// site for the sources. The client reads that shape too, so an
/// episode plays from whichever server the site lists.
#[tokio::test]
async fn a_megaplay_server_is_read_through_the_sites_sources_endpoint() {
    let c = client();
    let source = c.master_playlist_url(21430, "sub").await.expect("resolved");
    assert_eq!(
        source,
        StreamSource {
            master_url: "https://mp.example/v/master.m3u8".into(),
            referer: Some("https://megaplay.buzz/".into()),
            subtitles: vec![
                SubtitleTrack {
                    lang: "eng".into(),
                    label: "English".into(),
                    default: true,
                    url: "https://mp.example/v/subs/track_0_eng.vtt".into(),
                },
                SubtitleTrack {
                    lang: "spa".into(),
                    label: "Spanish (Latin American)".into(),
                    default: false,
                    url: "https://mp.example/v/subs/track_2_Latin_American_spa.vtt".into(),
                },
            ],
        },
        "the referer is the embed host's origin, which the CDN and the sources endpoint both check"
    );
}

#[tokio::test]
async fn a_megaplay_server_whose_sources_are_encrypted_is_a_parse_failure() {
    // The older endpoint's shape: the sources encrypted, null in the
    // clear. The site changed what it hands the client; that is
    // never an episode without a stream.
    let c = client();
    let err = c
        .master_playlist_url(21431, "sub")
        .await
        .expect_err("no usable source");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}
