//! Property coverage for whether the entry names a title: its name,
//! its reading and the words it carries past its name outside its
//! divisions.

use super::*;
use proptest::prelude::*;

const NAMES: [&str; 3] = ["Show", "Other Show", "Kaiju Story"];
const DIVISIONS: [&str; 6] = ["", " 2", " 2nd Season", " Season 3", " Part 2", " II"];
const WORDS: [&str; 4] = ["Extra", "Arc", "Movie", "Encore"];

fn subtitle() -> impl Strategy<Value = Vec<&'static str>> {
    proptest::collection::vec(proptest::sample::select(WORDS.to_vec()), 0..3)
}

fn title(name: &str, division: &str, subtitle: &[&str]) -> String {
    if subtitle.is_empty() {
        format!("{name}{division}")
    } else {
        format!("{name}{division}: {}", subtitle.join(" "))
    }
}

proptest! {
    /// The words past a name are the subtitle, whatever division sits
    /// between, and nothing when there is none. A title with no
    /// division has no rest: its name runs to its end, subtitle and
    /// all, and the name is what tells two such titles apart.
    #[test]
    fn the_rest_past_a_name_is_its_subtitle(
        name in proptest::sample::select(NAMES.to_vec()),
        division in proptest::sample::select(DIVISIONS.to_vec()),
        sub in subtitle(),
    ) {
        let want: Vec<String> = if division.is_empty() {
            Vec::new()
        } else {
            sub.iter().map(|w| w.to_lowercase()).collect()
        };
        let got: Vec<String> = undivided_rest(&title(name, division, &sub))
            .into_iter()
            .map(|w| w.to_lowercase())
            .collect();
        prop_assert_eq!(got, want);
    }

    /// An entry names each of its own titles, in any case, and names
    /// no title that differs from them only by its subtitle.
    #[test]
    fn an_entry_names_its_titles_and_no_other_subtitle(
        name in proptest::sample::select(NAMES.to_vec()),
        division in proptest::sample::select(DIVISIONS.to_vec()),
        sub in subtitle(),
        other in subtitle(),
    ) {
        let own = title(name, division, &sub);
        let titles = [own.as_str()];
        let entry = EntryTitles::new(&titles);
        prop_assert!(entry.names(&own));
        prop_assert!(entry.names(&own.to_uppercase()));
        let same = sub.iter().map(|w| w.to_lowercase()).collect::<Vec<_>>()
            == other.iter().map(|w| w.to_lowercase()).collect::<Vec<_>>();
        prop_assert_eq!(entry.names(&title(name, division, &other)), same);
    }
}

/// A Japanese division glued to the name is read past like one written
/// apart: what follows it is the subtitle, and an entry so titled names
/// the same title and not its season alone.
#[test]
fn a_glued_japanese_division_is_read_past_to_the_subtitle() {
    assert_eq!(
        undivided_rest("ショー第2期: 番外編"),
        vec!["番外編".to_string()]
    );
    assert!(undivided_rest("ショー第2期").is_empty());
    let titles = ["ショー第2期: 番外編"];
    let entry = EntryTitles::new(&titles);
    assert!(entry.names("ショー 第2期 番外編"));
    assert!(!entry.names("ショー第2期"));
}
