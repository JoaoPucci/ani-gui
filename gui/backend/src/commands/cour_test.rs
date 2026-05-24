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
    assert_eq!(
        cour_from_title("Some Show Cour 3 (24 episodes)"),
        Some(3)
    );
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
    // Bare allmanga name + bare kitsu slug → both default to cour 1 → agree.
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
    // The user's actual poison: Part 2 allmanga name (cour 2) paired
    // with Part 1 kitsu slug (no -part-N suffix, defaults to cour 1).
    assert!(!cours_agree(Some(2), None));
    assert!(!cours_agree(Some(2), Some(1)));
    assert!(!cours_agree(Some(3), Some(2)));
}
