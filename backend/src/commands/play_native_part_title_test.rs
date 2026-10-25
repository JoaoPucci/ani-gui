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

#[test]
fn the_part_reader_reads_every_marker_form_the_title_rules_read() {
    assert_eq!(part_ordinal("The Show", "The Show Season Two"), Some(2));
    assert_eq!(part_ordinal("The Show", "The Show VI"), Some(6));
    assert_eq!(part_ordinal("The Show", "The Show Sixth Season"), Some(6));
    assert_eq!(part_ordinal("The Show", "The Show Part ２"), Some(2));
}

#[test]
fn a_zeroth_part_is_no_later_part() {
    assert_eq!(part_ordinal("The Show", "The Show Season 0"), None);
    assert_eq!(part_ordinal("The Show", "The Show Part 0"), None);
}

#[test]
fn a_japanese_division_names_a_later_part_too() {
    // Kitsu can know a later entry by its Japanese title alone; the
    // predecessor guard reads the same grammar the title rules do.
    assert_eq!(part_ordinal("X", "X 第2期"), Some(2));
    assert_eq!(part_ordinal("X", "X第2期"), Some(2));
    assert_eq!(part_ordinal("X", "X 第２クール"), Some(2));
    assert_eq!(
        part_ordinal("X", "X 第一部"),
        None,
        "a first part is no later one"
    );
    assert!(precedes_entry("X", &["X 第2期"]));
}

proptest::proptest! {
    /// A title's digits are read in ASCII whatever width it writes them
    /// in, the sequel guard's comparison of a candidate with what an
    /// entry title adds to it included: "Lucky ２" is the season before
    /// "Lucky 2 Season 2" as "Lucky 2" is.
    #[test]
    fn the_sequel_guard_reads_full_width_digits_as_ascii(n in 0u32..100, k in 2u32..10) {
        let wide: String = n.to_string().chars().map(|c| char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap()).collect();
        let entry = format!("Lucky {n} Season {k}");
        let (full, ascii) = (format!("Lucky {wide}"), format!("Lucky {n}"));
        proptest::prop_assert!(precedes_entry(&full, &[entry.as_str()]), "{}", full);
        proptest::prop_assert!(precedes_entry(&ascii, &[entry.as_str()]), "{}", ascii);
    }
}

/// `s` with its ASCII digits written full-width.
fn full_width(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_digit() {
                char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

proptest::proptest! {
    /// Digits name the same part in either width, on either side: a
    /// stem and a title past it answer as their ASCII forms do,
    /// whichever of them, or both, write their digits full-width.
    #[test]
    fn a_part_is_read_alike_in_either_width_on_either_side(
        s in stem(),
        n in 0u32..100,
        k in 2u32..200,
        form in 0usize..4,
    ) {
        let stem_of = format!("{s} {n}");
        let rest = match form {
            0 => format!(" Season {k}"),
            1 => format!(" {} Stage", suffixed(k)),
            2 => format!(" Part {k}"),
            _ => format!(" {k}"),
        };
        let title = format!("{stem_of}{rest}");
        let want = part_ordinal(&stem_of, &title);
        proptest::prop_assert_eq!(part_ordinal(&stem_of, &full_width(&title)), want, "{}", title);
        proptest::prop_assert_eq!(part_ordinal(&full_width(&stem_of), &title), want, "{}", title);
        proptest::prop_assert_eq!(part_ordinal(&full_width(&stem_of), &full_width(&title)), want, "{}", title);
    }
}
