//! Property coverage for the pure parse helpers.
//!
//! Its own file rather than an append to `anidb_test.rs`, matching
//! the other property modules on this branch: appended blocks collide
//! with every later addition to the same file.

use crate::scraper::anidb::parse::{
    encode_query, is_cloudflare_interstitial, parse_browse, parse_detail_year, parse_episodes,
    parse_languages, preferred_embed,
};
use crate::scraper::anidb::LanguageEmbed;

use super::slug_numeric_id;

proptest::proptest! {
    /// Form-decoding the encoded output recovers the input exactly,
    /// over arbitrary unicode. The naive space swap this function
    /// replaced broke on `;` and friends — a failure class a
    /// fixed-example table cannot sweep.
    #[test]
    fn form_decoding_recovers_the_original_query(q in ".*") {
        let encoded = encode_query(&q);
        let decoded: String = url::form_urlencoded::parse(format!("q={encoded}").as_bytes())
            .next()
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default();
        proptest::prop_assert_eq!(decoded, q);
    }

    /// The output stays inside the form-urlencoding alphabet, so it
    /// splices into a query string without further quoting.
    #[test]
    fn encoded_output_is_query_safe(q in ".*") {
        let encoded = encode_query(&q);
        proptest::prop_assert!(encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'%' | b'+' | b'-' | b'.' | b'_' | b'*')));
    }
}

proptest::proptest! {
    /// Over arbitrary embed lists and modes: the choice is exactly
    /// the FIRST entry carrying the mode's language — `eng` for dub,
    /// `jpn` for everything else — and None precisely when no entry
    /// carries it. Stated against an independent first-match oracle
    /// so an implementation that started preferring the last match,
    /// or panicking on odd language strings, fails here.
    #[test]
    fn embed_choice_is_the_first_entry_in_the_modes_language(
        langs in proptest::collection::vec(
            proptest::prop_oneof![
                proptest::strategy::Just("jpn".to_string()),
                proptest::strategy::Just("eng".to_string()),
                "[a-z]{0,4}",
            ],
            0..8,
        ),
        mode in proptest::prop_oneof![
            proptest::strategy::Just("sub".to_string()),
            proptest::strategy::Just("dub".to_string()),
            "[a-z]{0,4}",
        ],
    ) {
        let embeds: Vec<LanguageEmbed> = langs
            .iter()
            .enumerate()
            .map(|(i, l)| LanguageEmbed {
                language: l.clone(),
                embed_url: format!("https://embed.example/e/{i}"),
            })
            .collect();
        let want_lang = if mode == "dub" { "eng" } else { "jpn" };
        let expected = embeds.iter().find(|e| e.language == want_lang);
        let got = preferred_embed(&embeds, &mode);
        proptest::prop_assert_eq!(
            got.map(|e| e.embed_url.as_str()),
            expected.map(|e| e.embed_url.as_str())
        );
    }

    /// Any slug ending in a hyphen-separated decimal tail yields that
    /// number, whatever the prefix holds — hyphens and digits
    /// included; a non-numeric tail yields nothing.
    #[test]
    fn slug_id_is_exactly_the_decimal_tail(
        prefix in ".*",
        n in proptest::num::u64::ANY,
        word in "[a-zA-Z]{1,8}",
    ) {
        proptest::prop_assert_eq!(slug_numeric_id(&format!("{prefix}-{n}")), Some(n));
        proptest::prop_assert_eq!(slug_numeric_id(&format!("{prefix}-{word}")), None);
    }

    /// The premiere year is read from the season link's `year=`
    /// parameter wherever the link sits in the page, and only the
    /// digits that follow it.
    #[test]
    fn detail_year_is_the_season_links_year_parameter(
        pre in "[a-z </>\"=]*",
        post in "([a-z </>\"=].*)?",
        year in 1900u32..2100,
    ) {
        let html = format!("{pre}browse?season=fall&year={year}{post}");
        proptest::prop_assert_eq!(parse_detail_year(&html), Some(year));
    }
}

proptest::proptest! {
    /// Over arbitrary surroundings: the FIRST `file: '…'` value is
    /// extracted exactly — an empty value is a miss, not Some("") —
    /// and whatever follows, including further stanzas, changes
    /// nothing.
    #[test]
    fn master_url_is_the_first_file_stanzas_value(
        prefix in ".*",
        url in "[^']*",
        suffix in ".*",
    ) {
        proptest::prop_assume!(!prefix.contains("file: '"));
        let html = format!("{prefix}file: '{url}'{suffix}");
        let expected = if url.is_empty() { None } else { Some(url) };
        proptest::prop_assert_eq!(crate::scraper::anidb::parse::extract_master_url(&html), expected);
    }

    /// A page without the marker yields nothing, whatever else it
    /// holds.
    #[test]
    fn a_page_without_a_file_stanza_yields_nothing(page in ".*") {
        proptest::prop_assume!(!page.contains("file: '"));
        proptest::prop_assert_eq!(crate::scraper::anidb::parse::extract_master_url(&page), None);
    }
}

proptest::proptest! {
    /// The challenge marker is recognized under any capitalization,
    /// wherever it sits in the body.
    #[test]
    fn interstitial_detection_ignores_case(
        pre in ".*",
        post in ".*",
        flips in proptest::collection::vec(proptest::bool::ANY, 13),
    ) {
        let marker: String = "just a moment"
            .chars()
            .zip(&flips)
            .map(|(c, up)| if *up { c.to_ascii_uppercase() } else { c })
            .collect();
        let page = format!("{pre}{marker}{post}");
        proptest::prop_assert!(is_cloudflare_interstitial(&page));
    }

    /// A body whose lowercased text lacks the marker is content,
    /// never a challenge.
    #[test]
    fn a_body_without_the_marker_is_content(body in ".*") {
        proptest::prop_assume!(!body.to_ascii_lowercase().contains("just a moment"));
        proptest::prop_assert!(!is_cloudflare_interstitial(&body));
    }

    /// Rendered cards round-trip: every generated card comes back in
    /// order, its entity-encoded title decoded to the original, its
    /// badge as the kind — and junk between cards contributes
    /// nothing. This drives the entity decoder over arbitrary titles
    /// as a side effect.
    #[test]
    fn browse_cards_round_trip(
        cards in proptest::collection::vec(
            (
                "[a-z]{1,8}(-[a-z]{1,8}){0,2}",
                proptest::num::u32::ANY,
                "[A-Za-z0-9 '\"&:.!]{1,20}",
                proptest::option::of(proptest::prop_oneof![
                    proptest::strategy::Just("TV"),
                    proptest::strategy::Just("Movie"),
                    proptest::strategy::Just("OVA"),
                ]),
            ),
            0..5,
        ),
        junk in "[a-z <>/=]*",
    ) {
        proptest::prop_assume!(!junk.contains("<a href"));
        let mut html = String::new();
        for (word, id, title, kind) in &cards {
            let escaped = title
                .replace('&', "&amp;")
                .replace('\'', "&#039;")
                .replace('"', "&quot;");
            html.push_str(&format!(
                r#"<a href="https://anidb.app/anime/{word}-{id}"><img alt="{escaped}">"#
            ));
            if let Some(k) = kind {
                html.push_str(&format!(r#"<span class="badge badge-x">{k}</span>"#));
            }
            html.push_str("</a>");
            html.push_str(&junk);
        }
        let parsed = parse_browse(&html);
        // Zero cards renders a page of bare junk: under the
        // zero-hit contract that is absence only when the page
        // shows the browse shape. The junk alphabet can spell the
        // no-results copy (it has letters and spaces) but never
        // the grid attribute (no quote character).
        if cards.is_empty() && !html.to_ascii_lowercase().contains("no results") {
            proptest::prop_assert!(parsed.is_err());
            return Ok(());
        }
        let hits = parsed.expect("pages with cards or the no-results copy parse");
        proptest::prop_assert_eq!(hits.len(), cards.len());
        for (hit, (word, id, title, kind)) in hits.iter().zip(&cards) {
            proptest::prop_assert_eq!(&hit.slug, &format!("{word}-{id}"));
            proptest::prop_assert_eq!(&hit.title, title);
            proptest::prop_assert_eq!(hit.kind.as_deref(), *kind);
        }
    }

    /// A well-formed episodes envelope round-trips in order; extra
    /// provider fields ride along unread.
    #[test]
    fn episode_rows_round_trip(
        rows in proptest::collection::vec(
            (proptest::num::u64::ANY, proptest::num::u32::ANY),
            0..8,
        ),
    ) {
        let body = serde_json::json!({
            "episodes": rows
                .iter()
                .map(|(id, number)| {
                    serde_json::json!({"id": id, "number": number, "number2": null, "filler": false})
                })
                .collect::<Vec<_>>()
        })
        .to_string();
        let eps = parse_episodes(&body);
        proptest::prop_assert!(eps.is_ok());
        let eps = eps.unwrap();
        proptest::prop_assert_eq!(eps.len(), rows.len());
        for (ep, (id, number)) in eps.iter().zip(&rows) {
            proptest::prop_assert_eq!(ep.id, *id);
            proptest::prop_assert_eq!(ep.number, *number);
        }
    }

    /// Anything that is not the episodes envelope is a typed parse
    /// failure, never an empty success.
    #[test]
    fn a_non_envelope_body_is_a_parse_failure(body in ".*") {
        proptest::prop_assume!(
            serde_json::from_str::<serde_json::Value>(&body)
                .map(|v| v.get("episodes").is_none())
                .unwrap_or(true)
        );
        proptest::prop_assert!(parse_episodes(&body).is_err());
    }

    /// A well-formed languages envelope round-trips codes and embed
    /// urls in order; the display name rides along unread.
    #[test]
    fn language_rows_round_trip(
        rows in proptest::collection::vec(
            ("[a-z]{2,4}", "https://[a-z]{1,8}\\.example/e/[a-z0-9-]{1,10}"),
            0..6,
        ),
    ) {
        let body = serde_json::json!({
            "languages": rows
                .iter()
                .map(|(code, url)| serde_json::json!({"code": code, "name": "x", "embed_url": url}))
                .collect::<Vec<_>>()
        })
        .to_string();
        let embeds = parse_languages(&body);
        proptest::prop_assert!(embeds.is_ok());
        let embeds = embeds.unwrap();
        proptest::prop_assert_eq!(embeds.len(), rows.len());
        for (e, (code, url)) in embeds.iter().zip(&rows) {
            proptest::prop_assert_eq!(&e.language, code);
            proptest::prop_assert_eq!(&e.embed_url, url);
        }
    }
}
