//! Property coverage for the pure hianime parsers — the site's
//! markup, envelopes and embed payload generated rather than
//! tabulated, so the failures that live in shapes a table does not
//! think to write get written.

use super::*;
use crate::error::AniError;
use crate::scraper::provider::{BrowseHit, EpisodeRef};
use base64::Engine as _;
use proptest::prelude::*;

/// A subtitle row as the generator wrote it — language, label,
/// default flag, source — for comparing against what decoded.
type WrittenTrack = (String, String, bool, String);

/// A title as the site would print it in an attribute: the four
/// entities the parser decodes, encoded.
fn encode_title(title: &str) -> String {
    title
        .replace('&', "&amp;")
        .replace('\'', "&#039;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A result list as the site renders it: every card leads with its
/// poster link — an href and title that are not the card's identity —
/// ahead of the detail block whose anchor is.
fn search_page(cards: &[(String, u64, String, String)]) -> String {
    let with_slugs: Vec<(String, String, String)> = cards
        .iter()
        .map(|(words, id, title, kind)| (format!("{words}-{id}"), title.clone(), kind.clone()))
        .collect();
    search_page_with_slugs(&with_slugs)
}

/// The same list with each card's slug given whole, so a card can
/// carry a slug of any shape.
fn search_page_with_slugs(cards: &[(String, String, String)]) -> String {
    let mut page = String::from(
        r#"<html><body><section class="block_area block_area_sidebar"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/decoy-1" title="Decoy">Decoy</a></h3></div></section><div class="film_list-wrap">"#,
    );
    for (slug, title, kind) in cards {
        page.push_str(&format!(
            r#"<div class="flw-item"><div class="film-poster"><a href="https://hianime.at/watch/poster-{slug}" class="film-poster-ahref item-qtip" title="Poster {slug}"></a></div><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/{slug}" title="{}" class="dynamic-name">x</a></h3><div class="fd-infor"><span class="fdi-item">{kind}</span><span class="dot"></span><span class="fdi-item fdi-duration">24m</span></div></div></div>"#,
            encode_title(title)
        ));
    }
    page.push_str(r#"</div><div id="main-sidebar"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/decoy-2" title="Decoy">Decoy</a></h3></div></div></body></html>"#);
    page
}

/// A failure a host can hand the server loop: an upstream status of
/// any shape, a rate limit, a dropped connection, or a page the
/// client could not read.
fn arb_weather() -> impl Strategy<Value = AniError> {
    prop_oneof![
        (100u16..600).prop_map(|status| AniError::Upstream { status }),
        "[a-z ]{1,20}".prop_map(|detail| AniError::ParseFailed { detail }),
        proptest::option::of(0u64..10_000)
            .prop_map(|retry_after_secs| AniError::RateLimited { retry_after_secs }),
        proptest::bool::ANY.prop_map(|dropped| if dropped {
            AniError::Network
        } else {
            AniError::Timeout
        }),
    ]
}

fn envelope(html: &str) -> String {
    serde_json::json!({"status": true, "html": html}).to_string()
}

/// The episode ref the reader gives the row at `index` (0-based) that
/// the site numbered `number`: the position is the slot, and the
/// number is the display tag unless it is the position itself.
fn episode_row(index: usize, number: &str, id: u64) -> EpisodeRef {
    let slot = u32::try_from(index + 1).expect("a listing fits");
    EpisodeRef {
        id,
        number: slot,
        number2: (number != slot.to_string()).then(|| number.to_string()),
    }
}

proptest::proptest! {
    /// Every card inside the result list comes back as its hit, in
    /// order, title decoded, badge kept, read from its detail anchor
    /// and never from the poster link that leads it — and the decoy
    /// cards outside the list never do, however many results there
    /// are.
    #[test]
    fn search_cards_round_trip(
        cards in proptest::collection::vec(
            (
                "[a-z0-9]{1,6}(-[a-z0-9]{1,6}){0,3}",
                1u64..1_000_000,
                "[A-Za-z0-9,:!&'\"<>-][A-Za-z0-9 ,:!&'\"<>-]{0,29}",
                "(TV|Movie|OVA|ONA|Special)",
            ),
            1..5,
        )
    ) {
        let page = search_page(&cards);
        let expected: Vec<BrowseHit> = cards
            .iter()
            .map(|(words, id, title, kind)| BrowseHit {
                slug: format!("{words}-{id}"),
                title: title.clone(),
                kind: Some(kind.clone()),
            })
            .collect();
        proptest::prop_assert_eq!(parse_search(&page).expect("search page"), expected);
    }

    /// A card whose slug carries no decimal tail cannot be resolved
    /// and never comes back: the resolvable cards come back in order,
    /// and a list of only unresolvable cards is refused rather than
    /// read as results.
    #[test]
    fn cards_whose_slug_carries_no_id_never_come_back(
        cards in proptest::collection::vec(
            (
                "[a-z]{1,6}(-[a-z]{1,6}){0,3}",
                proptest::option::of(1u64..1_000_000),
                "[A-Za-z0-9,:!&'\"<>-][A-Za-z0-9 ,:!&'\"<>-]{0,29}",
                "(TV|Movie|OVA|ONA|Special)",
            ),
            1..5,
        )
    ) {
        let with_slugs: Vec<(String, String, String)> = cards
            .iter()
            .map(|(words, id, title, kind)| {
                let slug = id.map_or_else(|| words.clone(), |id| format!("{words}-{id}"));
                (slug, title.clone(), kind.clone())
            })
            .collect();
        let page = search_page_with_slugs(&with_slugs);
        let expected: Vec<BrowseHit> = cards
            .iter()
            .filter_map(|(words, id, title, kind)| {
                id.map(|id| BrowseHit {
                    slug: format!("{words}-{id}"),
                    title: title.clone(),
                    kind: Some(kind.clone()),
                })
            })
            .collect();
        if expected.is_empty() {
            let refused = matches!(parse_search(&page), Err(AniError::ParseFailed { .. }));
            proptest::prop_assert!(refused, "a list of only unresolvable cards is a changed shape");
        } else {
            proptest::prop_assert_eq!(parse_search(&page).expect("search page"), expected);
        }
    }

    /// A result list whose card boundary is not the one the parser
    /// splits on — renamed, or the list rendered with nothing inside
    /// and no notice — is refused however many cards it holds: the
    /// site's no-results page carries the notice and no list, so a
    /// list with no boundary inside is a changed shape, and read as
    /// "no results" it would persist absence for every title.
    #[test]
    fn a_result_list_without_the_card_boundary_is_refused(
        cards in proptest::collection::vec(
            (
                "[a-z0-9]{1,6}(-[a-z0-9]{1,6}){0,3}",
                1u64..1_000_000,
                "[A-Za-z0-9,:!&'\"<>-][A-Za-z0-9 ,:!&'\"<>-]{0,29}",
                "(TV|Movie|OVA|ONA|Special)",
            ),
            0..5,
        ),
        boundary in "[a-z]{2,8}(-[a-z]{2,8})?",
    ) {
        proptest::prop_assume!(boundary != "flw-item");
        proptest::prop_assume!(!cards.iter().any(|(words, _, title, _)| words.contains("flw-item") || title.contains("flw-item")));
        let page = search_page(&cards).replace("flw-item", &boundary);
        let refused = matches!(parse_search(&page), Err(AniError::ParseFailed { .. }));
        proptest::prop_assert!(refused, "a list without the card boundary read as an answer: {page}");
    }

    /// A body that shows neither the result list nor the no-results
    /// notice is refused whatever else it contains.
    #[test]
    fn a_page_without_the_search_shape_is_refused(body in ".*") {
        proptest::prop_assume!(!body.contains("film_list-wrap") && !body.contains("No animes found"));
        let refused = matches!(parse_search(&body), Err(AniError::ParseFailed { .. }));
        proptest::prop_assert!(refused);
    }

    /// The trailing decimal is the id, whatever words precede it.
    #[test]
    fn slug_id_is_exactly_the_decimal_tail(
        words in proptest::collection::vec("[a-z0-9]{1,8}", 1..6),
        id in 0u64..1_000_000,
    ) {
        proptest::prop_assert_eq!(slug_id(&format!("{}-{id}", words.join("-"))), Some(id));
    }

    /// The year is the first four-digit run of the Aired value.
    #[test]
    fn detail_year_is_the_aired_start(
        month in "(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)",
        day in 1u32..29,
        year in 1917u32..2100,
        end in "(to \\?|to Dec 31, 2099|)",
    ) {
        let page = format!(
            r#"<div class="item item-title"><span class="item-head">Aired:</span><span class="name">{month} {day}, {year} {end}</span></div>"#
        );
        proptest::prop_assert_eq!(parse_detail_year(&page), Some(year));
    }

    /// The year comes from the Aired row or from nowhere: a page
    /// whose row is intact answers the row's year, one whose row
    /// lost its value span answers none, and a later `name` element
    /// carrying its own four-digit number never answers for either.
    #[test]
    fn detail_year_never_comes_from_outside_the_aired_row(
        month in "(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)",
        day in 1u32..29,
        year in 1917u32..2100,
        decoy in 1000u32..10000,
        intact in proptest::bool::ANY,
        wrapped in proptest::bool::ANY,
    ) {
        let value = if intact {
            format!(r#"<span class="name">{month} {day}, {year}</span>"#)
        } else {
            format!("<span>{month} {day}, {year}</span>")
        };
        let row = if wrapped {
            format!(r#"<div class="item item-title"><span class="item-head">Aired:</span>{value}</div>"#)
        } else {
            format!(r#"<span class="item-head">Aired:</span>{value}"#)
        };
        let page = format!(
            r#"{row}<div class="item item-list"><span class="item-head">Studios:</span><a class="name">Studio {decoy}</a></div>"#
        );
        let expected = if intact { Some(year) } else { None };
        proptest::prop_assert_eq!(parse_detail_year(&page), expected);
    }

    /// A card whose title is blank never comes back, whatever its
    /// slug; the cards with a title do, in order.
    #[test]
    fn cards_with_a_blank_title_never_come_back(
        cards in proptest::collection::vec(
            (
                "[a-z]{1,6}(-[a-z]{1,6}){0,3}",
                1u64..1_000_000,
                proptest::option::of("[A-Za-z0-9,:!-][A-Za-z0-9 ,:!-]{0,20}"),
                " {0,3}",
                "(TV|Movie|OVA|ONA|Special)",
            ),
            1..5,
        )
    ) {
        let with_slugs: Vec<(String, String, String)> = cards
            .iter()
            .map(|(words, id, title, pad, kind)| {
                let title = title.as_ref().map_or_else(|| pad.clone(), |t| format!("{pad}{t}{pad}"));
                (format!("{words}-{id}"), title, kind.clone())
            })
            .collect();
        let page = search_page_with_slugs(&with_slugs);
        let expected: Vec<String> = cards
            .iter()
            .filter(|(_, _, title, _, _)| title.is_some())
            .map(|(words, id, _, _, _)| format!("{words}-{id}"))
            .collect();
        match parse_search(&page) {
            Ok(hits) => {
                let slugs: Vec<String> = hits.iter().map(|h| h.slug.clone()).collect();
                proptest::prop_assert_eq!(slugs, expected);
            }
            Err(AniError::ParseFailed { .. }) => proptest::prop_assert!(expected.is_empty(), "refused with readable cards present"),
            Err(e) => proptest::prop_assert!(false, "unexpected error {e:?}"),
        }
    }

    /// A card's identity is one anchor's href and title together,
    /// in whichever order the site writes them; a card whose detail
    /// block spreads the two over different anchors is unreadable
    /// and never comes back as a hit stitched from both. The
    /// readable cards come back in order, and a list of only
    /// unreadable ones is refused.
    #[test]
    fn a_hit_is_never_stitched_from_two_anchors(
        cards in proptest::collection::vec(
            (
                "[a-z]{1,6}(-[a-z]{1,6}){0,3}",
                1u64..1_000_000,
                "[A-Za-z0-9,:!-][A-Za-z0-9 ,:!-]{0,20}",
                "(TV|Movie|OVA|ONA|Special)",
                proptest::sample::select(vec!["href-title", "title-href", "title-then-href", "href-then-title"]),
            ),
            1..5,
        )
    ) {
        let mut page = String::from(r#"<html><body><div class="film_list-wrap">"#);
        for (words, id, title, kind, shape) in &cards {
            let slug = format!("{words}-{id}");
            let title = encode_title(title);
            let detail = match *shape {
                "href-title" => format!(r#"<h3 class="film-name"><a href="https://hianime.at/{slug}" title="{title}" class="dynamic-name">x</a></h3>"#),
                "title-href" => format!(r#"<h3 class="film-name"><a class="dynamic-name" title="{title}" href="https://hianime.at/{slug}">x</a></h3>"#),
                "title-then-href" => format!(r#"<h3 class="film-name"><a title="{title}" class="dynamic-name">x</a></h3><div class="fd-infor"><a href="https://hianime.at/{slug}">more</a></div>"#),
                _ => format!(r#"<h3 class="film-name"><a href="https://hianime.at/{slug}" class="dynamic-name">x</a></h3><div class="fd-infor"><a title="{title}">more</a></div>"#),
            };
            page.push_str(&format!(
                r#"<div class="flw-item"><div class="film-poster"><a href="https://hianime.at/watch/poster-{slug}" class="film-poster-ahref" title="Poster"></a></div><div class="film-detail">{detail}<div class="fd-infor"><span class="fdi-item">{kind}</span></div></div></div>"#
            ));
        }
        page.push_str("</div></body></html>");
        let expected: Vec<BrowseHit> = cards
            .iter()
            .filter(|(_, _, _, _, shape)| matches!(*shape, "href-title" | "title-href"))
            .map(|(words, id, title, kind, _)| BrowseHit {
                slug: format!("{words}-{id}"),
                title: title.clone(),
                kind: Some(kind.clone()),
            })
            .collect();
        match parse_search(&page) {
            Ok(hits) => proptest::prop_assert_eq!(hits, expected),
            Err(AniError::ParseFailed { .. }) => proptest::prop_assert!(expected.is_empty(), "refused with readable cards present"),
            Err(e) => proptest::prop_assert!(false, "unexpected error {e:?}"),
        }
    }

    /// Every ep-item row comes back as its episode ref, in order:
    /// its position in the listing is its slot, and the site's number
    /// rides as the display tag whenever it is not that position.
    #[test]
    fn episode_rows_round_trip(
        rows in proptest::collection::vec((1u32..5000, 1u64..1_000_000_000), 1..8)
    ) {
        let html: String = rows
            .iter()
            .map(|(number, id)| format!(r#"<a class="ssl-item ep-item" data-number="{number}" data-id="{id}" href="/watch/x?ep={id}"><div class="ssli-order">{number}</div></a>"#))
            .collect();
        let expected: Vec<EpisodeRef> = rows
            .iter()
            .enumerate()
            .map(|(i, (number, id))| episode_row(i, &number.to_string(), *id))
            .collect();
        proptest::prop_assert_eq!(parse_episode_list(&envelope(&html)).expect("listing"), expected);
    }

    /// A listing mixing whole numbers with the site's recap and
    /// special numbers — `7.5`, `12.25` — loses no row: each keeps
    /// its position as its slot and its number as its display tag,
    /// so the fractional ones can be advertised and resolved by tag.
    #[test]
    fn episode_rows_keep_fractional_numbers_as_tags(
        rows in proptest::collection::vec(
            (
                prop_oneof![
                    (1u32..5000).prop_map(|n| n.to_string()),
                    (1u32..5000, prop_oneof![Just("5"), Just("25"), Just("75")])
                        .prop_map(|(n, f)| format!("{n}.{f}")),
                ],
                1u64..1_000_000_000,
            ),
            1..8,
        )
    ) {
        let html: String = rows
            .iter()
            .map(|(number, id)| format!(r#"<a class="ssl-item ep-item" data-number="{number}" data-id="{id}"></a>"#))
            .collect();
        let expected: Vec<EpisodeRef> = rows
            .iter()
            .enumerate()
            .map(|(i, (number, id))| episode_row(i, number, *id))
            .collect();
        let parsed = parse_episode_list(&envelope(&html)).expect("listing");
        proptest::prop_assert_eq!(&parsed, &expected);
        let tags: Vec<String> = parsed
            .iter()
            .map(|e| e.number2.clone().unwrap_or_else(|| e.number.to_string()))
            .collect();
        let written: Vec<String> = rows.iter().map(|(n, _)| n.clone()).collect();
        proptest::prop_assert_eq!(tags, written, "every row's display is the site's number");
    }

    /// The row's attributes come in whatever order the site writes
    /// them — the marker class first, last, or between — and every
    /// row still comes back as its episode ref, in order.
    #[test]
    fn episode_rows_round_trip_whatever_order_their_attributes_come_in(
        rows in proptest::collection::vec(
            (1u32..5000, 1u64..1_000_000_000, Just(vec![0usize, 1, 2, 3]).prop_shuffle()),
            1..8,
        )
    ) {
        let html: String = rows
            .iter()
            .map(|(number, id, order)| {
                let attrs = [
                    r#"class="ssl-item ep-item""#.to_string(),
                    format!(r#"data-number="{number}""#),
                    format!(r#"data-id="{id}""#),
                    format!(r#"href="/watch/x?ep={id}""#),
                ];
                let tag: Vec<&str> = order.iter().map(|i| attrs[*i].as_str()).collect();
                format!(r#"<a {}><div class="ssli-order">{number}</div></a>"#, tag.join(" "))
            })
            .collect();
        let expected: Vec<EpisodeRef> = rows
            .iter()
            .enumerate()
            .map(|(i, (number, id, _))| episode_row(i, &number.to_string(), *id))
            .collect();
        proptest::prop_assert_eq!(parse_episode_list(&envelope(&html)).expect("listing"), expected);
    }

    /// The listing container with only whitespace inside is the
    /// site's shape for an entry without episodes, whatever chrome
    /// surrounds it; the same container holding anything else and
    /// no row the parser knows is a changed shape, refused.
    #[test]
    fn a_blank_listing_container_is_no_episodes_and_a_filled_one_without_rows_is_refused(
        chrome_before in "[^<>\"]{0,40}",
        chrome_after in "[^<>\"]{0,40}",
        inside in "[ \t\n\r]{0,12}",
        filler in "[a-z][a-z ]{0,20}",
    ) {
        let blank = format!(
            r#"<div class="seasons-block">{chrome_before}<div class="ss-list">{inside}</div>{chrome_after}</div>"#
        );
        prop_assert_eq!(parse_episode_list(&envelope(&blank)).expect("answered"), Vec::<EpisodeRef>::new());
        let filled = format!(
            r#"<div class="seasons-block">{chrome_before}<div class="ss-list">{inside}<a class="ssl-item">{filler}</a></div>{chrome_after}</div>"#
        );
        let refused = matches!(parse_episode_list(&envelope(&filled)), Err(AniError::ParseFailed { .. }));
        prop_assert!(refused, "a filled container without rows: {filled}");
    }

    /// Every server row comes back with its type, its name and the
    /// URL its hash encodes, in order.
    #[test]
    fn server_rows_round_trip(
        rows in proptest::collection::vec(
            ("(sub|dub)", "(HD-1|HD-2|HD-3)", "[a-z]{2,10}\\.(video|buzz|to)", "[a-z0-9/]{0,20}"),
            1..6,
        )
    ) {
        let html: String = rows
            .iter()
            .map(|(mode, name, host, path)| {
                let url = format!("https://{host}/{path}");
                let hash = base64::engine::general_purpose::STANDARD.encode(url.as_bytes());
                format!(r#"<div class="item server-item" data-type="{mode}" data-server-name="{name}" data-hash="{hash}"><a class="btn">{name}</a></div>"#)
            })
            .collect();
        let expected: Vec<ServerEmbed> = rows
            .iter()
            .map(|(mode, name, host, path)| ServerEmbed {
                mode: mode.clone(),
                name: name.clone(),
                embed_url: format!("https://{host}/{path}"),
            })
            .collect();
        proptest::prop_assert_eq!(parse_servers(&envelope(&html)).expect("servers"), expected);
    }

    /// The server row reads the same whatever order its attributes
    /// come in: every row comes back with its type, name and URL,
    /// in order, and no mode is marked unreadable or unknown.
    #[test]
    fn server_rows_round_trip_whatever_order_their_attributes_come_in(
        rows in proptest::collection::vec(
            (
                "(sub|dub)",
                "(HD-1|HD-2|HD-3)",
                "[a-z]{2,10}\\.(video|buzz|to)",
                "[a-z0-9/]{0,20}",
                Just(vec![0usize, 1, 2, 3]).prop_shuffle(),
            ),
            1..6,
        )
    ) {
        let html: String = rows
            .iter()
            .map(|(mode, name, host, path, order)| {
                let url = format!("https://{host}/{path}");
                let hash = base64::engine::general_purpose::STANDARD.encode(url.as_bytes());
                let attrs = [
                    r#"class="item server-item""#.to_string(),
                    format!(r#"data-type="{mode}""#),
                    format!(r#"data-server-name="{name}""#),
                    format!(r#"data-hash="{hash}""#),
                ];
                let tag: Vec<&str> = order.iter().map(|i| attrs[*i].as_str()).collect();
                format!(r#"<div {}><a class="btn">{name}</a></div>"#, tag.join(" "))
            })
            .collect();
        let expected: Vec<ServerEmbed> = rows
            .iter()
            .map(|(mode, name, host, path, _)| ServerEmbed {
                mode: mode.clone(),
                name: name.clone(),
                embed_url: format!("https://{host}/{path}"),
            })
            .collect();
        let listing = parse_server_listing(&envelope(&html)).expect("listing");
        proptest::prop_assert_eq!(listing.servers, expected);
        proptest::prop_assert!(listing.unreadable_modes.is_empty(), "{:?}", listing.unreadable_modes);
        proptest::prop_assert!(!listing.unknown_modes);
    }

    /// The listing keeps its uncertainty per mode. Over rows of either
    /// mode, readable or not: the readable rows come back as servers
    /// in order; a mode is marked unreadable exactly when at least one
    /// of its rows could not be read, whether or not another of its
    /// rows could; a mode answers present when it has a readable
    /// server, uncertain when it has none but an unreadable row, and
    /// absent when the listing carried no row of it; and a listing
    /// with no readable row at all is refused.
    #[test]
    fn a_mode_is_uncertain_exactly_when_its_rows_were_unreadable(
        rows in proptest::collection::vec(
            ("(sub|dub)", "(HD-1|HD-2|HD-3)", "[a-z]{2,10}\\.(video|buzz|to)", "[a-z0-9/]{0,20}", proptest::bool::ANY),
            1..6,
        )
    ) {
        let html: String = rows
            .iter()
            .map(|(mode, name, host, path, readable)| {
                let hash = if *readable {
                    base64::engine::general_purpose::STANDARD.encode(format!("https://{host}/{path}").as_bytes())
                } else {
                    "!!!".to_string()
                };
                format!(r#"<div class="item server-item" data-type="{mode}" data-server-name="{name}" data-hash="{hash}"></div>"#)
            })
            .collect();
        let expected: Vec<ServerEmbed> = rows
            .iter()
            .filter(|(_, _, _, _, readable)| *readable)
            .map(|(mode, name, host, path, _)| ServerEmbed {
                mode: mode.clone(),
                name: name.clone(),
                embed_url: format!("https://{host}/{path}"),
            })
            .collect();
        match parse_server_listing(&envelope(&html)) {
            Err(AniError::ParseFailed { .. }) => proptest::prop_assert!(expected.is_empty(), "refused with readable rows present"),
            Err(e) => proptest::prop_assert!(false, "unexpected error {e:?}"),
            Ok(listing) => {
                proptest::prop_assert_eq!(&listing.servers, &expected);
                for mode in ["sub", "dub"] {
                    let readable_of_mode = rows.iter().any(|(m, _, _, _, r)| m == mode && *r);
                    let unreadable_of_mode = rows.iter().any(|(m, _, _, _, r)| m == mode && !*r);
                    proptest::prop_assert_eq!(listing.unreadable_modes.iter().any(|m| m == mode), unreadable_of_mode);
                    match listing.mode_readable(mode) {
                        Ok(true) => proptest::prop_assert!(readable_of_mode),
                        Ok(false) => proptest::prop_assert!(!readable_of_mode && !unreadable_of_mode),
                        Err(AniError::ParseFailed { .. }) => proptest::prop_assert!(!readable_of_mode && unreadable_of_mode),
                        Err(e) => proptest::prop_assert!(false, "unexpected error {e:?}"),
                    }
                }
            }
        }
    }

    /// Over rows of known and unknown modes, readable or not, a known
    /// mode answers present with a readable server of it; else
    /// uncertain when it had a row the client could not read or the
    /// listing had a row typed with a mode the client does not know —
    /// either may have been the mode's server under another name or
    /// shape; else absent. A listing with no readable row is refused.
    #[test]
    fn a_known_mode_is_absent_only_when_every_row_was_typed_and_read(
        rows in proptest::collection::vec(
            ("(sub|dub|softsub|raw|hardsub)", "(HD-1|HD-2|HD-3)", "[a-z]{2,10}\\.(video|buzz|to)", "[a-z0-9/]{0,20}", proptest::bool::ANY),
            1..6,
        )
    ) {
        let html: String = rows
            .iter()
            .map(|(mode, name, host, path, readable)| {
                let hash = if *readable {
                    base64::engine::general_purpose::STANDARD.encode(format!("https://{host}/{path}").as_bytes())
                } else {
                    "!!!".to_string()
                };
                format!(r#"<div class="item server-item" data-type="{mode}" data-server-name="{name}" data-hash="{hash}"></div>"#)
            })
            .collect();
        let known = |m: &str| matches!(m, "sub" | "dub");
        let expected: Vec<ServerEmbed> = rows
            .iter()
            .filter(|(mode, _, _, _, readable)| known(mode) && *readable)
            .map(|(mode, name, host, path, _)| ServerEmbed {
                mode: mode.clone(),
                name: name.clone(),
                embed_url: format!("https://{host}/{path}"),
            })
            .collect();
        let unknown_seen = rows.iter().any(|(m, _, _, _, _)| !known(m));
        match parse_server_listing(&envelope(&html)) {
            Err(AniError::ParseFailed { .. }) => proptest::prop_assert!(expected.is_empty(), "refused with readable rows present"),
            Err(e) => proptest::prop_assert!(false, "unexpected error {e:?}"),
            Ok(listing) => {
                proptest::prop_assert_eq!(&listing.servers, &expected);
                for mode in ["sub", "dub"] {
                    let readable_of_mode = rows.iter().any(|(m, _, _, _, r)| m == mode && *r);
                    let unreadable_of_mode = rows.iter().any(|(m, _, _, _, r)| m == mode && !*r);
                    match listing.mode_readable(mode) {
                        Ok(true) => proptest::prop_assert!(readable_of_mode),
                        Ok(false) => proptest::prop_assert!(!readable_of_mode && !unreadable_of_mode && !unknown_seen),
                        Err(AniError::ParseFailed { .. }) => proptest::prop_assert!(!readable_of_mode && (unreadable_of_mode || unknown_seen)),
                        Err(e) => proptest::prop_assert!(false, "unexpected error {e:?}"),
                    }
                }
            }
        }
    }

    /// Over rows that carry their mode attribute or not — typed with
    /// a known mode, an unknown one, or not typed at all — a known
    /// mode answers present with a readable server of it; else
    /// uncertain when it had a row the client could not read, or the
    /// listing had a row typed with a mode the client does not know
    /// or a row with no mode attribute — any of which may have been
    /// the mode's server under another name, shape or attribute;
    /// else absent. A listing with no readable row is refused.
    #[test]
    fn a_known_mode_is_absent_only_when_every_row_carried_a_known_mode_and_was_read(
        rows in proptest::collection::vec(
            (
                proptest::option::of("(sub|dub|softsub|raw)"),
                "(HD-1|HD-2|HD-3)",
                "[a-z]{2,10}\\.(video|buzz|to)",
                "[a-z0-9/]{0,20}",
                proptest::bool::ANY,
            ),
            1..6,
        )
    ) {
        let html: String = rows
            .iter()
            .map(|(mode, name, host, path, readable)| {
                let hash = if *readable {
                    base64::engine::general_purpose::STANDARD.encode(format!("https://{host}/{path}").as_bytes())
                } else {
                    "!!!".to_string()
                };
                let typed = mode.as_ref().map_or(String::new(), |m| format!(r#" data-type="{m}""#));
                format!(r#"<div class="item server-item"{typed} data-server-name="{name}" data-hash="{hash}"></div>"#)
            })
            .collect();
        let known = |m: &Option<String>| matches!(m.as_deref(), Some("sub" | "dub"));
        let expected: Vec<ServerEmbed> = rows
            .iter()
            .filter(|(mode, _, _, _, readable)| known(mode) && *readable)
            .map(|(mode, name, host, path, _)| ServerEmbed {
                mode: mode.clone().expect("known"),
                name: name.clone(),
                embed_url: format!("https://{host}/{path}"),
            })
            .collect();
        let unknown_seen = rows.iter().any(|(m, _, _, _, _)| !known(m));
        match parse_server_listing(&envelope(&html)) {
            Err(AniError::ParseFailed { .. }) => proptest::prop_assert!(expected.is_empty(), "refused with readable rows present"),
            Err(e) => proptest::prop_assert!(false, "unexpected error {e:?}"),
            Ok(listing) => {
                proptest::prop_assert_eq!(&listing.servers, &expected);
                proptest::prop_assert_eq!(listing.unknown_modes, unknown_seen);
                for mode in ["sub", "dub"] {
                    let readable_of_mode = rows.iter().any(|(m, _, _, _, r)| m.as_deref() == Some(mode) && *r);
                    let unreadable_of_mode = rows.iter().any(|(m, _, _, _, r)| m.as_deref() == Some(mode) && !*r);
                    match listing.mode_readable(mode) {
                        Ok(true) => proptest::prop_assert!(readable_of_mode),
                        Ok(false) => proptest::prop_assert!(!readable_of_mode && !unreadable_of_mode && !unknown_seen),
                        Err(AniError::ParseFailed { .. }) => proptest::prop_assert!(!readable_of_mode && (unreadable_of_mode || unknown_seen)),
                        Err(e) => proptest::prop_assert!(false, "unexpected error {e:?}"),
                    }
                }
            }
        }
    }

    /// A row whose mode is blank never comes back: the rows with a
    /// mode come back in order, and a listing of only blank rows is
    /// refused rather than read as no servers.
    #[test]
    fn rows_with_a_blank_mode_never_come_back_and_an_all_blank_listing_is_refused(
        rows in proptest::collection::vec(
            ("(sub|dub||[ \t]{1,3})", "(HD-1|HD-2|HD-3)", "[a-z]{2,10}\\.(video|buzz|to)", "[a-z0-9/]{0,20}"),
            1..6,
        )
    ) {
        let html: String = rows
            .iter()
            .map(|(mode, name, host, path)| {
                let url = format!("https://{host}/{path}");
                let hash = base64::engine::general_purpose::STANDARD.encode(url.as_bytes());
                format!(r#"<div class="item server-item" data-type="{mode}" data-server-name="{name}" data-hash="{hash}"><a class="btn">{name}</a></div>"#)
            })
            .collect();
        let expected: Vec<ServerEmbed> = rows
            .iter()
            .filter(|(mode, _, _, _)| !mode.trim().is_empty())
            .map(|(mode, name, host, path)| ServerEmbed {
                mode: mode.clone(),
                name: name.clone(),
                embed_url: format!("https://{host}/{path}"),
            })
            .collect();
        if expected.is_empty() {
            let refused = matches!(parse_servers(&envelope(&html)), Err(AniError::ParseFailed { .. }));
            proptest::prop_assert!(refused, "an all-blank listing is a changed shape");
        } else {
            proptest::prop_assert_eq!(parse_servers(&envelope(&html)).expect("servers"), expected);
        }
    }

    /// A row whose mode is not one the client knows never comes
    /// back: the sub and dub rows come back in order, and a listing
    /// of only other modes is refused rather than read as no servers.
    #[test]
    fn rows_with_a_mode_the_client_does_not_know_never_come_back(
        rows in proptest::collection::vec(
            ("(sub|dub|softsub|raw|SUB|Dub|sub |[a-z]{1,8})", "(HD-1|HD-2|HD-3)", "[a-z]{2,10}\\.(video|buzz|to)", "[a-z0-9/]{0,20}"),
            1..6,
        )
    ) {
        let html: String = rows
            .iter()
            .map(|(mode, name, host, path)| {
                let url = format!("https://{host}/{path}");
                let hash = base64::engine::general_purpose::STANDARD.encode(url.as_bytes());
                format!(r#"<div class="item server-item" data-type="{mode}" data-server-name="{name}" data-hash="{hash}"><a class="btn">{name}</a></div>"#)
            })
            .collect();
        let expected: Vec<ServerEmbed> = rows
            .iter()
            .filter(|(mode, _, _, _)| matches!(mode.trim(), "sub" | "dub"))
            .map(|(mode, name, host, path)| ServerEmbed {
                mode: mode.trim().to_string(),
                name: name.clone(),
                embed_url: format!("https://{host}/{path}"),
            })
            .collect();
        if expected.is_empty() {
            let refused = matches!(parse_servers(&envelope(&html)), Err(AniError::ParseFailed { .. }));
            proptest::prop_assert!(refused, "a listing of only unknown modes is a changed shape");
        } else {
            proptest::prop_assert_eq!(parse_servers(&envelope(&html)).expect("servers"), expected);
        }
    }

    /// A body that is not the envelope is a parse failure, never an
    /// empty answer — a throttling page or a redesign must not read
    /// as "no episodes".
    #[test]
    fn a_non_envelope_body_is_a_parse_failure(body in ".*") {
        proptest::prop_assume!(serde_json::from_str::<serde_json::Value>(&body).map_or(true, |v| v.get("status").is_none()));
        let episodes_refused = matches!(parse_episode_list(&body), Err(AniError::ParseFailed { .. }));
        let servers_refused = matches!(parse_servers(&body), Err(AniError::ParseFailed { .. }));
        proptest::prop_assert!(episodes_refused);
        proptest::prop_assert!(servers_refused);
    }

    /// The payload survives the page's encoding: XOR under the key,
    /// base64, quoted into the script — and comes back field for field.
    #[test]
    fn embed_payload_round_trips(
        src in "https://[a-z]{2,8}\\.example/[a-z0-9/]{1,20}\\.m3u8",
        tracks in proptest::collection::vec(("[a-z]{2}", "[A-Za-z ]{1,12}", proptest::bool::ANY, "https://[a-z]{2,8}\\.example/[a-z0-9/]{1,20}\\.vtt"), 0..3),
    ) {
        let subtitles: Vec<serde_json::Value> = tracks
            .iter()
            .map(|(lang, label, default, url)| serde_json::json!({"lang": lang, "label": label, "default": default, "src": url}))
            .collect();
        let json = serde_json::json!({"src": src, "subtitles": subtitles, "skip": null}).to_string();
        let key = b"otaku-embed-v1";
        let blob = base64::engine::general_purpose::STANDARD.encode(
            json.bytes().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect::<Vec<u8>>(),
        );
        let page = format!(r#"<html><body><script>window.__P="{blob}"</script></body></html>"#);
        let payload = decode_embed(&page).expect("payload");
        proptest::prop_assert_eq!(&payload.src, &src);
        proptest::prop_assert_eq!(payload.subtitles.len(), tracks.len());
        for (got, (lang, label, default, url)) in payload.subtitles.iter().zip(&tracks) {
            proptest::prop_assert_eq!(&got.lang, lang);
            proptest::prop_assert_eq!(&got.label, label);
            proptest::prop_assert_eq!(got.default, *default);
            proptest::prop_assert_eq!(&got.src, url);
        }
    }

    /// Whatever the subtitle list looks like — absent, `null`, not a
    /// list, or a list mixing rows the client reads with rows missing
    /// a field or not objects at all — the stream comes back, and
    /// exactly the readable rows come with it, in order.
    #[test]
    fn a_payload_keeps_its_stream_and_its_readable_tracks_whatever_the_rest_of_the_list(
        src in "https://[a-z]{2,8}\\.example/[a-z0-9/]{1,20}\\.m3u8",
        list in prop_oneof![
            Just(None),
            Just(Some(serde_json::Value::Null)),
            Just(Some(serde_json::json!("en"))),
            Just(Some(serde_json::json!(3))),
            proptest::collection::vec(
                prop_oneof![
                    ("[a-z]{2}", "[A-Za-z ]{1,12}", proptest::bool::ANY, "https://[a-z]{2,8}\\.example/[a-z0-9/]{1,20}\\.vtt")
                        .prop_map(|(lang, label, default, url)| {
                            let row = serde_json::json!({"lang": lang, "label": label, "default": default, "src": url});
                            (Some((lang, label, default, url)), row)
                        }),
                    "[a-z]{2}".prop_map(|lang| (None, serde_json::json!({"lang": lang, "label": "X"}))),
                    "[a-z]{2}".prop_map(|lang| (None, serde_json::json!({"label": "X", "src": format!("https://hls.example/{lang}.vtt")}))),
                    "[a-z]{2}".prop_map(|lang| (None, serde_json::json!({"lang": lang, "src": "https://hls.example/x.vtt"}))),
                    "[a-z]{2}".prop_map(|s| (None, serde_json::json!(s))),
                    Just((None, serde_json::json!(null))),
                ],
                0..6,
            )
            .prop_map(|rows| Some(serde_json::json!({"rows": rows.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>(), "kept": rows.iter().filter_map(|(k, _)| k.clone()).collect::<Vec<_>>()}))),
        ],
    ) {
        let (subtitles, kept): (Option<serde_json::Value>, Vec<WrittenTrack>) = match list {
            Some(serde_json::Value::Object(ref m)) if m.contains_key("rows") => (
                Some(m["rows"].clone()),
                serde_json::from_value(m["kept"].clone()).expect("kept rows"),
            ),
            other => (other, Vec::new()),
        };
        let mut json = serde_json::json!({"src": src});
        if let Some(list) = subtitles {
            json["subtitles"] = list;
        }
        let key = b"otaku-embed-v1";
        let blob = base64::engine::general_purpose::STANDARD.encode(
            json.to_string().bytes().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect::<Vec<u8>>(),
        );
        let page = format!(r#"<html><body><script>window.__P="{blob}"</script></body></html>"#);
        let payload = decode_embed(&page).expect("the stream is usable");
        proptest::prop_assert_eq!(&payload.src, &src);
        proptest::prop_assert_eq!(payload.subtitles.len(), kept.len());
        for (got, (lang, label, default, url)) in payload.subtitles.iter().zip(&kept) {
            proptest::prop_assert_eq!(&got.lang, lang);
            proptest::prop_assert_eq!(&got.label, label);
            proptest::prop_assert_eq!(got.default, *default);
            proptest::prop_assert_eq!(&got.src, url);
        }
    }

    /// A payload whose source is not an absolute http(s) URL is not a
    /// stream the client can fetch: blank, relative, or under another
    /// scheme, it is refused as a parse failure whatever else it says.
    #[test]
    fn a_payload_with_an_unfetchable_source_is_refused(
        src in prop_oneof![
            Just(String::new()),
            "/[a-z0-9/]{1,20}\\.m3u8",
            "[a-z]{3,10}",
            "(ftp|file|data|javascript)://?[a-z0-9./]{1,20}",
        ],
    ) {
        let json = serde_json::json!({"src": src, "subtitles": []}).to_string();
        let key = b"otaku-embed-v1";
        let blob = base64::engine::general_purpose::STANDARD.encode(
            json.bytes().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect::<Vec<u8>>(),
        );
        let page = format!(r#"<html><body><script>window.__P="{blob}"</script></body></html>"#);
        let refused = matches!(decode_embed(&page), Err(AniError::ParseFailed { .. }));
        prop_assert!(refused, "accepted a source the client cannot fetch: {src:?}");
    }

    /// The origin is scheme and host with a trailing slash — the path
    /// never leaks into the referer.
    #[test]
    fn embed_origin_is_scheme_and_host(
        scheme in "(http|https)",
        host in "[a-z]{2,10}\\.(video|buzz)",
        path in "/[a-z0-9/]{0,20}",
    ) {
        proptest::prop_assert_eq!(
            embed_origin(&format!("{scheme}://{host}{path}")),
            Some(format!("{scheme}://{host}/"))
        );
    }

    /// The walk's verdict when no server served a stream: the kept
    /// failure as it stands, lifted to a parse failure when the mode
    /// had a row the client could not read — which never outranks a
    /// rate limit — and the answered absence only when nothing was
    /// kept and every row was read.
    #[test]
    fn the_final_verdict_keeps_the_doubt_of_an_unreadable_row(
        kept in proptest::option::of(arb_weather()),
        uncertain in proptest::bool::ANY,
    ) {
        fn rank(w: &AniError) -> u8 {
            match w {
                AniError::RateLimited { .. } | AniError::Upstream { status: 429 } => 4,
                AniError::ParseFailed { .. } => 3,
                w if w.is_provider_block() => 2,
                AniError::Network | AniError::Timeout => 1,
                _ => 0,
            }
        }
        let kept_rank = kept.as_ref().map(rank);
        let kept_repr = kept.as_ref().map(|k| format!("{k:?}"));
        let verdict = final_verdict(kept, uncertain, "sub");
        match (kept_rank, uncertain) {
            (None, false) => prop_assert!(matches!(verdict, AniError::NoResults), "{verdict:?}"),
            (None, true) => prop_assert!(matches!(verdict, AniError::ParseFailed { .. }), "{verdict:?}"),
            (Some(_), false) => prop_assert_eq!(Some(format!("{verdict:?}")), kept_repr),
            (Some(r), true) => {
                // The doubt is a parse failure; a louder kept failure
                // stands, a parse failure kept first stays, anything
                // quieter yields to the doubt.
                prop_assert_eq!(rank(&verdict), r.max(3));
                if r >= 3 {
                    prop_assert_eq!(Some(format!("{verdict:?}")), kept_repr);
                } else {
                    prop_assert!(matches!(verdict, AniError::ParseFailed { .. }), "{verdict:?}");
                }
            }
        }
    }

    /// Of two hosts' failures the kept one is the louder: a rate limit
    /// over everything, a page the client could not read over any
    /// other provider block, a block over a dropped connection or a
    /// timeout, and those over an answered status — a server never
    /// heard from may carry the stream, an answered status is that
    /// host's own dead end; between two of a rank the first stays.
    #[test]
    fn the_kept_weather_is_the_louder_of_the_two(
        first in arb_weather(),
        second in arb_weather(),
    ) {
        fn rank(w: &AniError) -> u8 {
            match w {
                AniError::RateLimited { .. } | AniError::Upstream { status: 429 } => 4,
                AniError::ParseFailed { .. } => 3,
                w if w.is_provider_block() => 2,
                AniError::Network | AniError::Timeout => 1,
                _ => 0,
            }
        }
        let first_rank = rank(&first);
        let second_rank = rank(&second);
        let first_repr = format!("{first:?}");
        let second_repr = format!("{second:?}");
        let kept = weightier(first, second);
        prop_assert_eq!(rank(&kept), first_rank.max(second_rank));
        let kept_repr = format!("{kept:?}");
        if second_rank > first_rank {
            prop_assert_eq!(kept_repr, second_repr);
        } else {
            prop_assert_eq!(kept_repr, first_repr);
        }
    }

    /// Whatever the site names its servers, the ones to try for a mode
    /// are exactly that mode's servers — every one of them, once — with
    /// the hosts the client can read ahead of the rest and the site's
    /// order kept within each half.
    #[test]
    fn servers_for_a_mode_are_its_servers_readable_hosts_first(
        servers in proptest::collection::vec(
            (
                proptest::sample::select(vec!["sub", "dub"]),
                "[A-Za-z0-9-]{1,10}",
                proptest::sample::select(vec![
                    "https://zokoanime.video/stream/mal/1/1/sub",
                    "https://megaplay.buzz/stream/s-2/1/sub",
                    "https://vidtube.site/stream/abc/sub",
                ]),
            )
                .prop_map(|(mode, name, url)| ServerEmbed {
                    mode: mode.to_string(),
                    name,
                    embed_url: url.to_string(),
                }),
            0..8,
        ),
        mode in proptest::sample::select(vec!["sub", "dub"]),
    ) {
        let picked = servers_for(&servers, mode);
        let expected: Vec<&ServerEmbed> = servers.iter().filter(|s| s.mode == mode).collect();
        prop_assert_eq!(picked.len(), expected.len());
        for s in &expected {
            prop_assert!(picked.iter().any(|p| std::ptr::eq(*p, *s)));
        }
        let readable = |s: &ServerEmbed| s.embed_url.starts_with("https://zokoanime.video/");
        let first_unreadable = picked.iter().position(|s| !readable(s));
        let last_readable = picked.iter().rposition(|s| readable(s));
        if let (Some(u), Some(r)) = (first_unreadable, last_readable) {
            prop_assert!(r < u, "readable hosts lead: {picked:?}");
        }
        let readable_order: Vec<&ServerEmbed> = picked.iter().copied().filter(|s| readable(s)).collect();
        let readable_site_order: Vec<&ServerEmbed> = expected.iter().copied().filter(|s| readable(s)).collect();
        prop_assert!(readable_order.iter().zip(&readable_site_order).all(|(a, b)| std::ptr::eq(*a, *b)));
    }
}

// ── a page without the payload, by host ─────────────────────────────

proptest! {
    /// A page without the payload marker is a parse failure exactly
    /// when its host is one the client reads: such a host has changed
    /// shape under the client, while a host the client never read
    /// says nothing and is stepped over. The verdict names the host.
    #[test]
    fn a_missing_payload_is_a_parse_failure_exactly_on_a_readable_host(
        host in prop_oneof![
            Just("zokoanime.video".to_string()),
            "[a-z]{3,10}\\.(buzz|site|video|net)",
        ],
        path in "/stream/[a-z0-9/-]{1,20}",
    ) {
        let embed_url = format!("https://{host}{path}");
        let verdict = payload_missing_verdict(&embed_url);
        if ajax::readable(&embed_url) {
            match verdict {
                Some(AniError::ParseFailed { detail }) => prop_assert!(detail.contains(&host), "{detail}"),
                other => prop_assert!(false, "expected a parse failure, got {other:?}"),
            }
        } else {
            prop_assert!(verdict.is_none(), "{verdict:?}");
        }
    }
}
