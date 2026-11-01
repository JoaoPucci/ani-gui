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
fn a_brazilian_user_gets_the_default_then_brazilian_then_european_portuguese() {
    assert_eq!(
        labels_first(&steel_ball_run(), "pt-BR", 4),
        [
            "English",
            "Portuguese (- Brazil)",
            "Portuguese (- Portugal)",
            "Arabic"
        ]
    );
}

#[test]
fn a_latin_american_spanish_user_gets_both_latin_american_tracks_before_spains() {
    assert_eq!(
        labels_first(&steel_ball_run(), "es-419", 5),
        [
            "English",
            "Spanish (- Latin America)",
            "Spanish (- Latin America | SDH)",
            "Spanish (- Spain)",
            "Arabic"
        ]
    );
}

#[test]
fn a_user_whose_locale_names_no_region_gets_every_track_in_the_language() {
    assert_eq!(
        labels_first(&steel_ball_run(), "ru", 3),
        ["English", "Russian", "Arabic"]
    );
    assert_eq!(
        labels_first(&steel_ball_run(), "pt", 4),
        [
            "English",
            "Portuguese (- Brazil)",
            "Portuguese (- Portugal)",
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
            "English",
            "Chinese (- Traditional)",
            "Chinese (- Simplified)"
        ]
    );
}

/// English comes right after the user's language when it is not the
/// default, and every other language follows in listing order.
#[test]
fn english_follows_the_users_language_when_it_is_not_the_default() {
    let tracks = vec![
        track("ar", "Arabic", false, 0),
        track("en", "English", false, 1),
        track("ja", "Japanese", true, 2),
        track("pt-BR", "Portuguese (- Brazil)", false, 3),
    ];
    assert_eq!(
        labels_first(&tracks, "pt-BR", 4),
        ["Japanese", "Portuguese (- Brazil)", "English", "Arabic"]
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

    /// The defaults lead, in listing order; then, whenever the user's
    /// locale names a known language, every track tagged with it comes
    /// before every non-default track tagged with another; and among
    /// tracks of one rank the listing's order stands.
    #[test]
    fn defaults_lead_and_the_users_language_precedes_the_rest(
        tracks in listing(),
        locale in locale(),
    ) {
        let order = preference_order(&tracks, Wanted::locale(&locale));
        let defaults = tracks.iter().filter(|t| t.default).count();
        prop_assert!(order[..defaults].iter().all(|&i| tracks[i].default));
        prop_assert!(order[..defaults].windows(2).all(|w| w[0] < w[1]));
        let user = primary(&locale);
        let in_users = |i: usize| ["en", "pt", "es", "ja", "zh", "ru"].contains(&user.as_str())
            && primary(&tracks[i].lang) == user;
        let rest = &order[defaults..];
        if let Some(last_users) = rest.iter().rposition(|&i| in_users(i)) {
            let first_other = rest.iter().position(|&i| !in_users(i)).unwrap_or(rest.len());
            prop_assert!(
                last_users < first_other,
                "a track in the user's language came after another: {:?}",
                rest
            );
        }
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
