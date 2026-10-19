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

const ORDINALS: [(&str, bool); 5] = [
    ("First", true),
    ("Second", false),
    ("Seventh", false),
    ("2nd", false),
    ("1st", true),
];

proptest! {
    /// An ordinal written before "Half" is one division with it, as the
    /// reader reads "First Half" and "Second Half": it ends the name
    /// whatever follows, and leaves nothing past the name. So the entry
    /// "Show" names "Show First Half", whose division is the whole, and
    /// for the entry "Show Second Half" the bare "Show" is the entry
    /// before.
    #[test]
    fn an_ordinal_before_half_is_a_division_that_ends_the_name(
        name_of in proptest::sample::select(NAMES.to_vec()),
        (ordinal, first) in proptest::sample::select(ORDINALS.to_vec()),
    ) {
        let title = format!("{name_of} {ordinal} Half");
        let want: Vec<String> = name_of.split(' ').map(str::to_lowercase).collect();
        prop_assert_eq!(name(&title), want.clone());
        prop_assert!(undivided_rest(&title).is_empty(), "{}", title);
        prop_assert_eq!(name(&format!("{title}: Arc")), want);
        let bare = [name_of];
        prop_assert_eq!(EntryTitles::new(&bare).names(&title), first, "{}", title);
        let titles = [title.as_str()];
        prop_assert_eq!(EntryTitles::new(&titles).before_entry(name_of), !first, "{}", title);
    }
}

/// A word ordinal inside a name ("The Seventh Son") is a word of the
/// name, not a division the title reads as: the title reads as nothing,
/// so the entry "The Seventh Son Part 2" has it as the entry before.
#[test]
fn a_word_ordinal_inside_the_name_is_no_division_of_the_reading() {
    assert!(reading("The Seventh Son").is_empty());
    assert_eq!(reading("The Seventh Son Season 2"), vec![2]);
    assert_eq!(reading("Show Seventh"), vec![7]);
    let titles = ["The Seventh Son Part 2"];
    assert!(EntryTitles::new(&titles).before_entry("The Seventh Son"));
    let titles = ["The Seventh Son: 第2クール"];
    assert!(EntryTitles::new(&titles).before_entry("The Seventh Son"));
}

proptest! {
    /// What a word holds before a Japanese division glued to it is read
    /// as a word of its own: "Fourth第十二部" is the season "Fourth" and
    /// the part 第十二部, as "Fourth・第十二部" is, so the two titles read
    /// alike and one names the other.
    #[test]
    fn a_word_glued_before_a_japanese_division_is_read_as_its_own(
        name_of in proptest::sample::select(NAMES.to_vec()),
        (ordinal, n) in proptest::sample::select(vec![("Fourth", 4u32), ("2nd", 2), ("III", 3)]),
    ) {
        let glued = format!("{name_of} {ordinal}第十二部");
        let apart = format!("{name_of}: {ordinal}・第十二部");
        prop_assert_eq!(reading(&glued), vec![n, 12]);
        prop_assert_eq!(reading(&apart), vec![n, 12]);
        let titles = [apart.as_str()];
        prop_assert!(EntryTitles::new(&titles).names(&glued), "{}", glued);
    }
}

/// What a candidate names past an entry title's stem is read as the
/// reader reads it: a word ordinal inside a name names no season
/// there either, so "Second" with a word after it is no season before
/// "Spy x Family, 3".
#[test]
fn past_the_stem_a_word_ordinal_inside_a_name_names_no_season() {
    let titles = ["Spy x Family, 3"];
    assert!(!EntryTitles::new(&titles).another_entry("Spy x Family. Second Half12"));
    let titles = ["Show: Arc - IX"];
    assert!(!EntryTitles::new(&titles).another_entry("Show: Arc - Seventh CourSecond Part"));
    // a division still is one: "Second Half" names a season 2
    let titles = ["Show 3"];
    assert!(EntryTitles::new(&titles).another_entry("Show Second Half"));
}

proptest! {
    /// A number glued to the end of a Japanese division is read after
    /// that division, as it is after any Japanese word: "第3部12" is the
    /// part 3 and a 12, and "第〇期10" names its season 0 — a season
    /// before an entry's first.
    #[test]
    fn a_number_glued_after_a_japanese_division_is_read_after_it(
        n in 0u32..100,
        k in 1u32..10,
    ) {
        // a division whose ordinal is 1 is the whole, and left out
        let want: Vec<u32> = [k, n].into_iter().filter(|&x| x != 1).collect();
        prop_assert_eq!(reading(&format!("Show 第{k}部{n}")), want);
        let titles = ["Other Show 1st"];
        let candidate = format!("Other Show 第〇期{n} Recap");
        prop_assert!(EntryTitles::new(&titles).another_entry(&candidate), "{}", candidate);
    }
}

/// A word ordinal past a division with a word after it is a word of
/// what follows the name, as it would be of the name itself: "Show 2:
/// Second Arc" carries "Second Arc" past its name, and is not named by
/// a title carrying "Arc" alone.
#[test]
fn a_word_ordinal_past_a_division_is_a_word_of_the_rest() {
    assert_eq!(undivided_rest("Show 2: Second Arc"), vec!["second", "arc"]);
    assert_eq!(
        undivided_rest("Show 2nd Season: Seventh Son"),
        vec!["seventh", "son"]
    );
    let titles = ["Show 2: Second Arc"];
    assert!(!EntryTitles::new(&titles).names("Show 2 Arc"));
    assert!(EntryTitles::new(&titles).names("Show 2 - Second Arc"));
}
