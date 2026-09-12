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

/// The site's server list as captured on 2026-09-08, two days after
/// the first capture: `HD-1` and `HD-2` are megaplay.buzz now, and
/// the zokoanime server — the one whose page carries the payload —
/// is listed under its own name. The names rotate; the page shape
/// is what the client can read.
const SERVERS_RENAMED: &str = r#"{"status":true,"html":"<div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9zdWI/cz10Y2Ru\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"HD-2\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9zdWI/cz1iY2Ru\"></div><div class=\"item server-item\" data-type=\"sub\" data-server-name=\"ZokoAnime\" data-hash=\"aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC8xNzM1LzM5MS9zdWI=\"></div><div class=\"item server-item\" data-type=\"dub\" data-server-name=\"HD-1\" data-hash=\"aHR0cHM6Ly9tZWdhcGxheS5idXp6L3N0cmVhbS9zLTIvODI3Mi9kdWI/cz10Y2Ru\"></div>"}"#;

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
            src: "https://hls.example/v/subs/en.vtt".into(),
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
            "https://zokoanime.video/stream/mal/9/timeout/sub" => Err(AniError::Timeout),
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
        }
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
