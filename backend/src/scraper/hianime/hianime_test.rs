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
/// A card's identity is one anchor's href and title together. A
/// detail block whose first anchor carries only a title, followed by
/// a link that carries only an href, is not a card the parser can
/// read: read as one, the title would be paired with a slug that is
/// not its own, and an apparent exact-title match would send the
/// picker to the wrong entry. The same the other way round.
#[test]
fn a_card_whose_title_and_slug_sit_on_different_anchors_is_skipped() {
    let page = |detail: &str| {
        format!(
            r#"<html><body><div class="film_list-wrap"><div class="flw-item"><div class="film-detail">{detail}</div></div><div class="flw-item"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/naruto-5" title="Naruto" class="dynamic-name">Naruto</a></h3><div class="fd-infor"><span class="fdi-item">TV</span></div></div></div></div></body></html>"#
        )
    };
    let title_then_href = page(
        r#"<h3 class="film-name"><a title="Cowboy Bebop" class="dynamic-name">Cowboy Bebop</a></h3><div class="fd-infor"><a href="https://hianime.at/wrong-123">more</a><span class="fdi-item">TV</span></div>"#,
    );
    let hits = parse_search(&title_then_href).expect("the readable card");
    assert_eq!(
        hits.iter().map(|h| h.slug.as_str()).collect::<Vec<_>>(),
        vec!["naruto-5"],
        "{hits:?}"
    );
    let href_then_title = page(
        r#"<h3 class="film-name"><a href="https://hianime.at/wrong-123" class="dynamic-name">x</a></h3><div class="fd-infor"><a title="Cowboy Bebop">more</a><span class="fdi-item">TV</span></div>"#,
    );
    let hits = parse_search(&href_then_title).expect("the readable card");
    assert_eq!(
        hits.iter().map(|h| h.slug.as_str()).collect::<Vec<_>>(),
        vec!["naruto-5"],
        "{hits:?}"
    );
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
    let err = parse_search(no_cards).expect_err("refused");
    assert!(
        matches!(err, AniError::ParseFailed { .. }),
        "a result region with no card boundary and no notice is not an answer: {err:?}"
    );
}

/// The site's no-results page carries the notice and no list, so a
/// list region with no card boundary inside is not a shape the site
/// renders for an empty search — it is what the list looks like once
/// the boundary is renamed, whether the cards are there or not. Read
/// as "no results", every title searched would be persisted as
/// absent for as long as the rename lasts.
#[test]
fn a_result_list_without_card_boundaries_is_a_parse_failure_unless_the_notice_says_none() {
    let renamed = r#"<html><body><div class="film_list-wrap"><div class="film-item"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/cowboy-bebop-1281" title="Cowboy Bebop">Cowboy Bebop</a></h3></div></div></div><div id="main-sidebar"></div></body></html>"#;
    let err = parse_search(renamed).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let with_notice = r#"<html><body><div class="film_list-wrap"><p>No animes found.</p></div><div id="main-sidebar"></div></body></html>"#;
    assert_eq!(
        parse_search(with_notice).expect("parsed"),
        Vec::<BrowseHit>::new(),
        "the notice beside an empty region is still the provider answering none"
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

/// The value is read inside the Aired row alone: a row whose value
/// span lost its class names no year, rather than the first `name`
/// element further down the page — a studio or a genre carrying a
/// four-digit number would otherwise become the premiere year and
/// filter the right entry out.
#[test]
fn a_year_is_never_read_from_outside_the_aired_row() {
    let malformed_row_then_decoy = r#"<div class="item item-title">
        <span class="item-head">Aired:</span>
        <span>Apr 3, 1998 to Apr 24, 1999</span>
    </div>
    <div class="item item-list"><span class="item-head">Studios:</span><a class="name">Studio 2019</a></div>"#;
    assert_eq!(parse_detail_year(malformed_row_then_decoy), None);
    let bare_row_then_decoy = r#"<span class="item-head">Aired:</span><span>?</span><span class="item-head">Genres:</span><a class="name">2019</a>"#;
    assert_eq!(parse_detail_year(bare_row_then_decoy), None);
    let intact_row_then_decoy = r#"<div class="item item-title"><span class="item-head">Aired:</span><span class="name">Apr 3, 1998</span></div>
    <div class="item item-list"><span class="item-head">Studios:</span><a class="name">Studio 2019</a></div>"#;
    assert_eq!(parse_detail_year(intact_row_then_decoy), Some(1998));
}

// ── episode listing ─────────────────────────────────────────────────

const EPISODE_LIST: &str = r#"{"status":true,"totalItems":1,"html":"<div class=\"ss-list\">\n<a title=\"Episode 1\"\n   class=\"ssl-item ep-item\"\n   data-number=\"1\"\n   data-id=\"21418\"\n   href=\"https://hianime.at/watch/cowboy-bebop-1281?ep=21418\"><div class=\"ssli-order\">1</div></a>\n<a title=\"Episode 2\" class=\"ssl-item ep-item\" data-number=\"2\" data-id=\"21419\" href=\"https://hianime.at/watch/cowboy-bebop-1281?ep=21419\"></a>\n</div>"}"#;

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

/// The site's row marker is a class on the row's tag, and the tag's
/// other attributes may come before it as well as after: the same
/// row written `<a data-number="1" data-id="…" class="ep-item">` is
/// the same episode. A reader that took the text after the marker as
/// the row would find that row's attributes in the chunk before it —
/// and, with rows written both ways, read each row's attributes as
/// the next row's, dropping the first episode of the listing.
#[test]
fn an_episode_row_reads_the_same_whatever_order_its_attributes_come_in() {
    let reversed = r#"{"status":true,"html":"<div class=\"ss-list\"><a data-number=\"1\" data-id=\"21418\" class=\"ssl-item ep-item\" href=\"/watch/x?ep=21418\"><div class=\"ssli-order\">1</div></a><a data-number=\"2\" data-id=\"21419\" class=\"ssl-item ep-item\" href=\"/watch/x?ep=21419\"></a></div>"}"#;
    let expected = vec![
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
    ];
    assert_eq!(parse_episode_list(reversed).expect("listing"), expected);
    let mixed = r#"{"status":true,"html":"<div class=\"ss-list\"><a data-number=\"1\" data-id=\"21418\" class=\"ssl-item ep-item\"></a><a class=\"ssl-item ep-item\" data-number=\"2\" data-id=\"21419\"></a><a href=\"/watch/x?ep=21420\" data-id=\"21420\" class=\"ssl-item ep-item\" data-number=\"3\"></a></div>"}"#;
    let mut expected = expected;
    expected.push(EpisodeRef {
        id: 21420,
        number: 3,
        number2: None,
    });
    assert_eq!(parse_episode_list(mixed).expect("listing"), expected);
}

/// A listing of episodes 1 to 7, a recap the site numbers `7.5`,
/// then episode 8 — the recap at the eighth position, episode 8 at
/// the ninth — with ids the stub site serves for 7, 7.5 and 8.
fn listing_with_a_recap() -> String {
    let rows: Vec<String> = (1..=6)
        .map(|n| {
            format!(
                r#"<a class=\"ssl-item ep-item\" data-number=\"{n}\" data-id=\"2143{n}\"></a>"#
            )
        })
        .chain([
            r#"<a class=\"ssl-item ep-item\" data-number=\"7\" data-id=\"21419\"></a>"#.to_string(),
            r#"<a class=\"ssl-item ep-item\" data-number=\"7.5\" data-id=\"21418\"><div class=\"ssli-order\">8</div></a>"#.to_string(),
            r#"<a class=\"ssl-item ep-item\" data-number=\"8\" data-id=\"21420\"></a>"#.to_string(),
        ])
        .collect();
    format!(
        r#"{{"status":true,"html":"<div class=\"ss-list\">{}</div>"}}"#,
        rows.join("")
    )
}

/// A row the site numbers `7.5` is a recap or a special: it keeps
/// its position in the listing as its slot and carries the site's
/// number as its display tag, exactly as anidb.app's rows do, so the
/// tag can be advertised as an extra and a click on it resolves. The
/// rows after it keep their own numbers as tags too, since their
/// position no longer says their number. A row whose number is not
/// a number at all is kept the same way — the tag is the site's,
/// and only a row without a number or an id is unreadable, which
/// refuses the listing ([`a_listing_with_an_unreadable_row_is_refused_not_shortened`]).
#[test]
fn a_fractional_row_keeps_its_position_as_slot_and_its_number_as_tag() {
    use crate::commands::play_native_numbering::{
        extra_episode_tags, kitsu_episode_cap, numbering_offset,
    };
    let eps = parse_episode_list(&listing_with_a_recap()).expect("listing");
    assert_eq!(eps.len(), 9, "no row dropped: {eps:?}");
    assert_eq!(
        eps[7],
        EpisodeRef {
            id: 21418,
            number: 8,
            number2: Some("7.5".into()),
        }
    );
    assert_eq!(
        eps[8],
        EpisodeRef {
            id: 21420,
            number: 9,
            number2: Some("8".into()),
        }
    );
    assert_eq!(
        eps[6],
        EpisodeRef {
            id: 21419,
            number: 7,
            number2: None
        }
    );
    assert_eq!(numbering_offset(&eps), 0);
    assert_eq!(kitsu_episode_cap(&eps), Some(8));
    assert_eq!(extra_episode_tags(&eps), vec!["7.5".to_string()]);

    let odd = r#"{"status":true,"html":"<div class=\"ss-list\"><a class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\"></a><a class=\"ssl-item ep-item\" data-number=\"OVA\" data-id=\"21419\"></a></div>"}"#;
    assert_eq!(
        parse_episode_list(odd).expect("listing"),
        vec![
            EpisodeRef {
                id: 21418,
                number: 1,
                number2: None
            },
            EpisodeRef {
                id: 21419,
                number: 2,
                number2: Some("OVA".into()),
            },
        ],
        "a nonnumeric number is a tag"
    );
}

/// The listing is the show's count — what picks a candidate by its
/// episodes and what a click is looked up in — so a row the reader
/// cannot read is not a row to leave out: a listing shortened by one
/// undercounts the show, and the dropped episode answers "no such
/// episode" while its link may play. A listing with an unreadable
/// marked row is refused as the site having changed shape, whether
/// the row's id is missing, not a number, or its number is blank.
#[test]
fn a_listing_with_an_unreadable_row_is_refused_not_shortened() {
    let readable = r#"<a class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\"></a>"#;
    let no_id = format!(
        r#"{{"status":true,"html":"<div class=\"ss-list\">{readable}<a class=\"ssl-item ep-item\" data-number=\"2\"></a></div>"}}"#
    );
    let err = parse_episode_list(&no_id).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let bad_id = format!(
        r#"{{"status":true,"html":"<div class=\"ss-list\">{readable}<a class=\"ssl-item ep-item\" data-number=\"2\" data-id=\"x21419\"></a></div>"}}"#
    );
    let err = parse_episode_list(&bad_id).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let blank_number = format!(
        r#"{{"status":true,"html":"<div class=\"ss-list\">{readable}<a class=\"ssl-item ep-item\" data-number=\"\" data-id=\"21420\"></a></div>"}}"#
    );
    let err = parse_episode_list(&blank_number).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    assert_eq!(
        parse_episode_list(&format!(
            r#"{{"status":true,"html":"<div class=\"ss-list\">{readable}</div>"}}"#
        ))
        .expect("listing")
        .len(),
        1,
        "the readable row alone is a listing"
    );
}

/// The whole chain for the recap: the client lists the show, the
/// resolver finds the `7.5` row by its tag, and the stream comes
/// from that row's id — the recap's, not episode 7's or 8's.
#[tokio::test]
async fn a_fractional_episode_resolves_through_the_client_by_its_tag() {
    let c = client();
    let episodes = c.episodes("show-1282").await.expect("listing");
    let picked = crate::commands::play_native::PickedShow {
        hit: BrowseHit {
            slug: "show-1282".into(),
            title: "Show".into(),
            kind: None,
        },
        episodes,
    };
    let resolved =
        crate::commands::play_native_episode::resolve_episode(&c, &picked, "7.5", "sub", "720")
            .await
            .expect("the recap resolves");
    assert_eq!(resolved.master_url, "https://hls.example/v/720/index.m3u8");
    assert_eq!(resolved.slot, 8);
    assert_eq!(resolved.tag.as_deref(), Some("7.5"));
    let asked: Vec<String> = c
        .transport()
        .requests()
        .iter()
        .map(|r| r.url.to_string())
        .filter(|u| u.contains("servers?episodeId="))
        .collect();
    assert_eq!(
        asked,
        vec![format!("{BASE}/api/theme/episode/servers?episodeId=21418")],
        "the recap's own id, and only it"
    );
}

/// The server row has the same shape — its marker is a class on the
/// row's tag — and reads the same way whatever order the attributes
/// come in, so a listing written marker-last still names every
/// server of every mode.
#[test]
fn a_server_row_reads_the_same_whatever_order_its_attributes_come_in() {
    let rows = r#"{"status":true,"html":"<div data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\" class=\"item server-item\"><a class=\"btn\">HD-1</a></div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(rows).expect("listing");
    assert_eq!(
        listing.servers,
        vec![
            ServerEmbed {
                mode: "sub".into(),
                name: "HD-1".into(),
                embed_url: "https://zokoanime.video/stream/mal/1/1/sub".into(),
            },
            ServerEmbed {
                mode: "dub".into(),
                name: "HD-2".into(),
                embed_url: "https://zokoanime.video/stream/mal/1/1/dub".into(),
            },
        ]
    );
    assert!(listing.unreadable_modes.is_empty(), "{listing:?}");
    assert!(!listing.unknown_modes, "{listing:?}");
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

/// A row typed with a mode the client does not know beside a readable
/// row of a known one: the site may have renamed `sub` to `softsub`
/// while `dub` stayed. Read as "no sub", the mode probe would persist
/// an absence over the sub playback the site still lists, so the
/// listing keeps that an unknown row was seen, and a known mode with
/// no row of its own answers a parse failure while one was.
#[test]
fn a_known_mode_with_no_row_is_uncertain_when_a_row_of_an_unknown_mode_was_seen() {
    let renamed = r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"softsub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(renamed).expect("the dub row is readable");
    assert_eq!(listing.servers.len(), 1);
    assert_eq!(listing.servers[0].mode, "dub");
    assert!(listing.mode_readable("dub").expect("read"));
    assert!(
        matches!(
            listing.mode_readable("sub"),
            Err(AniError::ParseFailed { .. })
        ),
        "a mode with no row of its own is not absent while a row the client could not type was seen"
    );
    let dub_only = r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(dub_only).expect("parsed");
    assert!(
        !listing.mode_readable("sub").expect("read"),
        "a mode the listing carries no row of, with every row typed, is absent"
    );
}

/// A row that carries the site's row marker but no mode attribute at
/// all is a row whose mode the client cannot tell — the attribute
/// renamed under a row that may be the sub server — and beside a
/// readable dub row it leaves sub in doubt, not absent: read as "no
/// sub", the mode probe would persist an absence over a playback the
/// site still lists. A listing of nothing but such rows is refused.
#[test]
fn a_server_row_without_a_mode_attribute_leaves_a_known_mode_with_no_row_uncertain() {
    let untyped_beside_dub = r#"{"status":true,"html":"<div class=\"item server-item\" data-kind=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(untyped_beside_dub).expect("the dub row is readable");
    assert_eq!(listing.servers.len(), 1);
    assert_eq!(listing.servers[0].mode, "dub");
    assert!(listing.mode_readable("dub").expect("read"));
    assert!(
        matches!(
            listing.mode_readable("sub"),
            Err(AniError::ParseFailed { .. })
        ),
        "a mode with no row of its own is not absent while a row without a mode was seen"
    );
    let untyped_only = r#"{"status":true,"html":"<div class=\"item server-item\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#;
    let err = parse_server_listing(untyped_only).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
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

/// A listing of `rows` episode rows under an envelope declaring
/// `pages` page tabs, each row written the way the site writes one.
/// Generated rather than captured: the counts that matter here run
/// into the hundreds, and a capture of them would be megabytes of
/// fixture for one number.
fn listing_of(rows: usize, pages: u64) -> String {
    let html: String = (1..=rows)
        .map(|n| {
            let id = 21_000 + n;
            format!(
                r#"<a class="ssl-item ep-item" data-number="{n}" data-id="{id}" href="/watch/x?ep={id}"></a>"#
            )
        })
        .collect();
    serde_json::json!({ "status": true, "totalItems": pages, "html": html }).to_string()
}

/// The envelope's count is the number of page tabs the site draws
/// over the listing, one per hundred rows — the live listing of 366
/// episodes declares four, one of 23 declares one — so rows that
/// fill the last page the count names are the listing whole.
#[test]
fn rows_filling_the_last_page_the_envelope_declares_are_read_as_the_listing() {
    for (rows, pages) in [(23_usize, 1_u64), (100, 1), (101, 2), (366, 4)] {
        match parse_episode_list(&listing_of(rows, pages)) {
            Ok(listing) => assert_eq!(listing.len(), rows, "{rows} rows, {pages} declared"),
            Err(e) => panic!("{rows} rows, {pages} declared: {e:?}"),
        }
    }
}

/// Rows that stop short of the page declared, or spill past it, are
/// not the show's listing: the HTML cut off a page or more early, or
/// a row changed past everything the reader recognises as a row.
/// Read as complete it would undercount the show for the picker and
/// lose the missing rows' links. What the count cannot catch is a
/// listing cut short inside its own last page — the site declares
/// the same four tabs for 301 rows as for 400.
#[test]
fn rows_that_do_not_fill_the_page_the_envelope_declares_are_refused() {
    for (rows, pages) in [(101_usize, 1_u64), (99, 2), (0, 1)] {
        assert!(
            matches!(
                parse_episode_list(&listing_of(rows, pages)),
                Err(AniError::ParseFailed { .. })
            ),
            "{rows} rows, {pages} declared"
        );
    }
}

/// The refusal names what each number is: the rows the reader read,
/// and the pages the envelope declared. The captured envelope's two
/// rows are one page; three declared is two pages of rows gone.
#[test]
fn a_listing_declaring_pages_its_rows_do_not_reach_is_refused_naming_both() {
    let two_rows_declared_three = EPISODE_LIST.replacen("\"totalItems\":1", "\"totalItems\":3", 1);
    assert!(
        two_rows_declared_three.contains("\"totalItems\":3"),
        "fixture rewritten"
    );
    let err = parse_episode_list(&two_rows_declared_three).expect_err("refused");
    match err {
        AniError::ParseFailed { detail } => {
            assert!(
                detail.contains('3') && detail.contains('2') && detail.contains("pages"),
                "names the rows read and the pages declared: {detail}"
            );
        }
        other => panic!("{other:?}"),
    }
}

/// A count the rows fill, or no count at all, changes nothing about
/// a readable listing. The blank listing declares no page and reads
/// no row; the same blank container under a declared page is refused,
/// since a page of rows is then missing rather than absent.
#[test]
fn a_declared_count_the_rows_fill_or_that_is_absent_leaves_the_listing_as_read() {
    let agreeing = parse_episode_list(EPISODE_LIST).expect("listing");
    assert_eq!(
        agreeing.len(),
        2,
        "the captured envelope's rows are its one declared page"
    );
    let without_count = EPISODE_LIST.replacen("\"totalItems\":1,", "", 1);
    assert!(!without_count.contains("totalItems"), "fixture rewritten");
    assert_eq!(
        parse_episode_list(&without_count).expect("listing"),
        agreeing,
        "no count declared: the rows as read"
    );
    assert!(
        BLANK_EPISODE_LIST.contains("\"totalItems\":0"),
        "the blank capture declares none"
    );
    assert_eq!(
        parse_episode_list(BLANK_EPISODE_LIST).expect("answered"),
        Vec::<EpisodeRef>::new()
    );
    let blank_declaring_a_page =
        BLANK_EPISODE_LIST.replacen("\"totalItems\":0", "\"totalItems\":1", 1);
    assert!(
        matches!(
            parse_episode_list(&blank_declaring_a_page),
            Err(AniError::ParseFailed { .. })
        ),
        "an empty container under a page of rows declared"
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
        vec!["HD-1", "HD-2", "ZokoAnime"],
        "megaplay's pages are read too, so the site's order stands"
    );
    assert_eq!(
        servers_for(&renamed, "dub")
            .iter()
            .map(|s| s.embed_url.as_str())
            .collect::<Vec<_>>(),
        vec!["https://megaplay.buzz/stream/s-2/8272/dub?s=tcdn"],
        "a mode's only server is listed whatever its host"
    );
    let unread_first = parse_servers(
        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-3\" data-hash=\"aHR0cHM6Ly92aWR0dWJlLnNpdGUvc3RyZWFtL2FiYy9zdWI=\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9zdWI/cz1iY2Ru\"></div>"}"#,
    )
    .expect("parsed");
    assert_eq!(
        servers_for(&unread_first, "sub")
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["HD-2", "HD-3"],
        "a host the client reads comes before one it does not, though the site lists it after"
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

/// The page's payload as the site ships it: the player JSON XOR'd
/// under the versioned key, then base64'd — for payloads a table
/// writes in the clear.
fn embed_page(json: &str) -> String {
    use base64::Engine as _;
    const KEY: &[u8] = b"otaku-embed-v1";
    let xored: Vec<u8> = json
        .bytes()
        .zip(KEY.iter().cycle())
        .map(|(b, k)| b ^ k)
        .collect();
    let blob = base64::engine::general_purpose::STANDARD.encode(xored);
    format!(r#"<html><body><script>window.__P="{blob}"</script></body></html>"#)
}

/// The tracks are optional, and the stream is not: a payload whose
/// source is usable decodes whatever its subtitle list looks like —
/// `null`, absent, not a list, or holding rows the client cannot
/// read — because skipping a host over its subtitle list would fail
/// an episode whose stream is right there. The rows the client can
/// read are kept, in order; the rest are dropped.
#[test]
fn a_payload_decodes_to_its_stream_whatever_its_subtitle_list_looks_like() {
    let src = "https://hls.example/v/master.m3u8";
    let good = |lang: &str| serde_json::json!({"lang": lang, "label": lang.to_uppercase(), "default": false, "src": format!("https://hls.example/v/subs/{lang}.vtt")});
    let cases: Vec<(&str, serde_json::Value, Vec<&str>)> = vec![
        (
            "null tracks",
            serde_json::json!({"src": src, "subtitles": null}),
            vec![],
        ),
        ("no tracks field", serde_json::json!({"src": src}), vec![]),
        (
            "tracks not a list",
            serde_json::json!({"src": src, "subtitles": "en"}),
            vec![],
        ),
        (
            "a row without its source",
            serde_json::json!({"src": src, "subtitles": [good("en"), {"lang": "de", "label": "German"}]}),
            vec!["en"],
        ),
        (
            "a row without its language",
            serde_json::json!({"src": src, "subtitles": [{"label": "English", "src": "https://hls.example/v/subs/x.vtt"}, good("fr")]}),
            vec!["fr"],
        ),
        (
            "a row that is not an object",
            serde_json::json!({"src": src, "subtitles": ["en", good("es"), 7]}),
            vec!["es"],
        ),
        // A row whose source the transport cannot fetch — relative
        // to a page the client does not carry, or under another
        // scheme — would ride into the proxy, the handoffs and the
        // cache row as a track that never loads, and a cached row
        // with one is refused whole on every replay.
        (
            "a row whose source is relative",
            serde_json::json!({"src": src, "subtitles": [good("en"), {"lang": "de", "label": "German", "src": "/subs/de.vtt"}, good("fr")]}),
            vec!["en", "fr"],
        ),
        (
            "a row whose source is under another scheme",
            serde_json::json!({"src": src, "subtitles": [{"lang": "it", "label": "Italian", "src": "ftp://hls.example/v/subs/it.vtt"}, good("pt")]}),
            vec!["pt"],
        ),
        // A row whose source is absolute and fetchable but far longer
        // than any track URL a CDN signs: the hand-offs put every
        // track URL on the player's command line, whose budget the
        // packaged platforms set differently and Windows sets
        // smallest, so one such row would fail Open External and
        // Watch Together for the whole episode.
        (
            "a row whose source is overlong",
            serde_json::json!({"src": src, "subtitles": [good("en"), {"lang": "de", "label": "German", "src": format!("https://hls.example/v/subs/{}.vtt", "a".repeat(4096))}, good("fr")]}),
            vec!["en", "fr"],
        ),
    ];
    for (what, json, expected) in cases {
        let payload = decode_embed(&embed_page(&json.to_string()))
            .unwrap_or_else(|e| panic!("{what}: refused a usable stream: {e:?}"));
        assert_eq!(payload.src, src, "{what}");
        let langs: Vec<&str> = payload.subtitles.iter().map(|t| t.lang.as_str()).collect();
        assert_eq!(langs, expected, "{what}");
    }
}

/// The stream stays required: a payload with tracks and no source
/// is still the site having changed.
#[test]
fn a_payload_without_its_source_is_a_parse_failure_whatever_its_tracks() {
    let json = serde_json::json!({"subtitles": [{"lang": "en", "label": "English", "src": "https://hls.example/v/subs/en.vtt"}]});
    let err = decode_embed(&embed_page(&json.to_string())).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
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

/// How long the stub's slow hosts hold a master before answering —
/// past the budget the stalled-host tests give a server, well under
/// the ceiling they allow a whole walk.
const SLOW_HOST: std::time::Duration = std::time::Duration::from_millis(250);
/// A wait a bounded server clears inside the test's per-server budget.
const BRIEF_HOST: std::time::Duration = std::time::Duration::from_millis(40);
/// What each request of the stub's paced chain waits before it
/// answers. A chain of four outlasts the per-server budget the
/// stalled-host tests give a server, while no single request comes
/// near it — a healthy host under load, not one that has stopped
/// answering.
const CHAIN_STEP: std::time::Duration = std::time::Duration::from_millis(30);

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
            u if u == format!("{BASE}/api/theme/episode/list/1282") => {
                if ajax {
                    ok(listing_with_a_recap())
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
            // A sub server whose page carries no payload beside a sub
            // row whose hash no longer decodes: the mode is uncertain
            // and no server serves a stream.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21434") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9zdWI/cz10Y2Ru\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"!!!\"></div>"}"#,
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
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly92aWR0dWJlLnNpdGUvZW1iZWQvODI3Mi9zdWI=\"></div>"}"#,
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
            // Two readable hosts whose pages carry no payload marker
            // at all: the host the client reads has changed shape.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21435") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L25vcGF5bG9hZC9zdWI=\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L25vcGF5bG9hZDIvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A readable host without the marker, then a readable host
            // whose page decodes.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21436") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L25vcGF5bG9hZC9zdWI=\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A readable host without the marker beside a host the
            // client never read.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21437") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L25vcGF5bG9hZC9zdWI=\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9zdWI/cz10Y2Ru\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A host that answers not-found for its page, then one
            // whose connection times out.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21438") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85LzQwNC9zdWI=\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3RpbWVvdXQvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A host that refuses, slowly, then a host whose
            // connection drops at once: the block is the louder
            // failure, and it was the earlier attempt.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21441") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3Nsb3ctNDAzL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3RpbWVvdXQvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A host whose connection drops, slowly, then a host that
            // refuses at once: the block is the louder failure, and
            // it was the later attempt.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21442") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3Nsb3ctdGltZW91dC9zdWI=\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85LzQwMy9zdWI=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            "https://zokoanime.video/stream/mal/9/slow-403/sub" => {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                refused(403)
            }
            "https://zokoanime.video/stream/mal/9/slow-timeout/sub" => {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                Err(AniError::Timeout)
            }
            // A listing that answers slowly, with a sub row the client
            // reads beside one whose hash no longer decodes; the
            // readable host then answers not-found. The doubt is the
            // listing's, and the listing's attempt came first.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21460") => {
                if ajax {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L2xhdGUtNDA0L3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"!!!\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // The same slow listing with every row read: the
            // not-found is the verdict, and it is the host's attempt.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21461") => {
                if ajax {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L2xhdGUtNDA0L3N1Yg==\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            "https://zokoanime.video/stream/mal/9/late-404/sub" => refused(404),
            "https://zokoanime.video/stream/mal/9/timeout/sub" => Err(AniError::Timeout),
            // A host that answers not-found, then a fetch the gate
            // refuses, then a host whose page decodes.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21439") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85LzQwNC9zdWI=\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L2dhdGUvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-3\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            "https://zokoanime.video/stream/mal/9/gate/sub" => Err(AniError::GateRefused),
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
            // A zokoanime server whose page decodes to a master on a
            // dead host, then a megaplay server that serves.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21432") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L2RlYWQvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0MjkyL3N1Yj9zPWJjZG4=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // Every server's stream on a dead host: zokoanime's, then
            // megaplay's.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21433") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L2RlYWQvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0Mjk0L3N1Yg==\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A zokoanime server whose master answers but whose 720
            // rendition refuses, then a megaplay server whose whole
            // chain answers.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21444") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsZWQvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0MjkyL3N1Yj9zPWJjZG4=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // Both servers' masters answer; both 720 renditions refuse.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21445") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsZWQvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0Mjk1L3N1Yg==\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A zokoanime server whose master never answers, then a
            // megaplay server whose chain answers.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21446") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsaW5nL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0MjkyL3N1Yj9zPWJjZG4=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A lone zokoanime server whose master answers, slowly.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21448") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3Nsb3cvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A zokoanime server whose master never answers, then a
            // megaplay server whose master answers, slowly.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21449") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsaW5nL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0Mjk3L3N1Yg==\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A zokoanime server whose master answers, slowly, then a
            // host the client does not read: the readable server is
            // the last the walk can use, whatever trails it.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21450") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3Nsb3cvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"VidTube\" data-hash=\"aHR0cHM6Ly92aWR0dWJlLnNpdGUvZW1iZWQvODI3Mi9zdWI=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A zokoanime server whose master never answers, a
            // megaplay server whose master answers slowly, then a
            // host the client does not read.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21451") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsaW5nL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"MegaPlay\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0Mjk3L3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"VidTube\" data-hash=\"aHR0cHM6Ly92aWR0dWJlLnNpdGUvZW1iZWQvODI3Mi9zdWI=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A lone megaplay mirror server whose chain answers, slowly.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21452") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"MegaPlay\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS0xLmJ1enovc3RyZWFtL3MtMi83MzQyOTkvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A lone server on a host the client never named, whose
            // page carries the payload shape and whose master answers,
            // slowly.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21453") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"Moved\" data-hash=\"aHR0cHM6Ly9uZXdlbWJlZC5leGFtcGxlL2UvbW92ZWQvc3Vi\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // The same unnamed host, then a host the client never read:
            // no host the client names is listed.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21454") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"Moved\" data-hash=\"aHR0cHM6Ly9uZXdlbWJlZC5leGFtcGxlL2UvbW92ZWQvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"VidTube\" data-hash=\"aHR0cHM6Ly92aWR0dWJlLnNpdGUvZW1iZWQvODI3Mi9zdWI=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // Two zokoanime servers whose masters hold the connection,
            // then a megaplay server whose whole chain answers at once:
            // the shape in which stalled servers spend an attempt's
            // remainder before a healthy last server is reached.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21467") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsaW5nL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsaW5nL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0MjkyL3N1Yj9zPWJjZG4=\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // A zokoanime server whose master answers inside the bound,
            // then one whose master never answers — the remainder's
            // server. A stale deadline caps the first at nothing and the
            // walk ends on the second's silence.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21468") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L2JyaWVmL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsaW5nL3N1Yg==\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // Two zokoanime servers whose masters hold the connection,
            // then a megaplay server whose whole chain answers, every
            // request of it taking a slice: the healthy-but-loaded
            // shape, whose four sequential fetches together outlast a
            // per-server bound while each sits far inside the
            // transport's own wait.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21469") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsaW5nL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsaW5nL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0MzAxL3N1Yg==\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // Both servers' masters hold the connection: the first
            // for as long as it is waited for, the second until the
            // transport's own deadline reports it.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21447") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85L3N0YWxsaW5nL3N1Yg==\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvNzM0Mjk2L3N1Yg==\"></div>"}"#,
                    )
                } else {
                    refused(403)
                }
            }
            // The payload decodes to a master whose host holds the
            // connection open and never answers — the outage as the
            // transport sees it before its own deadline.
            "https://zokoanime.video/stream/mal/9/stalling/sub" => ok(
                r#"<html><body><script>window.__P="FFYSGRYPX08KERBdBQtAWwkHBgMAFQMIFEETHhlbEh8UQQkEDAJLQBdCGxETRRgeEFVASUZeA1MbHRUHEF5HVzk4GQ=="</script></body></html>"#,
            ),
            "https://hls.example/v/stalling/master.m3u8" => std::future::pending().await,
            "https://megaplay.buzz/stream/s-2/734296/sub" => {
                ok(MEGAPLAY_PAGE.replace("179411", "8"))
            }
            "https://megaplay.buzz/stream/getSourcesNew?id=8" => ok(
                r#"{"sources":{"file":"https://mp.example/v/stalling/master.m3u8"},"tracks":[]}"#,
            ),
            "https://mp.example/v/stalling/master.m3u8" => Err(AniError::Network),
            // The payload decodes to a master that answers but whose
            // 720 rendition the host refuses.
            "https://zokoanime.video/stream/mal/9/stalled/sub" => ok(
                r#"<html><body><script>window.__P="FFYSGRYPX08KERBdBQtAWwkHBgMAFQMIFEETHhlbEh8UQQkIBkoJTAVFCgZPBkZYXU9ORxdYFEUGAA0OBg9fNj8Y"</script></body></html>"#,
            ),
            "https://hls.example/v/stalled/master.m3u8" => {
                if header(req, "Referer") == Some("https://zokoanime.video/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720\n720/index.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://hls.example/v/stalled/720/index.m3u8" => refused(503),
            // The payload decodes to a master that answers after a
            // wait longer than the test's per-server budget.
            "https://zokoanime.video/stream/mal/9/slow/sub" => ok(
                r#"<html><body><script>window.__P="FFYSGRYPX08KERBdBQtAWwkHBgMAFQMIFEETHhlbEgcaWkoAAxYQSAQfAkcUU1cBRx4XBxBEAl0KB0NRLnAY"</script></body></html>"#,
            ),
            "https://hls.example/v/slow/master.m3u8" => {
                tokio::time::sleep(SLOW_HOST).await;
                if header(req, "Referer") == Some("https://zokoanime.video/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720\n720/index.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://hls.example/v/slow/720/index.m3u8" => ok("#EXTM3U\n"),
            // The payload decodes to a master that answers after a
            // wait inside the test's per-server budget.
            "https://zokoanime.video/stream/mal/9/brief/sub" => ok(
                r#"<html><body><script>window.__P="FFYSGRYPX08KERBdBQtAWwkHBgMAFQMIFEETHhlbAxkcSANCDwQXWRNDQRlSHk0PGA=="</script></body></html>"#,
            ),
            "https://hls.example/v/brief/master.m3u8" => {
                tokio::time::sleep(BRIEF_HOST).await;
                if header(req, "Referer") == Some("https://zokoanime.video/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720\n720/index.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://hls.example/v/brief/720/index.m3u8" => ok("#EXTM3U\n"),
            // The payload shape on a host the client never named: the
            // page reads, and its master answers after the same wait.
            "https://newembed.example/e/moved/sub" => ok(
                r#"<html><body><script>window.__P="FFYSGRYPX08KERBdBQtAWwkHBgMAFQMIFEETHhlbDAQDSAFCDwQXWRNDQRlSHk0PSU8REAZZH0UDERJJT3Y4EA=="</script></body></html>"#,
            ),
            "https://hls.example/v/moved/master.m3u8" => {
                tokio::time::sleep(SLOW_HOST).await;
                if header(req, "Referer") == Some("https://newembed.example/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720\n720/index.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://hls.example/v/moved/720/index.m3u8" => ok("#EXTM3U\n"),
            "https://megaplay.buzz/stream/s-2/734297/sub" => {
                ok(MEGAPLAY_PAGE.replace("179411", "10"))
            }
            "https://megaplay.buzz/stream/getSourcesNew?id=10" => {
                ok(r#"{"sources":{"file":"https://mp.example/v/slow/master.m3u8"},"tracks":[]}"#)
            }
            "https://mp.example/v/slow/master.m3u8" => {
                tokio::time::sleep(SLOW_HOST).await;
                if header(req, "Referer") == Some("https://megaplay.buzz/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720,NAME=\"720p\"\nindex-f2.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://mp.example/v/slow/index-f2.m3u8" => ok("#EXTM3U\n"),
            // A megaplay chain whose every request answers after a
            // wait: the embed page, the sources answer, the master and
            // the chosen rendition, four of them in sequence. Nothing
            // here has stopped answering — the host is loaded, and the
            // chain's total is what a server given only one request's
            // worth of time never reaches.
            "https://megaplay.buzz/stream/s-2/734301/sub" => {
                tokio::time::sleep(CHAIN_STEP).await;
                ok(MEGAPLAY_PAGE.replace("179411", "14"))
            }
            "https://megaplay.buzz/stream/getSourcesNew?id=14" => {
                tokio::time::sleep(CHAIN_STEP).await;
                ok(r#"{"sources":{"file":"https://mp.example/v/paced/master.m3u8"},"tracks":[]}"#)
            }
            "https://mp.example/v/paced/master.m3u8" => {
                tokio::time::sleep(CHAIN_STEP).await;
                if header(req, "Referer") == Some("https://megaplay.buzz/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720,NAME=\"720p\"\nindex-f2.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://mp.example/v/paced/index-f2.m3u8" => {
                tokio::time::sleep(CHAIN_STEP).await;
                ok("#EXTM3U\n")
            }
            "https://megaplay-1.buzz/stream/s-2/734299/sub" => {
                ok(MEGAPLAY_PAGE.replace("179411", "12"))
            }
            "https://megaplay-1.buzz/stream/getSourcesNew?id=12" => {
                ok(r#"{"sources":{"file":"https://mp.example/v/mirror/master.m3u8"},"tracks":[]}"#)
            }
            "https://mp.example/v/mirror/master.m3u8" => {
                tokio::time::sleep(SLOW_HOST).await;
                if header(req, "Referer") == Some("https://megaplay-1.buzz/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720,NAME=\"720p\"\nindex-f2.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://mp.example/v/mirror/index-f2.m3u8" => ok("#EXTM3U\n"),
            "https://megaplay.buzz/stream/s-2/734295/sub" => {
                ok(MEGAPLAY_PAGE.replace("179411", "7"))
            }
            "https://megaplay.buzz/stream/getSourcesNew?id=7" => {
                ok(r#"{"sources":{"file":"https://mp.example/v/stalled/master.m3u8"},"tracks":[]}"#)
            }
            "https://mp.example/v/stalled/master.m3u8" => {
                if header(req, "Referer") == Some("https://megaplay.buzz/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720\n720/index.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://mp.example/v/stalled/720/index.m3u8" => refused(503),
            // The payload decodes to a master on a host that is down.
            "https://zokoanime.video/stream/mal/9/dead/sub" => ok(
                r#"<html><body><script>window.__P="FFYSGRYPX08KERBdBQtAWwUOFElLCBoECV0aVEACTgYUXhEIEEsJHgMJTVhDGABPEQQWCQFeVAs0KRw="</script></body></html>"#,
            ),
            "https://dead.example/v/master.m3u8" => refused(503),
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
            "https://megaplay.buzz/stream/s-2/734294/sub" => {
                ok(MEGAPLAY_PAGE.replace("179411", "6"))
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
            "https://megaplay.buzz/stream/getSourcesNew?id=6" => {
                ok(r#"{"sources":{"file":"https://dead.example/v/master.m3u8"},"tracks":[]}"#)
            }
            "https://mp.example/v/master.m3u8" => {
                if header(req, "Referer") == Some("https://megaplay.buzz/") {
                    ok("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1920x1080,NAME=\"1080p\"\nindex-f1.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720,NAME=\"720p\"\nindex-f2.m3u8\n")
                } else {
                    refused(403)
                }
            }
            "https://mp.example/v/index-f2.m3u8" => {
                if header(req, "Referer") == Some("https://megaplay.buzz/") {
                    ok("#EXTM3U\n")
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
            // A host whose payload the key does not open, then one
            // that refuses outright.
            u if u == format!("{BASE}/api/theme/episode/servers?episodeId=21440") => {
                if ajax {
                    ok(
                        r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85Lzkvc3Vi\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC85LzQwMy9zdWI=\"></div>"}"#,
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
            // A readable host's page shipped without the payload
            // marker: the player is there, the blob is not.
            "https://zokoanime.video/stream/mal/9/nopayload/sub"
            | "https://zokoanime.video/stream/mal/9/nopayload2/sub" => {
                if header(req, "Referer") == Some(&format!("{BASE}/")) {
                    ok(
                        r#"<html><body><div id="player"></div><script type="module" src="/player.js?v=14"></script></body></html>"#,
                    )
                } else {
                    refused(403)
                }
            }
            // megaplay's player page: no payload in the markup, the
            // sources come from a call its script makes.
            "https://vidtube.site/embed/8272/sub" => ok(
                r#"<html><head><title>VidTube</title></head><body><div id="player"></div><script src="/assets/vt.js"></script></body></html>"#,
            ),
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

/// A mode the listing left uncertain — a row the client could not
/// read beside one it could — stays uncertain when the readable
/// server's page carries no payload: the unreadable row may have
/// been the playable server, so the walk ends in a parse failure,
/// never in the answered absence the resolver would treat as an
/// episode's dead end.
#[tokio::test]
async fn an_uncertain_mode_whose_readable_pages_carry_no_payload_is_a_parse_failure() {
    let c = client();
    assert!(c.has_mode(21434, "sub").await.expect("a readable server"));
    let err = c
        .master_playlist_url(21434, "sub")
        .await
        .expect_err("no stream can be read");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

/// A lone host the client never read, serving a page without the
/// payload, says nothing about the client: it is stepped over, and
/// an episode with only such hosts has no stream.
#[tokio::test]
async fn an_episode_whose_servers_all_serve_unreadable_pages_has_no_stream() {
    let c = client();
    let err = c
        .master_playlist_url(21421, "sub")
        .await
        .expect_err("nothing readable");
    assert!(matches!(err, AniError::NoResults), "{err:?}");
}

/// A host the client reads, serving its page without the payload
/// marker, is the site having changed shape under the client — the
/// same event as a blob the key no longer opens — and when every
/// readable server's page is like that, the walk's end is a parse
/// failure, never the answered absence the resolver would persist
/// as the episode's dead end and the breaker as health.
#[tokio::test]
async fn a_readable_host_whose_page_lost_the_payload_is_a_parse_failure_once_every_server_is_tried()
{
    let c = client();
    let err = c
        .master_playlist_url(21435, "sub")
        .await
        .expect_err("no page decoded");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
}

/// The same page beside a readable host whose page decodes: the
/// stream, as for any stepped-over server.
#[tokio::test]
async fn a_readable_host_whose_page_lost_the_payload_is_stepped_over_when_a_later_server_decodes() {
    let c = client();
    let source = c
        .master_playlist_url(21436, "sub")
        .await
        .expect("the second server decodes");
    assert_eq!(source.master_url, "https://hls.example/v/master.m3u8");
}

/// The readable host's missing marker outranks the silence of a host
/// the client never read: the mixed listing ends in a parse failure.
#[tokio::test]
async fn a_readable_host_without_the_payload_beside_an_unreadable_host_is_a_parse_failure() {
    let c = client();
    let err = c
        .master_playlist_url(21437, "sub")
        .await
        .expect_err("no page decoded");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
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

/// The first host answers not-found for its page and the next one's
/// connection times out. Both are quiet next to a block, but they
/// are not of a kind: the answered status is that host's own dead
/// end, while the timeout says a server that may carry the stream
/// was never heard from. Surfacing the status makes the episode an
/// answered dead end — recorded as the provider's health, never
/// failed over — when a possible server was unreachable; the
/// transport failure is what the walk needs, since it alone moves
/// the walk on.
#[tokio::test]
async fn the_weather_kept_across_servers_is_the_transport_failure_over_an_answered_status() {
    let c = client();
    let err = c
        .master_playlist_url(21438, "sub")
        .await
        .expect_err("no server served a stream");
    assert!(matches!(err, AniError::Timeout), "{err:?}");
}

/// A fetch the gate refuses is the gate speaking, not the host: the
/// breaker opened, or a pause began, between two embed fetches of a
/// background walk. Ranked with the hosts' own failures it lost to
/// an earlier answered status, the episode became an answered dead
/// end, and the walk went on asking hosts the gate would refuse
/// too. The refusal ends the walk at once, as it is, and no later
/// server is asked.
#[tokio::test]
async fn a_gate_refusal_ends_the_server_walk_at_once() {
    let c = client();
    let err = c
        .master_playlist_url(21439, "sub")
        .await
        .expect_err("the gate refused");
    assert!(matches!(err, AniError::GateRefused), "{err:?}");
    let asked: Vec<String> = c
        .transport()
        .requests()
        .iter()
        .map(|r| r.url.to_string())
        .collect();
    assert!(
        asked.iter().any(|u| u.ends_with("/mal/9/gate/sub")),
        "the refused fetch was made: {asked:?}"
    );
    assert!(
        !asked.iter().any(|u| u.ends_with("/mal/1/1/sub")),
        "no server is asked after the gate refused: {asked:?}"
    );
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
async fn a_block_from_a_later_server_outranks_an_earlier_parse_failure() {
    // One host's payload the key does not open; the next host
    // refuses outright. The parse failure says the client no longer
    // reads the site, but it is transient to the shared walk, which
    // would go on through the show's other candidates against a
    // provider that has just blocked it; the block is the verdict
    // that stops the walk, so it is the one kept.
    let c = client();
    let err = c
        .master_playlist_url(21440, "sub")
        .await
        .expect_err("no server served a stream");
    assert!(matches!(err, AniError::Upstream { status: 403 }), "{err:?}");
    assert!(
        err.is_provider_block(),
        "the verdict is the block the shared walk stops on"
    );
}

#[tokio::test]
async fn a_payload_the_client_cannot_decode_is_stepped_over_when_a_later_server_decodes() {
    let c = client();
    let source = c.master_playlist_url(21423, "sub").await.expect("resolved");
    assert_eq!(source.master_url, "https://hls.example/v/master.m3u8");
}

// ── rows that lost the marker ───────────────────────────────────────

/// A row is a row by its own attributes; the site's row class only
/// names it. A row that lost the class while its neighbours kept it
/// would otherwise be passed over as chrome, and the listing read as
/// complete one row short — renumbered, since the position is the
/// slot, with the dropped episode's link lost. Such a listing is
/// refused, naming the row's position among the rows.
#[test]
fn an_episode_listing_with_a_row_that_lost_its_marker_is_refused_not_shortened() {
    let lost = r#"{"status":true,"html":"<div class=\"ss-list\"><a class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\"></a><a class=\"ssl-item\" data-number=\"2\" data-id=\"21419\"></a><a class=\"ssl-item ep-item\" data-number=\"3\" data-id=\"21420\"></a></div>"}"#;
    let err = parse_episode_list(lost).expect_err("refused");
    match err {
        AniError::ParseFailed { detail } => assert!(detail.contains("row 2"), "{detail}"),
        other => panic!("expected a parse failure, got {other:?}"),
    }
    let kept = r#"{"status":true,"html":"<div class=\"ss-list\"><a class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\"></a><a class=\"ssl-item ep-item\" data-number=\"2\" data-id=\"21419\"></a></div>"}"#;
    assert_eq!(
        parse_episode_list(kept).expect("listing").len(),
        2,
        "rows that keep their marker read as before"
    );
}

/// A server row that lost its marker beside one that kept it is a
/// row whose mode the client cannot tell — exactly as a marked row
/// without its mode attribute is — so a mode with no marked row is
/// uncertain, not absent: read as absent, the lost sub row would let
/// the mode probe persist a "no sub" over a playback the site lists.
#[test]
fn a_server_row_that_lost_its_marker_leaves_its_mode_uncertain() {
    let lost_beside_dub = r#"{"status":true,"html":"<div class=\"item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(lost_beside_dub).expect("the dub row is readable");
    assert_eq!(
        listing.servers.len(),
        1,
        "the row without the marker is no server"
    );
    assert_eq!(listing.servers[0].mode, "dub");
    assert!(
        listing.unknown_modes,
        "a row that lost its marker is a row of a mode the client cannot tell"
    );
    assert!(
        matches!(
            listing.mode_readable("sub"),
            Err(AniError::ParseFailed { .. })
        ),
        "sub is uncertain, not absent"
    );
    assert!(listing.mode_readable("dub").expect("read"));
    let lost_only = r#"{"status":true,"html":"<div class=\"item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#;
    let err = parse_server_listing(lost_only).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let all_marked = parse_server_listing(SERVERS).expect("parsed");
    assert!(
        !all_marked.unknown_modes,
        "rows that keep their marker leave nothing in doubt"
    );
}

/// A server row is known by its signature — the server's name and
/// the hash of its embed, which every captured row carries and no
/// other tag on the page does — not by any one data attribute. A
/// tab or control beside the rows with a generic `data-id` is
/// chrome: it leaves nothing in doubt, so a mode with no row is
/// absent, which the mode probe may persist. A tag carrying the
/// signature without the marker is still a row the class left.
#[test]
fn chrome_with_a_generic_data_attribute_is_not_a_server_row() {
    let chrome_beside_sub = r#"{"status":true,"html":"<div class=\"ps_-tabs\"><div class=\"ps_-tab\" data-id=\"tab-1\">Sub</div><div class=\"ps_-tab\" data-type=\"dub\">Dub</div></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div>"}"#;
    let listing = parse_server_listing(chrome_beside_sub).expect("the sub row is readable");
    assert_eq!(listing.servers.len(), 1);
    assert!(
        !listing.unknown_modes,
        "chrome with one generic attribute is not a row that lost its marker"
    );
    assert!(
        !listing.mode_readable("dub").expect("dub is decided"),
        "with no dub row and nothing in doubt, dub is absent"
    );
    let signed_without_marker = r#"{"status":true,"html":"<div class=\"item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(signed_without_marker).expect("the dub row is readable");
    assert!(
        listing.unknown_modes,
        "a tag carrying the row's signature without the marker is a row the class left"
    );
}

/// A row can lose its marker and one attribute of its signature in
/// the same change of shape — a sub row keeping its mode and its
/// server's name while the hash attribute is renamed — and it is
/// still a row, not chrome: chrome beside the rows carries at most
/// one of the row's attributes, a row two or more. Such a row leaves
/// its mode uncertain, as a row that lost the marker alone does;
/// read as chrome, a readable dub row beside it would let the mode
/// probe persist a "no sub" the site lists.
#[test]
fn a_server_row_that_lost_its_marker_and_one_attribute_is_still_a_row() {
    let hash_renamed = r#"{"status":true,"html":"<div class=\"item\" data-type=\"sub\" data-server-name=\"HD-1\" data-embed=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(hash_renamed).expect("the dub row is readable");
    assert_eq!(listing.servers.len(), 1);
    assert_eq!(listing.servers[0].mode, "dub");
    assert!(
        listing.unknown_modes,
        "a row keeping two of its three attributes is a row the class left"
    );
    assert!(
        matches!(
            listing.mode_readable("sub"),
            Err(AniError::ParseFailed { .. })
        ),
        "sub is uncertain, not absent"
    );
    assert!(listing.mode_readable("dub").expect("read"));
    let mode_renamed = r#"{"status":true,"html":"<div class=\"item\" data-kind=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvc3Vi\"></div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(mode_renamed).expect("the dub row is readable");
    assert!(
        listing.unknown_modes,
        "the name and the hash without the mode are still a row"
    );
    let one_attribute_only = r#"{"status":true,"html":"<div class=\"ps_-tab\" data-type=\"sub\">Sub</div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xLzEvZHVi\"></div>"}"#;
    let listing = parse_server_listing(one_attribute_only).expect("the dub row is readable");
    assert!(
        !listing.unknown_modes,
        "one attribute alone is chrome, as before"
    );
    assert!(
        !listing.mode_readable("sub").expect("sub is decided"),
        "with no sub row and nothing in doubt, sub is absent"
    );
}

/// An episode row is known by its number: every captured row is an
/// anchor carrying `data-number`, and no control anchor in the
/// listing's chrome does — a control carries a generic `data-id`
/// alone. So an anchor with the number and no marker is a row the
/// class left even when its id attribute was renamed in the same
/// change, and the listing is refused rather than read one row
/// short; an anchor with an id alone is chrome, as before.
#[test]
fn an_episode_row_that_lost_its_marker_and_its_id_attribute_is_still_a_row() {
    let id_renamed = r#"{"status":true,"html":"<div class=\"ss-list\"><a class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\"></a><a class=\"ssl-item\" data-number=\"2\" data-ep=\"21419\"></a><a class=\"ssl-item ep-item\" data-number=\"3\" data-id=\"21420\"></a></div>"}"#;
    let err = parse_episode_list(id_renamed).expect_err("refused");
    match err {
        AniError::ParseFailed { detail } => assert!(detail.contains("row 2"), "{detail}"),
        other => panic!("expected a parse failure, got {other:?}"),
    }
    let number_alone = r#"{"status":true,"html":"<div class=\"ss-list\"><a class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\"></a><a class=\"ssl-item\" data-number=\"2\"></a></div>"}"#;
    let err = parse_episode_list(number_alone).expect_err("refused");
    assert!(matches!(err, AniError::ParseFailed { .. }), "{err:?}");
    let control = r#"{"status":true,"html":"<div class=\"ss-list\"><a class=\"ssc-btn\" data-id=\"search\" href=\"/search\">Search</a><a class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\"></a></div>"}"#;
    assert_eq!(
        parse_episode_list(control)
            .expect("a control is chrome")
            .len(),
        1
    );
}

/// An episode row is an anchor carrying both its number and its id,
/// as every captured row does; an anchor beside the rows with only
/// a generic `data-id` — a control in the listing's chrome — is not
/// a row that lost its marker, and the listing reads as its marked
/// rows. An anchor with both and no marker is still refused.
#[test]
fn chrome_with_a_generic_data_attribute_is_not_an_episode_row() {
    let chrome_beside_rows = r#"{"status":true,"html":"<div class=\"ss-list\"><a class=\"ssc-btn\" data-id=\"search\" href=\"/search\">Search</a><a class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\"></a><a class=\"ssl-item ep-item\" data-number=\"2\" data-id=\"21419\"></a></div>"}"#;
    assert_eq!(
        parse_episode_list(chrome_beside_rows)
            .expect("chrome beside the rows is not a lost row")
            .len(),
        2
    );
    let signed_without_marker = r#"{"status":true,"html":"<div class=\"ss-list\"><a class=\"ssl-item ep-item\" data-number=\"1\" data-id=\"21418\"></a><a class=\"ssl-item\" data-number=\"2\" data-id=\"21419\"></a></div>"}"#;
    let err = parse_episode_list(signed_without_marker).expect_err("refused");
    match err {
        AniError::ParseFailed { detail } => assert!(detail.contains("row 2"), "{detail}"),
        other => panic!("expected a parse failure, got {other:?}"),
    }
}

// ── the kept failure carries the instant of its own attempt ────────

/// A client over the transport that stamps each attempt, as the
/// production one does.
fn stamping_client() -> HianimeClient<crate::scraper::gated::GatedFetch<'static, Site>> {
    HianimeClient::with_base(
        crate::scraper::gated::GatedFetch::new(
            Site::new(),
            None,
            crate::scraper::gate::ScrapePriority::Interactive,
        ),
        BASE,
    )
}

/// The walk keeps the louder of two hosts' failures, and the gate is
/// told when that failure was observed: an outcome stamped by the
/// LATER attempt would be read as evidence gathered after a recovery
/// a concurrent resolve recorded between the two, and reopen the
/// breaker on a failure that predates it. The kept failure carries
/// the instant of the attempt that produced it.
#[tokio::test(start_paused = true)]
async fn the_kept_failure_carries_the_instant_of_the_attempt_that_produced_it() {
    let c = stamping_client();
    let began = tokio::time::Instant::now();
    let err = c
        .master_playlist_url(21441, "sub")
        .await
        .expect_err("nobody served it");
    assert!(
        matches!(err, AniError::Upstream { status: 403 }),
        "the block is kept over the later dropped connection: {err:?}"
    );
    assert_eq!(
        c.last_attempt_at(),
        Some(began),
        "the block's own attempt, before the slow host answered, not the timeout's after it"
    );
}

/// The control: when the later attempt produced the louder failure,
/// the later instant is the right one.
#[tokio::test(start_paused = true)]
async fn a_louder_later_failure_carries_its_own_instant() {
    let c = stamping_client();
    let began = tokio::time::Instant::now();
    let err = c
        .master_playlist_url(21442, "sub")
        .await
        .expect_err("nobody served it");
    assert!(
        matches!(err, AniError::Upstream { status: 403 }),
        "the block is kept over the earlier dropped connection: {err:?}"
    );
    assert_eq!(
        c.last_attempt_at(),
        Some(began + std::time::Duration::from_millis(10)),
        "the block's own attempt, which began once the slow host had answered"
    );
}

/// The listing's doubt — a row of the mode the client could not read
/// — is the verdict when the readable hosts end in something quieter,
/// and it is the listing attempt's finding, not the last host's: the
/// gate is told when the doubt was observed, so a recovery recorded
/// between the listing and a later host's attempt does not read the
/// doubt as evidence gathered after it.
#[tokio::test(start_paused = true)]
async fn the_listings_doubt_carries_the_listing_attempts_instant() {
    let c = stamping_client();
    let began = tokio::time::Instant::now();
    let err = c
        .master_playlist_url(21460, "sub")
        .await
        .expect_err("nobody served it");
    assert!(
        matches!(err, AniError::ParseFailed { .. }),
        "the doubt outranks the host's not-found: {err:?}"
    );
    assert_eq!(
        c.last_attempt_at(),
        Some(began),
        "the listing's own attempt, before the slow listing answered and the host was asked"
    );
}

/// The control: with every row read, the host's not-found is the
/// verdict and carries the host's own attempt, after the listing.
#[tokio::test(start_paused = true)]
async fn a_hosts_failure_after_a_slow_listing_carries_the_hosts_instant() {
    let c = stamping_client();
    let began = tokio::time::Instant::now();
    let err = c
        .master_playlist_url(21461, "sub")
        .await
        .expect_err("nobody served it");
    assert!(
        matches!(err, AniError::Upstream { status: 404 }),
        "the host's own dead end: {err:?}"
    );
    assert_eq!(
        c.last_attempt_at(),
        Some(began + std::time::Duration::from_millis(10)),
        "the host's attempt, which began once the slow listing had answered"
    );
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

/// A server's page can decode to a stream on a host that is down.
/// Taking it ends the walk on a master that never answers — the
/// episode step's fetch times out and the resolver moves to the next
/// alias, never the next server — while the site listed another
/// server whose stream is up.
#[tokio::test]
async fn a_server_whose_stream_host_is_dead_is_stepped_over_for_the_next_server() {
    let c = client();
    let source = c.master_playlist_url(21432, "sub").await.expect("resolved");
    assert_eq!(source.master_url, "https://mp.example/v/master.m3u8");
    assert_eq!(source.referer.as_deref(), Some("https://megaplay.buzz/"));
    let urls: Vec<String> = c
        .transport()
        .requests()
        .iter()
        .map(|r| r.url.clone())
        .collect();
    assert!(
        urls.contains(&"https://dead.example/v/master.m3u8".to_string()),
        "the dead master was asked before the next server: {urls:?}"
    );
}

#[tokio::test]
async fn every_servers_stream_host_dead_surfaces_the_loudest_failure() {
    let c = client();
    let err = c
        .master_playlist_url(21433, "sub")
        .await
        .expect_err("no server served a stream");
    assert!(matches!(err, AniError::Upstream { status: 503 }), "{err:?}");
}

/// The episode step selects a quality and fetches the rendition;
/// that is the validation a play rides on. A server whose master
/// answers but whose rendition refuses is a server that does not
/// serve the play, and the walk steps to the next one — the
/// validation the caller uses happens inside the walk of the
/// servers, or a dead rendition ends the walk after it returned and
/// the resolver moves to the next alias, never the next server.
#[tokio::test]
async fn a_server_whose_rendition_refuses_is_stepped_over_for_one_whose_chain_answers() {
    let c = client();
    let stream = c.stream_for(21444, "sub", "720").await.expect("resolved");
    assert_eq!(stream.url, "https://mp.example/v/index-f2.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://megaplay.buzz/"));
    let urls: Vec<String> = c
        .transport()
        .requests()
        .iter()
        .map(|r| r.url.clone())
        .collect();
    assert!(
        urls.contains(&"https://hls.example/v/stalled/720/index.m3u8".to_string()),
        "the first server's rendition was asked before the next server: {urls:?}"
    );
}

#[tokio::test]
async fn every_servers_rendition_dead_surfaces_the_loudest_failure() {
    let c = client();
    let err = c
        .stream_for(21445, "sub", "720")
        .await
        .expect_err("no server served the rendition");
    assert!(matches!(err, AniError::Upstream { status: 503 }), "{err:?}");
}

/// A client whose per-server budget is short enough for a test to
/// wait out: the production bound is sized for real hosts.
fn client_with_server_budget(ms: u64) -> HianimeClient<Site> {
    client().with_server_budget(std::time::Duration::from_millis(ms))
}

/// The reserve is sized against the attempt it is carved out of. A
/// provider's attempt has twenty seconds for the search, the
/// candidate, the listings and every server together; the reserve
/// holds ten of them for the server that runs on the remainder,
/// which is a whole chain at a pace a loaded CDN keeps and still
/// leaves the attempt room for the work ahead of the walk and for a
/// bounded server besides.
#[test]
fn the_reserve_is_one_chain_of_the_attempts_budget() {
    let reserve = chain_reserve(SERVER_ATTEMPT_BUDGET);
    assert_eq!(reserve, std::time::Duration::from_secs(10), "{reserve:?}");
    assert!(
        reserve + SERVER_ATTEMPT_BUDGET < crate::commands::providers::PRIMARY_ATTEMPT_BUDGET,
        "the reserve left the attempt no room for a bounded server ahead of it: {reserve:?}"
    );
}

/// A bounded server's share is carved out of what the attempt has
/// left, and what it has left is not always more than the reserve:
/// the search, the candidate and the listings are spent before the
/// first server is asked, and a slow site can leave the walk the
/// reserve and nothing over. Held back whole there, the reserve
/// would cap every server ahead of the remainder's at nothing, and
/// each would be stepped over without a request window while the
/// attempt still ran — a stream the walk had and did not take,
/// should the remainder's server be the dead one. The reserve gives
/// way instead: what remains is split among the servers ahead and
/// the remainder's server alike, so each of them has a window, and
/// the remainder's server is left no less than any one of them.
#[test]
fn a_server_ahead_of_the_remainder_keeps_a_window_when_only_the_reserve_is_left() {
    let bound = SERVER_ATTEMPT_BUDGET;
    let reserve = chain_reserve(bound);
    for ahead in 1..4usize {
        for remaining in [std::time::Duration::from_millis(1), reserve / 2, reserve] {
            let cap = server_cap(bound, reserve, Some(remaining), ahead);
            assert!(
                !cap.is_zero(),
                "a healthy server was capped at nothing with {remaining:?} of the \
                 attempt left and {ahead} server(s) ahead of the remainder's"
            );
            assert!(cap <= bound, "{cap:?} past the per-server bound");
            assert!(
                cap <= remaining,
                "{cap:?} past the {remaining:?} the attempt has left"
            );
            let spent = cap * u32::try_from(ahead).expect("servers ahead");
            assert!(
                spent <= remaining - cap,
                "the servers ahead spent {spent:?} of {remaining:?} and left the \
                 remainder's server less than one of their shares"
            );
        }
    }
}

/// A stream host can hold a connection open without answering; the
/// transport gives such a fetch ten seconds, and the walk of one
/// provider has twenty in all, part of them spent on the search and
/// the listings before any server is asked. Unbounded, one such
/// server spends what the later servers needed and the whole
/// attempt times out with a healthy server unasked. Each server's
/// chain has its own bound, and a server that stalls past it is
/// stepped over like one that refused.
#[tokio::test]
async fn a_server_that_stalls_past_its_budget_is_stepped_over_for_the_next_server() {
    let c = client_with_server_budget(100);
    let started = std::time::Instant::now();
    let stream = c.stream_for(21446, "sub", "720").await.expect("resolved");
    assert_eq!(stream.url, "https://mp.example/v/index-f2.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://megaplay.buzz/"));
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "the stalled server was cut off at its budget, not waited out: {:?}",
        started.elapsed()
    );
    let urls: Vec<String> = c
        .transport()
        .requests()
        .iter()
        .map(|r| r.url.clone())
        .collect();
    assert!(
        urls.contains(&"https://hls.example/v/stalling/master.m3u8".to_string()),
        "the stalling master was asked before the next server: {urls:?}"
    );
}

/// The per-server bound holds time back for the servers still to
/// come; the last server has nobody to hold time back for, so it
/// runs on the walk's own remainder — the provider attempt's
/// deadline above the client, and the transport's per-request wait
/// below it — and a healthy chain slower than the bound is served.
#[tokio::test]
async fn a_lone_server_slower_than_the_per_server_budget_is_still_served() {
    let c = client_with_server_budget(100);
    let stream = c.stream_for(21448, "sub", "720").await.expect("served");
    assert_eq!(stream.url, "https://hls.example/v/slow/720/index.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://zokoanime.video/"));
}

#[tokio::test]
async fn the_last_server_runs_on_the_remainder_after_an_earlier_one_was_cut_off() {
    let c = client_with_server_budget(100);
    let stream = c.stream_for(21449, "sub", "720").await.expect("served");
    assert_eq!(stream.url, "https://mp.example/v/slow/index-f2.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://megaplay.buzz/"));
}

/// The site serves megaplay's player from numbered mirrors as well
/// as its own host — `megaplay-1.buzz` beside `megaplay.buzz` — and
/// the client reads a mirror's page by its shape like the host's.
/// A mirror is a host the client reads, so a lone mirror server
/// runs on the walk's remainder like any last readable server,
/// rather than being cut off at the bound while nothing trails it.
#[tokio::test]
async fn a_lone_mirror_server_slower_than_the_per_server_budget_is_still_served() {
    let c = client_with_server_budget(100);
    let stream = c.stream_for(21452, "sub", "720").await.expect("served");
    assert_eq!(stream.url, "https://mp.example/v/mirror/index-f2.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://megaplay-1.buzz/"));
}

/// A mirror sorts with the hosts the client reads, ahead of the
/// hosts it never read, whatever order the site listed them in.
#[test]
fn a_megaplay_mirror_sorts_with_the_readable_hosts() {
    let servers = vec![
        ServerEmbed {
            mode: "sub".into(),
            name: "VidTube".into(),
            embed_url: "https://vidtube.site/embed/1/sub".into(),
        },
        ServerEmbed {
            mode: "sub".into(),
            name: "MegaPlay".into(),
            embed_url: "https://megaplay-1.buzz/stream/s-2/1/sub".into(),
        },
    ];
    let ordered: Vec<&str> = servers_for(&servers, "sub")
        .into_iter()
        .map(|s| s.embed_url.as_str())
        .collect();
    assert_eq!(
        ordered,
        vec![
            "https://megaplay-1.buzz/stream/s-2/1/sub",
            "https://vidtube.site/embed/1/sub"
        ],
        "the mirror is a host the client reads"
    );
}

/// The site lists the hosts the client reads first and the rest
/// after them, so a listing can end with a host the client never
/// read. The remainder belongs to the last server the walk can use,
/// not to the last entry: a readable server whose chain is slower
/// than the bound is still served when only unread hosts trail it,
/// which would otherwise be cut off while the trailing page took the
/// remainder and answered nothing.
#[tokio::test]
async fn the_last_readable_server_runs_on_the_remainder_when_unread_hosts_trail_it() {
    let c = client_with_server_budget(100);
    let stream = c.stream_for(21450, "sub", "720").await.expect("served");
    assert_eq!(stream.url, "https://hls.example/v/slow/720/index.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://zokoanime.video/"));
}

#[tokio::test]
async fn an_earlier_readable_server_is_still_cut_off_when_unread_hosts_trail_the_last() {
    let c = client_with_server_budget(100);
    let started = std::time::Instant::now();
    let stream = c.stream_for(21451, "sub", "720").await.expect("served");
    assert_eq!(stream.url, "https://mp.example/v/slow/index-f2.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://megaplay.buzz/"));
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "the stalling server was cut off at its bound: {:?}",
        started.elapsed()
    );
}

/// A page's shape says whether the client reads it, whatever its
/// host, so a listing with no host the client names may still hold
/// a readable server, and which one cannot be known before its page
/// is fetched. The remainder then belongs to the listing's last
/// server: a lone server on an unnamed host whose page carries the
/// payload shape is served, slow chain and all.
#[tokio::test]
async fn a_lone_server_on_an_unnamed_host_runs_on_the_remainder() {
    let c = client_with_server_budget(100);
    let stream = c.stream_for(21453, "sub", "720").await.expect("served");
    assert_eq!(stream.url, "https://hls.example/v/moved/720/index.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://newembed.example/"));
}

/// The attempt above the walk has one budget for the search, the
/// candidate, the listings and the servers together, and part of it
/// is spent before the first server is asked. A fixed bound per
/// server then lets two stalled servers spend most of what is left,
/// and the last server — the one that runs on the remainder — gets a
/// remainder too small for a healthy chain. Told the attempt's
/// deadline, the walk gives each bounded server its share of what
/// remains after one chain's worth is held back for the last, so the
/// stalled ones are cut short enough for the healthy one to be
/// served inside the attempt.
#[tokio::test]
async fn stalled_servers_share_the_attempts_remainder_so_the_last_server_is_still_served() {
    let c = client_with_server_budget(100);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(180);
    c.bound_attempt(Some(deadline));
    let stream = tokio::time::timeout_at(deadline, c.stream_for(21467, "sub", "720"))
        .await
        .expect("the attempt's deadline was not spent on the stalled servers")
        .expect("served");
    assert_eq!(stream.url, "https://mp.example/v/index-f2.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://megaplay.buzz/"));
}

/// What is held back for the server that runs on the remainder has
/// to be a whole chain's worth, and a chain is four sequential
/// requests: megaplay's embed page, its sources answer, the master
/// playlist and the chosen rendition. A reserve of one per-server
/// bound is one request's worth of patience spread over four, so a
/// healthy chain on a loaded host — every request far inside the
/// transport's own wait — outlasts it, and the attempt cancels the
/// last server the walk had left. Two stalled servers ahead of it
/// share what the attempt has left after the reserve; the chain,
/// slower than one bound and well inside the reserve, is served.
#[tokio::test(start_paused = true)]
async fn the_reserve_covers_a_whole_chain_so_a_loaded_last_server_is_served() {
    let c = client_with_server_budget(100);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(200);
    c.bound_attempt(Some(deadline));
    let stream = tokio::time::timeout_at(deadline, c.stream_for(21469, "sub", "720"))
        .await
        .expect("the stalled servers left the last one its chain's worth of the attempt")
        .expect("served");
    assert_eq!(stream.url, "https://mp.example/v/paced/index-f2.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://megaplay.buzz/"));
}

/// Without an attempt deadline — the client driven outside the walk
/// — every bounded server keeps the fixed bound: the two stalled
/// servers each spend it before the last is served.
#[tokio::test]
async fn without_an_attempt_deadline_each_stalled_server_spends_the_fixed_bound() {
    let c = client_with_server_budget(100);
    let started = std::time::Instant::now();
    let stream = c.stream_for(21467, "sub", "720").await.expect("served");
    assert_eq!(stream.url, "https://mp.example/v/index-f2.m3u8");
    let elapsed = started.elapsed();
    assert!(
        elapsed >= std::time::Duration::from_millis(190)
            && elapsed < std::time::Duration::from_secs(5),
        "two stalled servers, each cut off at the fixed bound: {elapsed:?}"
    );
}

/// A client outside an attempt carries no deadline. The walk above
/// tells the client the attempt's deadline and clears it before
/// handing the client back, and a range download resolves every
/// later episode against that client: a deadline that outlived the
/// attempt would cap each bounded server at nothing once the window
/// had passed, and a later episode would fail with a healthy server
/// unasked. Told a deadline already past and then none, the client
/// serves a bounded server that answers inside the fixed bound.
#[tokio::test]
async fn a_client_told_no_deadline_keeps_the_fixed_bound_for_a_later_resolve() {
    let c = client_with_server_budget(100);
    c.bound_attempt(Some(
        tokio::time::Instant::now() - std::time::Duration::from_secs(1),
    ));
    c.bound_attempt(None);
    let stream = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        c.stream_for(21468, "sub", "720"),
    )
    .await
    .expect("the bounded server was served inside its bound")
    .expect("served");
    assert_eq!(stream.url, "https://hls.example/v/brief/720/index.m3u8");
}

/// The attempt's remainder can come down to the reserve itself —
/// the search, the candidate and the listings are spent before the
/// first server is asked — and the servers ahead of the remainder's
/// still have to be asked. Here the first server is healthy and
/// answers well inside what is left, and the server that runs on the
/// remainder is the one whose master never answers: capped at
/// nothing, the healthy server would be stepped over unasked and the
/// attempt spent waiting on the silent one.
#[tokio::test]
async fn a_healthy_server_is_still_asked_when_the_attempt_has_only_the_reserve_left() {
    let c = client_with_server_budget(100);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(150);
    c.bound_attempt(Some(deadline));
    let stream = tokio::time::timeout_at(deadline, c.stream_for(21468, "sub", "720"))
        .await
        .expect("the healthy server was skipped and the attempt waited on the stalling one")
        .expect("served");
    assert_eq!(stream.url, "https://hls.example/v/brief/720/index.m3u8");
    assert_eq!(stream.referer.as_deref(), Some("https://zokoanime.video/"));
}

/// The control: a deadline already past, never cleared, caps the
/// bounded server at nothing, and the walk moves to the remainder's
/// server, whose master never answers.
#[tokio::test]
async fn a_stale_deadline_caps_a_healthy_bounded_server_at_nothing() {
    let c = client_with_server_budget(100);
    c.bound_attempt(Some(
        tokio::time::Instant::now() - std::time::Duration::from_secs(1),
    ));
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        c.stream_for(21468, "sub", "720"),
    )
    .await;
    assert!(
        outcome.is_err(),
        "the bounded server was cut off at nothing and the walk waits on the stalling one: {outcome:?}"
    );
}

/// The limit that rule accepts: with no host the client names, the
/// remainder goes to the last listed server, and an earlier server
/// on an unnamed host is held to the bound even when its page would
/// have read — the walk cannot tell before the fetch, and a trailing
/// host must not take the remainder from a named one when there is
/// one. Here the last is a host the client never read, so the walk
/// ends on the earlier server's cut-off.
#[tokio::test]
async fn an_unnamed_host_ahead_of_the_last_listed_server_keeps_the_bound() {
    let c = client_with_server_budget(100);
    let started = std::time::Instant::now();
    let err = c
        .stream_for(21454, "sub", "720")
        .await
        .expect_err("the readable page's chain was cut off, the last page read nothing");
    assert!(matches!(err, AniError::Timeout), "{err:?}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "the earlier server was cut off at its bound: {:?}",
        started.elapsed()
    );
}

/// When every server stalls, the earlier ones are cut off at the
/// per-server bound and the last is the transport's to bound; the
/// walk surfaces the first cut-off, the two verdicts ranking the
/// same.
#[tokio::test]
async fn every_server_stalling_surfaces_a_timeout() {
    let c = client_with_server_budget(100);
    let err = c
        .stream_for(21447, "sub", "720")
        .await
        .expect_err("no server answered in time");
    assert!(matches!(err, AniError::Timeout), "{err:?}");
}
