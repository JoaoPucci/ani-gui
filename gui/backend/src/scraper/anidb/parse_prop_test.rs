//! Property coverage for the pure parse helpers.
//!
//! Its own file rather than an append to `anidb_test.rs`, matching
//! the other property modules on this branch: appended blocks collide
//! with every later addition to the same file.

use crate::scraper::anidb::parse::{encode_query, parse_detail_year, preferred_embed};
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
