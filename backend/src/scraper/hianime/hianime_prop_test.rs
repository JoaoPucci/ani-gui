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

fn search_page(cards: &[(String, u64, String, String)]) -> String {
    let mut page = String::from(
        r#"<html><body><section class="block_area block_area_sidebar"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/decoy-1" title="Decoy">Decoy</a></h3></div></section><div class="film_list-wrap">"#,
    );
    for (words, id, title, kind) in cards {
        page.push_str(&format!(
            r#"<div class="flw-item"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/{words}-{id}" title="{}" class="dynamic-name">x</a></h3><div class="fd-infor"><span class="fdi-item">{kind}</span><span class="dot"></span><span class="fdi-item fdi-duration">24m</span></div></div></div>"#,
            encode_title(title)
        ));
    }
    page.push_str(r#"</div><div id="main-sidebar"><div class="film-detail"><h3 class="film-name"><a href="https://hianime.at/decoy-2" title="Decoy">Decoy</a></h3></div></div></body></html>"#);
    page
}

/// A failure a host can hand the server loop: an upstream status of
/// any shape, a rate limit, or a dropped connection.
fn arb_weather() -> impl Strategy<Value = AniError> {
    prop_oneof![
        (100u16..600).prop_map(|status| AniError::Upstream { status }),
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
    /// order, title decoded, badge kept — and the decoy cards outside
    /// the list never do, however many results there are.
    #[test]
    fn search_cards_round_trip(
        cards in proptest::collection::vec(
            (
                "[a-z0-9]{1,6}(-[a-z0-9]{1,6}){0,3}",
                1u64..1_000_000,
                "[A-Za-z0-9 ,:!&'\"<>-]{1,30}",
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

    /// Of two hosts' failures the kept one is a provider block exactly
    /// when either was; between two of a rank the first stays.
    #[test]
    fn the_kept_weather_is_a_block_whenever_either_was(
        first in arb_weather(),
        second in arb_weather(),
    ) {
        let first_block = first.is_provider_block();
        let second_block = second.is_provider_block();
        let first_repr = format!("{first:?}");
        let second_repr = format!("{second:?}");
        let kept = weightier(first, second);
        prop_assert_eq!(kept.is_provider_block(), first_block || second_block);
        let kept_repr = format!("{kept:?}");
        if first_block == second_block {
            prop_assert_eq!(kept_repr, first_repr);
        } else if second_block {
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
