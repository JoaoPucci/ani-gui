//! The order the app keeps a listing's subtitle tracks in wherever it
//! can keep only some: the provider's default, the user's language in
//! the user's region, the rest of the user's language, English, every
//! other language, then tracks whose language is unknown — ties in
//! the listing's order.

use super::{most_wanted_within, preference_order, preferred_first, Wanted};
use crate::scraper::provider::SubtitleTrack;
use proptest::prelude::*;

fn track(lang: &str, label: &str, default: bool, at: usize) -> SubtitleTrack {
    SubtitleTrack {
        lang: lang.into(),
        label: label.into(),
        default,
        url: format!("https://cdn.example/subs/{at}.vtt"),
    }
}

/// Steel Ball Run's first episode as the reader tags it: 33 tracks in
/// the provider's alphabetical order, English the default.
fn steel_ball_run() -> Vec<SubtitleTrack> {
    [
        ("ar", "Arabic"),
        ("zh-Hans", "Chinese (- Simplified)"),
        ("zh-Hant", "Chinese (- Traditional)"),
        ("hr", "Croatian"),
        ("cs", "Czech"),
        ("da", "Danish"),
        ("nl", "Dutch"),
        ("en", "English"),
        ("fi", "Finnish"),
        ("fr", "French"),
        ("de", "German"),
        ("el", "Greek"),
        ("he", "Hebrew"),
        ("hu", "Hungarian"),
        ("id", "Indonesian"),
        ("it", "Italian"),
        ("ja", "Japanese"),
        ("ko", "Korean"),
        ("ms", "Malay"),
        ("nb", "Norwegian (Bokmål)"),
        ("pl", "Polish"),
        ("pt-BR", "Portuguese (- Brazil)"),
        ("pt-PT", "Portuguese (- Portugal)"),
        ("ro", "Romanian"),
        ("ru", "Russian"),
        ("es-419", "Spanish (- Latin America)"),
        ("es-419", "Spanish (- Latin America | SDH)"),
        ("es-ES", "Spanish (- Spain)"),
        ("sv", "Swedish"),
        ("th", "Thai"),
        ("th", "Thai (- SDH)"),
        ("tr", "Turkish"),
        ("uk", "Ukrainian"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (lang, label))| track(lang, label, *lang == "en", i))
    .collect()
}

fn labels_first(tracks: &[SubtitleTrack], locale: &str, n: usize) -> Vec<String> {
    preferred_first(tracks, Wanted::locale(locale))
        .into_iter()
        .take(n)
        .map(|t| t.label.clone())
        .collect()
}

#[test]
fn a_brazilian_user_gets_brazilian_then_european_portuguese_then_the_default() {
    assert_eq!(
        labels_first(&steel_ball_run(), "pt-BR", 4),
        [
            "Portuguese (- Brazil)",
            "Portuguese (- Portugal)",
            "English",
            "Arabic"
        ]
    );
}

#[test]
fn a_latin_american_spanish_user_gets_both_latin_american_tracks_before_spains() {
    assert_eq!(
        labels_first(&steel_ball_run(), "es-419", 5),
        [
            "Spanish (- Latin America)",
            "Spanish (- Latin America | SDH)",
            "Spanish (- Spain)",
            "English",
            "Arabic"
        ]
    );
}

#[test]
fn a_user_whose_locale_names_no_region_gets_every_track_in_the_language() {
    assert_eq!(
        labels_first(&steel_ball_run(), "ru", 3),
        ["Russian", "English", "Arabic"]
    );
    assert_eq!(
        labels_first(&steel_ball_run(), "pt", 4),
        [
            "Portuguese (- Brazil)",
            "Portuguese (- Portugal)",
            "English",
            "Arabic"
        ]
    );
}

/// The listing names Chinese tracks by script; a region's user is
/// taken to want the script it writes.
#[test]
fn a_taiwanese_user_gets_traditional_chinese_first() {
    assert_eq!(
        labels_first(&steel_ball_run(), "zh-TW", 3),
        [
            "Chinese (- Traditional)",
            "Chinese (- Simplified)",
            "English"
        ]
    );
}

/// The user's language leads, then the provider's default, then
/// English, and every other language follows in listing order — the
/// order the built-in player starts by, so an external player that
/// takes one track takes the same one.
#[test]
fn the_users_language_leads_then_the_default_then_english() {
    let tracks = vec![
        track("ar", "Arabic", false, 0),
        track("en", "English", false, 1),
        track("ja", "Japanese", true, 2),
        track("pt-BR", "Portuguese (- Brazil)", false, 3),
    ];
    assert_eq!(
        labels_first(&tracks, "pt-BR", 4),
        ["Portuguese (- Brazil)", "Japanese", "English", "Arabic"]
    );
}

/// The language last picked on the show leads the user's locale: its
/// own region first, then the language's other regions, then the
/// locale's language, then the default. A remembered language the
/// episode lacks leaves the locale's order as it was.
#[test]
fn the_shows_remembered_language_leads_the_locale() {
    let wanted = |remembered| Wanted {
        remembered: Some(remembered),
        locale: "pt-BR",
    };
    let first = |remembered, n| -> Vec<String> {
        preferred_first(&steel_ball_run(), wanted(remembered))
            .into_iter()
            .take(n)
            .map(|t| t.label.clone())
            .collect()
    };
    assert_eq!(
        first("es-419", 5),
        [
            "Spanish (- Latin America)",
            "Spanish (- Latin America | SDH)",
            "Spanish (- Spain)",
            "Portuguese (- Brazil)",
            "Portuguese (- Portugal)"
        ]
    );
    assert_eq!(
        first("zh-Hant", 3),
        [
            "Chinese (- Traditional)",
            "Chinese (- Simplified)",
            "Portuguese (- Brazil)"
        ]
    );
    assert_eq!(
        first("fil", 2),
        ["Portuguese (- Brazil)", "Portuguese (- Portugal)"]
    );
}

/// A tag that names neither region nor script is read in its
/// language's usual script, as the built-in player reads it: a bare
/// `zh` is Simplified, so a Simplified pick takes it over `zh-TW`, and
/// a bare `zh` pick takes `zh-Hans` over `zh-Hant`.
/// Serbian reads as Cyrillic but where Montenegro, Romania and Turkey
/// write it in Latin, and a former code reads as its language — by the
/// same table and canon the built-in player uses.
#[test]
fn serbian_by_region_and_former_codes_read_as_the_built_in_player_reads_them() {
    let tracks = vec![
        track("sr-Cyrl", "Serbian (Cyrillic)", false, 0),
        track("sr-Latn", "Serbian (Latin)", false, 1),
    ];
    assert_eq!(labels_first(&tracks, "sr-ME", 1), ["Serbian (Latin)"]);
    assert_eq!(labels_first(&tracks, "sr", 1), ["Serbian (Cyrillic)"]);
    let hebrew = vec![
        track("en", "English", true, 0),
        track("iw", "Hebrew", false, 1),
    ];
    assert_eq!(labels_first(&hebrew, "he", 1), ["Hebrew"]);
}

#[test]
fn a_bare_tag_reads_in_its_languages_usual_script() {
    let first = |tracks: &[SubtitleTrack], remembered| -> String {
        preferred_first(
            tracks,
            Wanted {
                remembered: Some(remembered),
                locale: "",
            },
        )[0]
        .label
        .clone()
    };
    let taiwan_then_bare = vec![
        track("zh-TW", "Chinese (Taiwan)", false, 0),
        track("zh", "Chinese", false, 1),
    ];
    assert_eq!(first(&taiwan_then_bare, "zh-Hans"), "Chinese");
    let scripts = vec![
        track("zh-Hant", "Chinese (Traditional)", false, 0),
        track("zh-Hans", "Chinese (Simplified)", false, 1),
    ];
    assert_eq!(first(&scripts, "zh"), "Chinese (Simplified)");
}

/// Within a language, the exact region comes before a track that only
/// shares the script, and a track's script is read from its region —
/// as the built-in player starts.
#[test]
fn within_a_language_the_region_then_the_script() {
    let tracks = vec![
        track("zh-CN", "Chinese (PRC)", false, 0),
        track("zh-Hant", "Chinese (Traditional)", false, 1),
        track("zh-HK", "Chinese (Hong Kong)", false, 2),
    ];
    assert_eq!(
        labels_first(&tracks, "zh-HK", 3),
        [
            "Chinese (Hong Kong)",
            "Chinese (Traditional)",
            "Chinese (PRC)"
        ]
    );
    assert_eq!(
        labels_first(&tracks, "zh-TW", 3),
        [
            "Chinese (Traditional)",
            "Chinese (Hong Kong)",
            "Chinese (PRC)"
        ]
    );
}

/// A track whose language the reader could not read comes after
/// every track whose language it could.
#[test]
fn a_track_of_unknown_language_comes_last() {
    let tracks = vec![
        track("und", "Klingon", false, 0),
        track("ko", "Korean", false, 1),
        track("en", "English", false, 2),
    ];
    assert_eq!(
        labels_first(&tracks, "en", 3),
        ["English", "Korean", "Klingon"]
    );
}

/// A provider whose `lang` is a name rather than a tag is read by that
/// name or its label.
#[test]
fn a_track_whose_lang_is_a_name_is_read_by_it() {
    let tracks = vec![
        track("Arabic", "Arabic", false, 0),
        track("Portuguese", "Portuguese (- Brazil)", false, 1),
        track("English", "English", false, 2),
    ];
    assert_eq!(
        labels_first(&tracks, "pt-BR", 3),
        ["Portuguese (- Brazil)", "English", "Arabic"]
    );
}

/// A provider whose `lang` is a three-letter code is read by the code.
#[test]
fn a_track_whose_lang_is_a_three_letter_code_is_read_by_it() {
    let tracks = vec![
        track("ara", "Arabic", false, 0),
        track("eng", "English", false, 1),
        track("por", "Portuguese", false, 2),
    ];
    assert_eq!(
        labels_first(&tracks, "pt-BR", 3),
        ["Portuguese", "English", "Arabic"]
    );
}

/// Past the cap the most wanted are kept: cut to four, a Brazilian
/// user's Portuguese tracks survive where listing order would have
/// kept Arabic, both Chinese tracks and Croatian; and what is kept
/// stays in the listing's order.
#[test]
fn a_cut_keeps_the_most_wanted_in_listing_order() {
    let (kept, dropped) = most_wanted_within(steel_ball_run(), 4, Wanted::locale("pt-BR"));
    let labels: Vec<&str> = kept.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Arabic",
            "English",
            "Portuguese (- Brazil)",
            "Portuguese (- Portugal)"
        ]
    );
    assert_eq!(dropped, 29);
}

/// Tracks of arbitrary tags, labels and default flags, URLs distinct by
/// position.
fn listing() -> impl Strategy<Value = Vec<SubtitleTrack>> {
    let lang = prop_oneof![
        Just("en".to_string()),
        Just("pt-BR".to_string()),
        Just("pt-PT".to_string()),
        Just("pt".to_string()),
        Just("es-419".to_string()),
        Just("es-ES".to_string()),
        Just("ja".to_string()),
        Just("zh-Hant".to_string()),
        Just("und".to_string()),
        // Free text that names no language: a digit first, so it is
        // neither a code nor the start of a language's name.
        "[0-9][A-Za-z -]{0,11}",
    ];
    proptest::collection::vec((lang, any::<bool>()), 0..40).prop_map(|rows| {
        rows.into_iter()
            .enumerate()
            .map(|(i, (lang, default))| track(&lang, &lang, default, i))
            .collect()
    })
}

fn locale() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("en".to_string()),
        Just("pt-BR".to_string()),
        Just("es-419".to_string()),
        Just("ru".to_string()),
        Just("zh-TW".to_string()),
        "[A-Za-z_-]{0,10}",
    ]
}

/// The primary subtag of a tag, lowercased.
fn primary(tag: &str) -> String {
    tag.trim()
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

proptest! {
    /// The order is a permutation of the listing: nothing dropped,
    /// nothing twice.
    #[test]
    fn the_order_is_a_permutation(tracks in listing(), locale in locale()) {
        let mut order = preference_order(&tracks, Wanted::locale(&locale));
        order.sort_unstable();
        prop_assert_eq!(order, (0..tracks.len()).collect::<Vec<_>>());
    }

    /// The show's remembered language leads; then the user's locale's
    /// language; then the provider's defaults; and within each the
    /// listing's order stands among tracks of one kind.
    #[test]
    fn the_remembered_language_then_the_locales_then_the_defaults_lead(
        tracks in listing(),
        locale in locale(),
        remembered in proptest::option::of(prop_oneof![
            Just("es-419".to_string()),
            Just("pt-BR".to_string()),
            Just("ja".to_string()),
            Just("zh-Hant".to_string()),
        ]),
    ) {
        let wanted = Wanted { remembered: remembered.as_deref(), locale: &locale };
        let order = preference_order(&tracks, wanted);
        let known = ["en", "pt", "es", "ja", "zh", "ru"];
        let speaks = |want: &str, i: usize| {
            let want = primary(want);
            known.contains(&want.as_str()) && primary(&tracks[i].lang) == want
        };
        let tier = |i: usize| -> u8 {
            if remembered.as_deref().is_some_and(|r| speaks(r, i)) {
                0
            } else if speaks(&locale, i) {
                1
            } else if tracks[i].default {
                2
            } else {
                3
            }
        };
        let tiers: Vec<u8> = order.iter().map(|&i| tier(i)).collect();
        prop_assert!(tiers.windows(2).all(|w| w[0] <= w[1]), "{:?}", tiers);
        let defaults: Vec<usize> = order.iter().copied().filter(|&i| tier(i) == 2).collect();
        prop_assert!(defaults.windows(2).all(|w| w[0] < w[1]));
    }

    /// A cut keeps exactly `cap` tracks (or all of them), in the
    /// listing's order, says how many it left, and keeps the first
    /// `cap` of the preference order.
    #[test]
    fn a_cut_keeps_the_head_of_the_order_in_listing_order(
        tracks in listing(),
        locale in locale(),
        cap in 0usize..45,
    ) {
        let order = preference_order(&tracks, Wanted::locale(&locale));
        let (kept, dropped) = most_wanted_within(tracks.clone(), cap, Wanted::locale(&locale));
        prop_assert_eq!(kept.len(), tracks.len().min(cap));
        prop_assert_eq!(kept.len() + dropped, tracks.len());
        let mut wanted: Vec<usize> = order.into_iter().take(cap).collect();
        wanted.sort_unstable();
        let got: Vec<&str> = kept.iter().map(|t| t.url.as_str()).collect();
        let expected: Vec<&str> = wanted.iter().map(|&i| tracks[i].url.as_str()).collect();
        prop_assert_eq!(got, expected);
    }
}
