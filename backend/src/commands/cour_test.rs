//! Tests for the cour-detection helpers used by mark-watched's
//! cross-cour integrity guard. See `commands/cour.rs` for the
//! production code.

use super::*;

#[test]
fn title_cour_finds_trailing_part_n() {
    assert_eq!(
        cour_from_title("JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2"),
        Some(2)
    );
    assert_eq!(
        cour_from_title("JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 3"),
        Some(3)
    );
}

#[test]
fn title_cour_returns_none_when_part_appears_only_mid_title() {
    // "Part 6" here describes the parent series, not the cour. Only a
    // trailing match counts as a cour disambiguator.
    assert_eq!(
        cour_from_title("JoJo no Kimyou na Bouken Part 6: Stone Ocean"),
        None
    );
}

#[test]
fn title_cour_accepts_cour_n_and_season_n() {
    assert_eq!(cour_from_title("Some Show Cour 2"), Some(2));
    assert_eq!(cour_from_title("Some Show Season 3"), Some(3));
}

#[test]
fn title_cour_is_case_insensitive() {
    assert_eq!(cour_from_title("Foo part 2"), Some(2));
    assert_eq!(cour_from_title("Foo PART 2"), Some(2));
    assert_eq!(cour_from_title("Foo SEASON 4"), Some(4));
}

#[test]
fn title_cour_returns_none_for_bare_titles() {
    assert_eq!(cour_from_title("Stone Ocean"), None);
    assert_eq!(cour_from_title("One Piece"), None);
    assert_eq!(cour_from_title(""), None);
}

/// `play_resolution_cache::put` stores `show_title` as
/// `"<name> (<N> episodes)"` (see `commands/play.rs`). The detector
/// must strip that trailing count before parsing the cour suffix, or
/// every production cache row returns None and the integrity guard
/// becomes a no-op.
#[test]
fn title_cour_strips_trailing_episode_count_suffix() {
    assert_eq!(
        cour_from_title("JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2 (12 episodes)"),
        Some(2)
    );
    assert_eq!(cour_from_title("Some Show Cour 3 (24 episodes)"), Some(3));
    // Bare title with a trailing count still resolves to None — the
    // count isn't a cour suffix on its own.
    assert_eq!(cour_from_title("Stone Ocean (12 episodes)"), None);
}

#[test]
fn slug_cour_finds_trailing_part_n() {
    assert_eq!(
        cour_from_slug("jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-2"),
        Some(2)
    );
    assert_eq!(
        cour_from_slug("jojo-no-kimyou-na-bouken-part-6-stone-ocean-part-3"),
        Some(3)
    );
}

#[test]
fn slug_cour_returns_none_when_part_is_mid_slug() {
    // Part 1's slug ends with "stone-ocean", no trailing "-part-N".
    assert_eq!(
        cour_from_slug("jojo-no-kimyou-na-bouken-part-6-stone-ocean"),
        None
    );
}

#[test]
fn slug_cour_accepts_cour_n_and_season_n() {
    assert_eq!(cour_from_slug("some-show-cour-2"), Some(2));
    assert_eq!(cour_from_slug("some-show-season-3"), Some(3));
}

#[test]
fn slug_cour_returns_none_for_bare_slugs() {
    assert_eq!(cour_from_slug("stone-ocean"), None);
    assert_eq!(cour_from_slug(""), None);
}

#[test]
fn mappings_agree_treats_both_none_as_cour_one() {
    // Bare provider name + bare kitsu slug → both default to cour 1 → agree.
    assert!(cours_agree(None, None));
}

#[test]
fn mappings_agree_treats_explicit_one_as_default() {
    assert!(cours_agree(Some(1), None));
    assert!(cours_agree(None, Some(1)));
    assert!(cours_agree(Some(1), Some(1)));
}

#[test]
fn mappings_agree_rejects_cross_cour_pairing() {
    // The user's actual poison: Part 2 the provider name (cour 2) paired
    // with Part 1 kitsu slug (no -part-N suffix, defaults to cour 1).
    assert!(!cours_agree(Some(2), None));
    assert!(!cours_agree(Some(2), Some(1)));
    assert!(!cours_agree(Some(3), Some(2)));
}

/// Edge cases below pin every short-circuit branch in the parsers so
/// that future refactors can't regress quietly. Each case exercises a
/// specific guard inside `cour_from_title`, `cour_from_slug`, or
/// `strip_trailing_episode_count`.

#[test]
fn title_cour_rejects_unanchored_keyword_prefix() {
    // "RePart 2" — keyword preceded by a letter, not start / whitespace
    // / colon. Must NOT match (otherwise mid-word coincidences would
    // claim a cour suffix that isn't really there).
    assert_eq!(cour_from_title("RePart 2"), None);
    assert_eq!(cour_from_title("xPart 2"), None);
}

#[test]
fn title_cour_handles_keyword_at_string_start() {
    // "Part 2" with no leading text — the `kw_start == 0` branch.
    assert_eq!(cour_from_title("Part 2"), Some(2));
    assert_eq!(cour_from_title("Season 5"), Some(5));
}

#[test]
fn title_cour_short_strings_dont_match_long_keywords() {
    // Trailing digit but the leading slice can't fit any cour keyword.
    // Forces the `kw_end < want` short-circuit inside find_map.
    assert_eq!(cour_from_title("X 2"), None);
    assert_eq!(cour_from_title("12 8"), None);
}

#[test]
fn slug_cour_rejects_digits_at_start_of_slug() {
    // Pure-digit slug — no preceding `-(part|cour|season)-` anchor.
    assert_eq!(cour_from_slug("2"), None);
    assert_eq!(cour_from_slug("12"), None);
}

#[test]
fn slug_cour_rejects_digits_after_non_dash_byte() {
    // Digits glued directly to the previous segment without a `-`
    // separator are not a cour segment.
    assert_eq!(cour_from_slug("partpart2"), None);
    assert_eq!(cour_from_slug("seasonx2"), None);
}

#[test]
fn slug_cour_rejects_unanchored_keyword_inside_segment() {
    // "x-bypart-2" — keyword exists but isn't preceded by start-of-
    // string or `-`, so the segment isn't a real cour suffix.
    assert_eq!(cour_from_slug("x-bypart-2"), None);
    assert_eq!(cour_from_slug("preseason-3"), None);
}

#[test]
fn slug_cour_short_slug_before_dash_cant_fit_keyword() {
    // Slug shape "<short>-<digits>" where the segment before the dash
    // is too short to be any keyword. Forces the `before_dash.len()
    // < want` short-circuit inside find_map.
    assert_eq!(cour_from_slug("xy-2"), None);
}

/// `cour_from_title` walked back from `kw_end = after_kw.len()` by
/// `kw.len()` bytes and then sliced `after_kw[kw_start..]` directly.
/// On non-ASCII titles ending in digits with no `Part/Cour/Season`
/// suffix that offset can land inside a multi-byte UTF-8 codepoint
/// (`"アニメ123"` → after_kw="アニメ", kw_start=5, byte 5 is inside `ニ`),
/// crashing `mark-watched` for any non-Latin title.
#[test]
fn title_cour_does_not_panic_on_non_ascii_trailing_digits() {
    // Each of these would panic at `byte index N is not a char
    // boundary` before the fix. The expected result is `None`
    // (no trailing cour suffix).
    assert_eq!(cour_from_title("アニメ123"), None);
    assert_eq!(cour_from_title("プレイ 2"), None);
    assert_eq!(cour_from_title("ピアノの森 12"), None);
}

#[test]
fn title_cour_accepts_keyword_after_non_ascii_prefix() {
    // Non-ASCII characters preceding a real cour keyword still
    // match the trailing suffix. The "preceded by whitespace /
    // colon" anchor check must walk back by char, not byte.
    assert_eq!(cour_from_title("アニメ Part 2"), Some(2));
    assert_eq!(cour_from_title("ピアノの森 Cour 3"), Some(3));
}

#[test]
fn title_cour_passes_through_trailing_text_without_episode_count() {
    // strip_trailing_episode_count short-circuits on ANY of:
    // - no trailing ")"
    // - ")" present but not preceded by " episodes"
    // - " episodes" but no opening "("
    // - "(<non-digits> episodes)" — non-digit content
    // Each case below trips one of those guards.
    assert_eq!(cour_from_title("Foo Part 2)"), None); // suffix is ")" only
    assert_eq!(cour_from_title("Foo Part 2 (oops)"), None); // not " episodes"
    assert_eq!(cour_from_title("Foo Part 2 abc episodes)"), None); // no "("
    assert_eq!(cour_from_title("Foo Part 2 (many episodes)"), None); // non-digits inside parens
}

// ── the ordinal forms Kitsu's titles and slugs use ──────────────────

/// Kitsu writes a cour before the keyword as often as after it —
/// "2nd Season", "Second Season" — and the page's own resolver reads
/// both. A qualified row spelled `season-2` meeting a Kitsu hit
/// spelled `2nd-season` read as cour 1 here and was passed over.
#[test]
fn title_cour_reads_a_numeric_ordinal_before_the_keyword() {
    assert_eq!(cour_from_title("Foo 2nd Season"), Some(2));
    assert_eq!(cour_from_title("Foo 3rd Part"), Some(3));
    assert_eq!(cour_from_title("Foo 4th Cour"), Some(4));
    assert_eq!(cour_from_title("Foo 1st season"), Some(1));
    assert_eq!(cour_from_title("Foo 11th Season"), Some(11));
    assert_eq!(cour_from_title("Foo: 2ND SEASON"), Some(2));
    assert_eq!(cour_from_title("Foo 2nd Season (12 episodes)"), Some(2));
}

#[test]
fn title_cour_reads_a_spelled_ordinal_before_the_keyword() {
    assert_eq!(cour_from_title("Foo Second Season"), Some(2));
    assert_eq!(cour_from_title("Foo Fourth Part"), Some(4));
    assert_eq!(cour_from_title("Foo tenth cour"), Some(10));
    assert_eq!(cour_from_title("Foo First Season"), Some(1));
    assert_eq!(cour_from_title("アニメ Second Season"), Some(2));
}

#[test]
fn title_cour_anchors_the_ordinal_like_the_keyword() {
    // The ordinal must begin a token: a letter glued to it is a word
    // that happens to end in an ordinal, not a cour.
    assert_eq!(cour_from_title("Foo x2nd Season"), None);
    assert_eq!(cour_from_title("Foo Resecond Season"), None);
    // A keyword with nothing before it names no cour.
    assert_eq!(cour_from_title("Foo Season"), None);
    assert_eq!(cour_from_title("Season"), None);
    // Only the page's ten spelled ordinals are read.
    assert_eq!(cour_from_title("Foo Eleventh Season"), None);
    // A bare number before the keyword is a title form the page does
    // not read either.
    assert_eq!(cour_from_title("Foo 2 Season"), None);
}

#[test]
fn slug_cour_reads_an_ordinal_segment_before_the_keyword() {
    assert_eq!(cour_from_slug("foo-2nd-season"), Some(2));
    assert_eq!(cour_from_slug("foo-3rd-part"), Some(3));
    assert_eq!(cour_from_slug("foo-4th-cour"), Some(4));
    assert_eq!(cour_from_slug("foo-11th-season"), Some(11));
    // Kitsu also writes the bare number before the keyword.
    assert_eq!(cour_from_slug("foo-2-season"), Some(2));
    assert_eq!(cour_from_slug("foo-second-season"), Some(2));
    assert_eq!(cour_from_slug("foo-fourth-part"), Some(4));
    assert_eq!(cour_from_slug("second-season"), Some(2));
    assert_eq!(cour_from_slug("2nd-season"), Some(2));
}

#[test]
fn slug_cour_keeps_the_ordinal_forms_trailing_and_anchored() {
    // Trailing only, as with `-season-N`: a cour segment followed by
    // more of the slug names the parent series.
    assert_eq!(cour_from_slug("foo-2nd-season-extra"), None);
    assert_eq!(cour_from_slug("foo-second-season-x"), None);
    // The ordinal segment must be a whole segment.
    assert_eq!(cour_from_slug("foosecond-season"), None);
    assert_eq!(cour_from_slug("x2nd-season"), None);
    assert_eq!(cour_from_slug("foo-eleventh-season"), None);
    assert_eq!(cour_from_slug("foo-season"), None);
}

mod cour_form_props {
    use super::super::{cour_from_slug, cour_from_title};
    use proptest::prelude::*;

    const WORDS: [&str; 10] = [
        "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth",
        "tenth",
    ];
    const KEYWORDS: [&str; 3] = ["part", "cour", "season"];
    const SUFFIXES: [&str; 4] = ["st", "nd", "rd", "th"];

    /// The title forms for cour `n`, over the keyword and the
    /// ordinal suffix or spelled word, each form's index for the
    /// failure message.
    fn title_forms(n: u32, kw: &str, suffix: &str) -> Vec<String> {
        let mut forms = vec![format!("{kw} {n}"), format!("{n}{suffix} {kw}")];
        if let Some(word) = WORDS.get((n as usize).wrapping_sub(1)) {
            forms.push(format!("{word} {kw}"));
        }
        forms
    }

    fn slug_forms(n: u32, kw: &str, suffix: &str) -> Vec<String> {
        let mut forms = vec![
            format!("{kw}-{n}"),
            format!("{n}{suffix}-{kw}"),
            format!("{n}-{kw}"),
        ];
        if let Some(word) = WORDS.get((n as usize).wrapping_sub(1)) {
            forms.push(format!("{word}-{kw}"));
        }
        forms
    }

    proptest! {
        /// Every form of cour `n` parses to `n`, on a title and on a
        /// slug alike, whatever precedes it; so a form for `n` never
        /// parses to another cour.
        #[test]
        fn every_form_of_a_cour_parses_to_it(
            words in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            n in 1u32..40,
            kw in prop::sample::select(KEYWORDS.to_vec()),
            suffix in prop::sample::select(SUFFIXES.to_vec()),
        ) {
            for form in title_forms(n, kw, suffix) {
                let title = format!("{words} {form}");
                prop_assert_eq!(cour_from_title(&title), Some(n), "title {}", title);
            }
            let base = words.replace(' ', "-");
            for form in slug_forms(n, kw, suffix) {
                let slug = format!("{base}-{form}");
                prop_assert_eq!(cour_from_slug(&slug), Some(n), "slug {}", slug);
            }
        }

        /// Words alone, or a keyword with nothing to number it, name
        /// no cour on either side.
        #[test]
        fn words_without_a_form_name_no_cour(
            words in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            kw in prop::sample::select(KEYWORDS.to_vec()),
        ) {
            prop_assert_eq!(cour_from_title(&words), None);
            prop_assert_eq!(cour_from_title(&format!("{words} {kw}")), None);
            let base = words.replace(' ', "-");
            prop_assert_eq!(cour_from_slug(&base), None);
            prop_assert_eq!(cour_from_slug(&format!("{base}-{kw}")), None);
        }
    }
}

mod hit_cour_props {
    use super::super::{cour_from_slug, cour_from_title, hit_cour_disagrees};
    use proptest::prelude::*;

    proptest! {
        /// A hit disagrees with a term exactly when both carry cour
        /// evidence and it differs: a term without a trailing cour,
        /// or a hit without a slug, disagrees with nothing, and a
        /// slug without a suffix is the parent cour.
        #[test]
        fn a_hit_disagrees_exactly_when_both_sides_speak_and_differ(
            words in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            term_cour in prop::option::of(1u32..5),
            slug in prop::option::of(("[a-z]{2,6}(-[a-z]{2,6}){0,2}", prop::option::of(1u32..5))),
        ) {
            let term = match term_cour {
                Some(n) => format!("{words} part {n}"),
                None => words.clone(),
            };
            let kitsu_slug: Option<String> = slug.as_ref().map(|(base, cour)| match cour {
                Some(n) => format!("{base}-part-{n}"),
                None => base.clone(),
            });
            let expected = match (cour_from_title(&term), kitsu_slug.as_deref()) {
                (Some(p), Some(s)) => p != cour_from_slug(s).unwrap_or(1),
                _ => false,
            };
            prop_assert_eq!(hit_cour_disagrees(&term, kitsu_slug.as_deref()), expected);
        }
    }
}
