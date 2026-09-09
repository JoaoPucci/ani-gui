//! Property coverage for the pure hianime parsers — the site's
//! markup, envelopes and embed payload generated rather than
//! tabulated, so the failures that live in shapes a table does not
//! think to write get written.

use super::*;
use crate::error::AniError;
use crate::scraper::provider::{BrowseHit, EpisodeRef};
use base64::Engine as _;
use proptest::prelude::*;

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
            0..5,
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

    /// Every ep-item row comes back as its episode ref, in order.
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
            .map(|(number, id)| EpisodeRef { id: *id, number: *number, number2: None })
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

    /// Of two hosts' failures the kept one is the louder: a rate limit
    /// over everything, a page the client could not read over any
    /// other provider block, a block over the rest; between two of a
    /// rank the first stays.
    #[test]
    fn the_kept_weather_is_the_louder_of_the_two(
        first in arb_weather(),
        second in arb_weather(),
    ) {
        fn rank(w: &AniError) -> u8 {
            match w {
                AniError::RateLimited { .. } | AniError::Upstream { status: 429 } => 3,
                AniError::ParseFailed { .. } => 2,
                w if w.is_provider_block() => 1,
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
