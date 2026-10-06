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

#[test]
fn real_provider_sequel_titles_name_their_part() {
    assert_eq!(
        part_ordinal("Spy x Family", "Spy x Family, Part 2"),
        Some(2)
    );
    assert_eq!(
        part_ordinal("Vinland Saga", "Vinland Saga: 2nd Season"),
        Some(2)
    );
    assert_eq!(
        part_ordinal("Mob Psycho 100", "Mob Psycho 100 III"),
        Some(3)
    );
    assert_eq!(
        part_ordinal(
            "Mushoku Tensei: Jobless Reincarnation Season 2",
            "Mushoku Tensei: Jobless Reincarnation Season 2 Part 2"
        ),
        Some(2)
    );
}

#[test]
fn real_franchise_titles_that_are_not_parts_of_each_other() {
    // Gintama's entries differ only by punctuation; none of them is a
    // part of another, and none is the bare title either.
    for t in ["Gintama'", "Gintama°", "Gintama.", "Gintama: Enchousen"] {
        assert_eq!(part_ordinal("Gintama", t), None, "{t}");
    }
    assert_eq!(part_ordinal("Gintama.", "Gintama.: Slip Arc"), None);
    assert_eq!(
        part_ordinal(
            "Gintama.: Silver Soul Arc",
            "Gintama.: Silver Soul Arc - Second Half War"
        ),
        None
    );
    assert_eq!(part_ordinal("Naruto", "Naruto: Shippuden"), None);
    assert_eq!(part_ordinal("Naruto", "Naruto: Shippuden the Movie"), None);
    assert_eq!(part_ordinal("Overlord II", "Overlord III"), None);
    assert_eq!(
        part_ordinal(
            "Attack on Titan: Final Season, Part 1",
            "Attack on Titan: Final Season, Part 2"
        ),
        None
    );
    assert_eq!(
        part_ordinal(
            "Haikyuu!!: To the Top (Part 1+2)",
            "Haikyuu!!: To the Top 2nd Season"
        ),
        None
    );
}

/// A stem the marker forms can follow: ASCII words, ending in a letter
/// so the marker is a separate word.
fn stem() -> impl proptest::strategy::Strategy<Value = String> {
    "[A-Za-z][A-Za-z: ]{0,24}[A-Za-z]"
}

/// The ordinal suffix English writes after `n`.
fn suffixed(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// Random letter case and runs of whitespace — neither may change the
/// answer.
fn scrambled(s: &str, upper: &[bool], spaces: usize) -> String {
    s.chars()
        .zip(upper.iter().cycle())
        .map(|(c, up)| {
            if c == ' ' {
                " ".repeat(spaces)
            } else if *up {
                c.to_ascii_uppercase().to_string()
            } else {
                c.to_ascii_lowercase().to_string()
            }
        })
        .collect()
}

proptest::proptest! {
    #[test]
    fn a_title_is_the_first_part_of_itself_whatever_its_case_and_spacing(
        s in stem(),
        upper in proptest::collection::vec(proptest::bool::ANY, 1..8),
        spaces in 1usize..4,
    ) {
        proptest::prop_assert_eq!(part_ordinal(&s, &scrambled(&s, &upper, spaces)), Some(1));
    }

    #[test]
    fn every_recognised_marker_form_names_its_ordinal(
        s in stem(),
        n in 2u32..200,
        form in 0usize..6,
        upper in proptest::collection::vec(proptest::bool::ANY, 1..8),
        spaces in 1usize..4,
    ) {
        let marker = match form {
            0 => format!("{} Stage", suffixed(n)),
            1 => format!("Part {n}"),
            2 => format!(": Season {n}"),
            3 => format!(" - {} Cour", suffixed(n)),
            4 => format!("Stage {n}"),
            _ => format!("{} Season", suffixed(n)),
        };
        let title = scrambled(&format!("{s} {marker}"), &upper, spaces);
        proptest::prop_assert_eq!(part_ordinal(&s, &title), Some(n));
    }

    #[test]
    fn a_title_that_does_not_start_with_the_stem_is_no_part_of_it(
        s in stem(),
        other in "[A-Za-z ]{0,30}",
        n in 2u32..50,
    ) {
        let title = format!("{other} Part {n}");
        let starts = title.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
            .starts_with(&s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" "));
        proptest::prop_assume!(!starts);
        proptest::prop_assert_eq!(part_ordinal(&s, &title), None);
    }

    #[test]
    fn any_text_at_all_reads_as_no_part_or_a_positive_one(
        s in "\\PC{0,30}",
        t in "\\PC{0,40}",
    ) {
        // Arbitrary Unicode: never a panic, and never a part zero.
        match part_ordinal(&s, &t) {
            None => {}
            Some(k) => proptest::prop_assert!(k >= 1),
        }
    }

    #[test]
    fn a_later_part_is_never_the_first(s in stem(), t in "\\PC{0,40}") {
        // Some(1) means the two titles are the same title.
        if part_ordinal(&s, &t) == Some(1) {
            proptest::prop_assert_eq!(normalized(&s), normalized(&t));
        }
    }
}

#[test]
fn a_title_the_entry_extends_with_a_later_part_precedes_it() {
    assert!(precedes_entry("X", &["X Season 2"]));
    assert!(precedes_entry(
        "Golden Kamuy",
        &["Ekkusu", "Golden Kamuy: Season 2"]
    ));
    // The entry itself, its own later parts and strangers do not.
    assert!(!precedes_entry("X Season 2", &["X Season 2"]));
    assert!(!precedes_entry("X Season 2", &["X"]));
    assert!(!precedes_entry("Y", &["X Season 2"]));
    assert!(!precedes_entry(SBR, &[SBR]));
    assert!(!precedes_entry(&format!("{SBR} 2nd Stage"), &[SBR]));
    assert!(!precedes_entry("X", &[]));
}

proptest::proptest! {
    #[test]
    fn a_title_precedes_the_entry_exactly_when_an_entry_title_names_its_later_part(
        stem in "[A-Za-z][A-Za-z ]{0,12}",
        parts in proptest::collection::vec(proptest::option::of(1u32..6), 0..4),
    ) {
        let titles: Vec<String> = parts
            .iter()
            .map(|p| match p {
                Some(1) => stem.clone(),
                Some(k) => format!("{stem} Part {k}"),
                None => "Another Show".to_string(),
            })
            .collect();
        let refs: Vec<&str> = titles.iter().map(String::as_str).collect();
        let later = parts.iter().any(|p| p.is_some_and(|k| k >= 2));
        proptest::prop_assert_eq!(precedes_entry(&stem, &refs), later);
    }
}
