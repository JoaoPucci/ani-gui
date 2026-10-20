//! Property coverage for the entry before: a title's name followed by
//! first divisions alone, and no entry before a title with no name.

use super::*;
use proptest::prelude::*;

const NAMES: [&str; 3] = ["Show", "Other Show", "Kaiju Story"];
const DIVISIONS: [&str; 6] = [
    "Season 2",
    "2nd Season",
    "Part 2",
    "Season 3 Part 2",
    "第2期",
    "Season 1",
];
const MARKS: [&str; 6] = ["", ": ", "- ", "・", "： ", "~ "];

/// A title that is nothing but a division, after any marks that open
/// it, or blank.
fn nameless() -> impl Strategy<Value = String> {
    prop_oneof![
        (
            proptest::sample::select(MARKS.to_vec()),
            proptest::sample::select(DIVISIONS.to_vec()),
        )
            .prop_map(|(m, d)| format!("{m}{d}")),
        "[ \t\n]{0,4}",
    ]
}

/// A candidate: a name, a name and a first division, a nameless title,
/// or any text at all.
fn candidate() -> impl Strategy<Value = String> {
    prop_oneof![
        proptest::sample::select(NAMES.to_vec()).prop_map(str::to_string),
        proptest::sample::select(NAMES.to_vec()).prop_map(|n| format!("{n} Season 1")),
        nameless(),
        any::<String>(),
    ]
}

/// Any title: a name and a division, a nameless one, or any text.
fn title() -> impl Strategy<Value = String> {
    prop_oneof![
        (
            proptest::sample::select(NAMES.to_vec()),
            proptest::sample::select(DIVISIONS.to_vec()),
        )
            .prop_map(|(n, d)| format!("{n} {d}")),
        nameless(),
        any::<String>(),
    ]
}

proptest! {
    /// A title that is nothing but a division, or blank, has an empty
    /// name, and nothing is the entry before it.
    #[test]
    fn nothing_is_the_entry_before_a_title_with_no_name(
        candidate in candidate(),
        title in nameless(),
    ) {
        prop_assert!(name(&untagged_normalized(&title)).is_empty(), "{title:?}");
        prop_assert!(!is_name_and_first_divisions(&candidate, &title));
    }

    /// Whatever is the entry before a title, the title has a name.
    #[test]
    fn an_entry_before_is_read_only_past_a_name(
        candidate in candidate(),
        title in title(),
    ) {
        if is_name_and_first_divisions(&candidate, &title) {
            prop_assert!(!name(&untagged_normalized(&title)).is_empty());
        }
    }

    /// A title's name, alone or with a first season, is the entry
    /// before the title's later division.
    #[test]
    fn a_name_is_the_entry_before_its_later_division(
        name in proptest::sample::select(NAMES.to_vec()),
        division in proptest::sample::select(DIVISIONS[..5].to_vec()),
        first in proptest::sample::select(vec!["", " Season 1"]),
    ) {
        let title = format!("{name} {division}");
        let candidate = format!("{name}{first}");
        prop_assert!(is_name_and_first_divisions(&candidate, &title));
    }

    /// A full-width closing bracket around the name separates it from
    /// what follows as the ASCII one does: "［Show］ 2nd Season" has
    /// the entries before it that "[Show] 2nd Season" has.
    #[test]
    fn a_full_width_bracket_around_the_name_reads_as_the_ascii_one(
        candidate in candidate(),
        name in proptest::sample::select(NAMES.to_vec()),
        division in proptest::sample::select(DIVISIONS.to_vec()),
    ) {
        let ascii = format!("[{name}] {division}");
        let wide = format!("［{name}］ {division}");
        prop_assert_eq!(
            is_name_and_first_divisions(&candidate, &wide),
            is_name_and_first_divisions(&candidate, &ascii)
        );
        let plain = name.to_string();
        prop_assert!(is_name_and_first_divisions(&plain, &wide) || division == "Season 1");
    }
}
