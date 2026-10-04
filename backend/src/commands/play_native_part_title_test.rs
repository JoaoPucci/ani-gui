//! Tests for `crate::commands::play_native_part_title`.

use super::*;

const SBR: &str = "Steel Ball Run: JoJo no Kimyou na Bouken";

#[test]
fn the_stem_is_the_first_part() {
    assert_eq!(part_ordinal(SBR, SBR), Some(1));
    assert_eq!(part_ordinal(SBR, &SBR.to_uppercase()), Some(1));
}

#[test]
fn a_short_ordinal_suffix_names_a_later_part() {
    assert_eq!(part_ordinal(SBR, &format!("{SBR} 2nd Stage")), Some(2));
    assert_eq!(part_ordinal("The Show", "The Show Part 3"), Some(3));
    assert_eq!(part_ordinal("The Show", "The Show: Season 2"), Some(2));
    assert_eq!(part_ordinal("The Show", "The Show - Second Cour"), Some(2));
    assert_eq!(part_ordinal("The Show", "The Show II"), Some(2));
}

#[test]
fn anything_else_is_not_a_part() {
    assert_eq!(part_ordinal("The Show", "The Show Movie"), None);
    assert_eq!(part_ordinal("The Show", "The Show - The Calamity"), None);
    assert_eq!(part_ordinal("The Show", "Another Show 2nd Season"), None);
    assert_eq!(part_ordinal("The Show", "The Showdown 2"), None);
    // A long tail is a different show that happens to start the same.
    assert_eq!(
        part_ordinal("The Show", "The Show 2nd Season Recap Special Edition"),
        None
    );
}
