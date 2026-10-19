use super::*;

fn entry<'a>(titles: &'a [&'a str]) -> EntryTitles<'a> {
    EntryTitles::new(titles)
}

// ── which candidates an entry admits ────────────────────────────────

#[test]
fn a_season_the_entry_never_names_is_not_admitted() {
    // [Oshi no Ko]'s first season, against hianime's pools: its titles
    // name no second or third anything.
    let e = entry(&[
        "[Oshi no Ko]",
        "【推しの子】",
        "Oshi no Ko",
        "My Idol's Child",
    ]);
    assert!(!e.admits("My Star: Season 2"));
    assert!(!e.admits("My Star Season 3"));
    assert!(e.admits("My Star"));
    assert!(
        e.admits("[Oshi no Ko] Final Season"),
        "no ordinal, no claim"
    );
}

#[test]
fn a_season_named_under_another_kind_is_admitted() {
    // The catalogues disagree on the kind: Kitsu's "86 Part 2" is
    // hianime's "Eighty Six: 2nd Season".
    let e = entry(&["86 Part 2"]);
    assert!(e.admits("Eighty Six: 2nd Season"));
    // A trailing number or numeral names the season too.
    assert!(entry(&["Tensei shitara Slime Datta Ken 2"]).admits("Slime Season 2"));
    assert!(entry(&["Overlord II"]).admits("Overlord Season 2"));
    // So does any alias.
    assert!(entry(&["Haikyuu!!: To the Top", "Haikyuu!! 4th Season"]).admits("Haikyuu!! Season 4"));
}

#[test]
fn a_first_part_and_a_span_through_it_never_disqualify() {
    let e = entry(&["Attack on Titan: The Final Season"]);
    assert!(e.admits("Attack on Titan: Final Season, Part 1"));
    assert!(!e.admits("Attack on Titan: Final Season, Part 2"));
    let e = entry(&["Haikyuu!!: To the Top"]);
    assert!(e.admits("Haikyuu!!: To the Top (Part 1+2)"));
    assert!(!e.admits("Haikyuu!!: To the Top 2nd Season"));
}

#[test]
fn a_marker_inside_the_title_is_a_story_part_not_a_cour() {
    // JoJo numbers its story parts in the middle of the title; only
    // what a title ends on is a division of the entry.
    let e = entry(&["JoJo no Kimyou na Bouken: Stardust Crusaders"]);
    assert!(e.admits("JoJo's Bizarre Adventure Part 2: Stardust Crusaders (Uncensored)"));
    assert!(!e.admits("JoJo's Bizarre Adventure Part 3: Stardust Crusaders 2nd Season"));
}

#[test]
fn every_marker_a_title_ends_on_must_be_named() {
    // "Season 3 Part 2" ends on both; Season 3's entry names only 3.
    let e = entry(&["Attack on Titan Season 3"]);
    assert!(e.admits("Attack on Titan Season 3"));
    assert!(!e.admits("Attack on Titan Season 3 Part 2"));
}

#[test]
fn japanese_divisions_are_read_as_markers() {
    assert_eq!(
        trailing_markers("推しの子 第2期"),
        vec![Marker {
            kind: Kind::Season,
            ordinals: vec![2]
        }]
    );
    assert!(entry(&["Shingeki no Kyojin", "進撃の巨人 第3期"]).admits("Attack on Titan Season 3"));
}

#[test]
fn a_bare_number_is_not_an_ordinal_beside_a_marker() {
    // "100 season" is not a hundredth season; "Mob Psycho 100" names
    // no division at all.
    assert!(trailing_markers("Mob Psycho 100").is_empty());
    assert!(trailing_markers("Show 100 Season").is_empty());
    assert_eq!(named_ordinals("Mob Psycho 100"), BTreeSet::new());
}

#[test]
fn a_bare_number_a_title_ends_on_is_held_to_what_the_entry_names() {
    // A sequel is numbered with a bare number as often as with a
    // marker: Overlord's titles name no 2.
    let e = entry(&["Overlord", "オーバーロード"]);
    assert!(!e.admits("Overlord: Ple Ple Pleiades 2"));
    assert!(e.admits("Overlord: Ple Ple Pleiades"));
    let e = entry(&["Show"]);
    assert!(!e.admits("Show 2"));
    assert!(!e.admits("Show 2 Part 1"), "the number before the markers");
    assert!(!e.admits("ショー２"), "a number glued to a Japanese title");
    assert!(e.admits("Show 1"), "a first never disqualifies");
    assert!(entry(&["Show 2"]).admits("Show 2"));
    assert!(entry(&["ショー２"]).admits("Show 2"));
    // A number the entry's titles name is admitted; three digits name
    // no sequel.
    assert!(entry(&["Kaijuu 8-gou", "Kaiju No. 8"]).admits("Kaiju No. 8"));
    assert!(entry(&["Mob Psycho 100"]).admits("Mob Psycho 100"));
    assert!(entry(&["Mob Psycho"]).admits("Mob Psycho 100"));
}

#[test]
fn a_number_before_a_japanese_counter_is_named_by_the_entry() {
    // Japanese writes the number of "No. 8" glued between the title
    // and a counter, often full-width: an entry known only as
    // "怪獣８号" names 8.
    for title in ["怪獣８号", "怪獣8号", "ショー２話", "ショー2期"] {
        assert!(!named_ordinals(title).is_empty(), "{title}");
    }
    assert!(entry(&["怪獣８号"]).admits("Kaiju No. 8"));
    assert!(!entry(&["怪獣８号"]).admits("Kaiju No. 9"));
    assert!(entry(&["ショー２話"]).admits("Show 2"));
}

proptest::proptest! {
    /// A number of one or two digits written straight before a
    /// Japanese counter is named, in ASCII or full-width digits; three
    /// digits name nothing.
    #[test]
    fn a_number_before_a_counter_is_named_exactly_when_small(
        head in "[ぁ-んァ-ン一-龥]{1,4}",
        n in proptest::prop_oneof![0u32..100, 100u32..1000],
        wide in proptest::bool::ANY,
        counter in proptest::sample::select(vec!['号', '期', '話']),
    ) {
        let digits: String = n
            .to_string()
            .chars()
            .map(|c| if wide { char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap() } else { c })
            .collect();
        // 第 before the number makes a division, read by its own rule.
        proptest::prop_assume!(!head.ends_with('第'));
        let title = format!("{head}{digits}{counter}");
        proptest::prop_assert_eq!(named_ordinals(&title).contains(&n), n < 100, "{:?}", title);
    }

    /// A title of plain words followed by a bare number is admitted by
    /// an entry of plain words exactly when the number is a first or
    /// has three digits — the entry names nothing else.
    #[test]
    fn a_bare_number_the_entry_never_names_is_refused(
        a in "[A-Za-z]{1,8}( [A-Za-z]{1,8}){0,3}",
        b in "[A-Za-z]{1,8}( [A-Za-z]{1,8}){0,3}",
        n in proptest::prop_oneof![0u32..100, 100u32..1000],
        padded in proptest::bool::ANY,
    ) {
        proptest::prop_assume!(trailing_markers(&a).is_empty() && trailing_markers(&b).is_empty());
        proptest::prop_assume!(named_ordinals(&a).is_empty());
        // A one-digit number written with a leading zero ("02") is
        // the same number.
        let number = if padded && n < 10 { format!("0{n}") } else { n.to_string() };
        let candidate = format!("{b} {number}");
        // "Show Season" and a number make a marker, read by the rule
        // above; only a bare number is under test.
        proptest::prop_assume!(trailing_markers(&candidate).is_empty());
        proptest::prop_assert_eq!(
            EntryTitles::bare(&a).admits(&candidate),
            n == 1 || n >= 100,
            "{:?} for {:?}", candidate, a
        );
    }

    /// Over titles built from plain words and markers in any order,
    /// an entry admits its own title, whatever the markers say — no
    /// rule can refuse the very title the entry goes by.
    #[test]
    fn an_entry_admits_its_own_title(
        parts in proptest::collection::vec(
            proptest::prop_oneof![
                proptest::strategy::Just("Show"),
                proptest::strategy::Just("Arc"),
                proptest::strategy::Just("100"),
                proptest::strategy::Just("Season 2"),
                proptest::strategy::Just("3rd Season"),
                proptest::strategy::Just("Part 2"),
                proptest::strategy::Just("Part 1+2"),
                proptest::strategy::Just("Cour 3"),
                proptest::strategy::Just("II"),
                proptest::strategy::Just("第二部"),
                proptest::strategy::Just(":"),
                proptest::strategy::Just("(Part 4)"),
            ],
            1..6,
        ),
    ) {
        let title = parts.join(" ");
        proptest::prop_assert!(EntryTitles::bare(&title).admits(&title), "{title:?} refused itself");
    }

    /// A title that ends on no marker is admitted by every entry
    /// whose canonical title ends on none: the rules only ever read
    /// the divisions titles claim.
    #[test]
    fn titles_without_markers_never_refuse_each_other(
        a in "[A-Za-z]{1,8}( [A-Za-z]{1,8}){0,3}",
        b in "[A-Za-z]{1,8}( [A-Za-z]{1,8}){0,3}",
    ) {
        proptest::prop_assume!(trailing_markers(&a).is_empty() && trailing_markers(&b).is_empty());
        proptest::prop_assert!(EntryTitles::bare(&a).admits(&b));
    }
}

// ── which part a candidate's title agrees on ────────────────────────

#[test]
fn a_part_marker_agrees_only_with_the_part_the_entry_ends_on() {
    // Slime's second season: Kitsu's titles end on "Part 1" or on no
    // part at all; hianime lists "Season 2" and "2nd Season Part 2"
    // at the same length and year.
    let e = entry(&[
        "Tensei shitara Slime Datta Ken 2",
        "Tensei Shitara Slime Datta Ken 2nd Season Part 1",
    ]);
    assert!(e.part_agrees("That Time I Got Reincarnated as a Slime Season 2"));
    assert!(!e.part_agrees("That Time I Got Reincarnated as a Slime 2nd Season Part 2"));
    let e = entry(&["Tensei shitara Slime Datta Ken 2nd Season Part 2"]);
    assert!(!e.part_agrees("That Time I Got Reincarnated as a Slime Season 2"));
    assert!(e.part_agrees("That Time I Got Reincarnated as a Slime 2nd Season Part 2"));
    // No part on either side is agreement.
    assert!(entry(&["Vinland Saga Season 2"]).part_agrees("Vinland Saga: 2nd Season"));
}

proptest::proptest! {
    /// An entry agrees with the part its own title ends on, whatever
    /// that title is.
    #[test]
    fn an_entry_agrees_with_its_own_part(
        stem in "[A-Za-z]{1,8}( [A-Za-z]{1,8}){0,2}",
        marker in proptest::prop_oneof![
            proptest::strategy::Just(""),
            proptest::strategy::Just(" Part 2"),
            proptest::strategy::Just(" Season 2 Part 3"),
            proptest::strategy::Just(" (Part 1+2)"),
            proptest::strategy::Just(" 2nd Cour"),
        ],
    ) {
        let title = format!("{stem}{marker}");
        proptest::prop_assert!(EntryTitles::bare(&title).part_agrees(&title));
    }
}

// ── the name the seasons share ──────────────────────────────────────

#[test]
fn the_stem_drops_every_marker_a_title_ends_on() {
    let words = |t: &str| t.split(' ').map(str::to_string).collect::<Vec<_>>();
    assert_eq!(
        stem("Attack on Titan Season 3 Part 2"),
        words("attack on titan")
    );
    assert_eq!(
        stem("Haikyuu!!: To the Top (Part 1+2)"),
        words("haikyuu to the top")
    );
    assert_eq!(
        stem("Gintama.: Silver Soul Arc"),
        words("gintama silver soul arc")
    );
}

#[test]
fn a_numeral_or_number_inside_the_entry_s_title_names_its_season() {
    // Kitsu calls Mushoku Tensei's second season "Mushoku Tensei II:
    // Isekai Ittara Honki Dasu"; hianime calls it "… Season 2".
    let e = entry(&["Mushoku Tensei II: Isekai Ittara Honki Dasu"]);
    assert!(e.admits("Mushoku Tensei: Jobless Reincarnation Season 2"));
    assert!(entry(&["Show 2: The Return"]).admits("Show Season 2"));
}

#[test]
fn the_rules_read_every_marker_form_the_part_reader_reads() {
    // A sequel numbered by a numeral or an ordinal alone, or by a
    // stage, is named for a season its predecessor's titles never
    // name — as hianime lists Mob Psycho 100's and Overlord's later
    // seasons, and Steel Ball Run's weekly run.
    assert!(!entry(&["Mob Psycho 100"]).admits("Mob Psycho 100 II"));
    assert!(entry(&["Mob Psycho 100 II"]).admits("Mob Psycho 100 II"));
    assert!(!entry(&["Overlord II"]).admits("Overlord III"));
    let stone_ocean = entry(&["JoJo no Kimyou na Bouken: Stone Ocean", "Jojo part 6"]);
    assert!(!stone_ocean.admits("Steel Ball Run: JoJo's Bizarre Adventure 2nd Stage"));
    assert!(!entry(&["Show"]).part_agrees("Show 2nd Stage"));
}

#[test]
fn spelled_full_width_and_japanese_cour_forms_are_read() {
    assert!(!entry(&["Show"]).admits("Show Season Two"));
    assert!(entry(&["Show 2nd Season"]).admits("Show Season Two"));
    assert_eq!(
        trailing_markers("無職転生 第2クール"),
        vec![Marker {
            kind: Kind::Part,
            ordinals: vec![2]
        }]
    );
    assert_eq!(
        trailing_markers("ショー 第10期"),
        vec![Marker {
            kind: Kind::Season,
            ordinals: vec![10]
        }]
    );
    // Full-width digits, as Kitsu's Japanese titles write them, also
    // where they follow the title with no space.
    assert!(entry(&["Show", "ショー 第２期"]).admits("Show Season 2"));
    assert!(entry(&["Show", "ショー２"]).admits("Show Season 2"));
}

#[test]
fn only_the_first_part_is_admitted_unnamed_and_zero_is_an_ordinal_like_any_other() {
    // Ordinal 1 — or a span through it — needs no naming. Zero is a
    // marker like any other: admitted only where the entry names it.
    let e = entry(&["Show"]);
    assert!(e.admits("Show Part 1"));
    assert!(e.admits("Show (Part 1+2)"));
    assert!(!e.admits("Show Season 0"));
    assert!(!e.admits("Show Part 0+2"));
    assert!(entry(&["Jujutsu Kaisen 0"]).admits("Jujutsu Kaisen Season 0"));
    assert_eq!(
        trailing_markers("Show Part 0+2"),
        vec![Marker {
            kind: Kind::Part,
            ordinals: vec![0, 2]
        }]
    );
}

#[test]
fn the_stem_drops_a_japanese_division_too() {
    let words = |t: &str| t.split(' ').map(str::to_string).collect::<Vec<_>>();
    assert_eq!(stem("進撃の巨人 第3期"), words("進撃の巨人"));
    assert_eq!(stem("Show Season 2 第2部"), words("show"));
}

#[test]
fn a_span_without_the_first_part_needs_every_ordinal_named() {
    // A span through part 1 stays the first-part exception; any other
    // span is admitted only when the entry names every part in it.
    assert!(!entry(&["Show 2"]).admits("Show Part 0+2"));
    assert!(!entry(&["Show 2"]).admits("Show Part 2+3"));
    assert!(entry(&["Show 2", "Show 3"]).admits("Show Part 2+3"));
    assert!(entry(&["Show"]).admits("Show (Part 1+2)"));
    // Agreement on a part reads a span the same way.
    let e = entry(&["Show Part 2"]);
    assert!(!e.part_agrees("Show Part 0+2"));
    assert!(!e.part_agrees("Show Part 2+3"));
    assert!(entry(&["Show Part 2", "Show Part 3"]).part_agrees("Show Part 2+3"));
}

#[test]
fn japanese_divisions_glued_one_after_another_are_each_read() {
    // "ショー第2期第2部" is the second season's second part, glued in
    // one word: it reads [2, 2] and is named "ショー", so it is no
    // listing of the second season as a whole.
    assert_eq!(reading("ショー第2期第2部"), vec![2, 2]);
    assert_eq!(name("ショー第2期第2部"), vec!["ショー".to_string()]);
    let e = entry(&["ショー 第2期"]);
    assert!(!e.reads_as_entry("ショー第2期第2部"));
    assert!(e.reads_as_entry("ショー第2期"));
}

proptest::proptest! {
    /// A sibling that repeats the broad listing's own season and goes on
    /// with an ordinal standing alone is never the season before it, and
    /// is a later part whenever that ordinal is above the first.
    #[test]
    fn an_ordinal_after_w_s_season_is_a_part_of_it(
        season in 1u32..=20,
        form in 0usize..3,
        ordinal in 1u32..=6,
    ) {
        const SPELLED: [&str; 6] = ["First", "Second", "Third", "Fourth", "Fifth", "Sixth"];
        let wide = format!("Show Season {season}");
        let after = match form {
            0 => SPELLED[ordinal as usize - 1].to_string(),
            1 => format!("{ordinal}{}", if ordinal == 1 { "st" } else if ordinal == 2 { "nd" } else if ordinal == 3 { "rd" } else { "th" }),
            _ => format!("Part {ordinal}"),
        };
        let sibling = format!("{wide} {after} Half");
        let titles = [wide.as_str()];
        let e = entry(&titles);
        proptest::prop_assert!(!e.names_earlier_season(&sibling, &wide), "{sibling}");
        proptest::prop_assert_eq!(e.names_later_part(&sibling, &wide), ordinal > 1, "{}", sibling);
    }
}

// ── catalogue tags ──────────────────────────────────────────────────

const TAGGED_TITLES: &[&str] = &[
    "Show",
    "Show 2",
    "Show Season 2",
    "Show Part 2",
    "Show 2nd Season Part 2",
    "Show II Part 2",
    "Kaiju No. 8 Season 2",
    "ショー第2期",
    "ショー２",
    "86 Part 2",
    "Gintama'",
];

const CATALOGUE_TAGS: &[&str] = &[
    "(2024)",
    "(TV)",
    "[OVA]",
    "（ＯＮＡ）",
    "【OAD】",
    "（２０２０）",
    // each character of a tag in ASCII or full-width on its own
    "(ｔv)",
    "（２0２0）",
    "[Ｏva]",
    // full-width square brackets, as round and lenticular ones are
    "［ＴＶ］",
    "［2020］",
];

proptest::proptest! {
    /// Every reader reads a title as it reads the title without its
    /// catalogue tags, wherever the tag stands.
    #[test]
    fn every_reader_reads_a_title_without_its_catalogue_tags(
        t in 0..TAGGED_TITLES.len(),
        g in 0..CATALOGUE_TAGS.len(),
        at_end in proptest::bool::ANY,
    ) {
        use super::super::play_native_part_title::part_ordinal;
        use super::super::play_native_title_grammar::{named_ordinals, part_ordinals, stem_number, trailing_markers};
        use super::super::play_native_title_reading::reading;
        let plain = TAGGED_TITLES[t];
        let tag = CATALOGUE_TAGS[g];
        let tagged = if at_end {
            format!("{plain} {tag}")
        } else {
            let (first, rest) = plain.split_once(' ').unwrap_or((plain, ""));
            format!("{first} {tag} {rest}")
        };
        proptest::prop_assert_eq!(trailing_markers(&tagged), trailing_markers(plain), "markers of {}", tagged);
        proptest::prop_assert_eq!(stem(&tagged), stem(plain), "stem of {}", tagged);
        proptest::prop_assert_eq!(stem_number(&tagged), stem_number(plain), "stem number of {}", tagged);
        proptest::prop_assert_eq!(named_ordinals(&tagged), named_ordinals(plain), "named in {}", tagged);
        proptest::prop_assert_eq!(part_ordinals(&tagged), part_ordinals(plain), "parts of {}", tagged);
        proptest::prop_assert_eq!(reading(&tagged), reading(plain), "reading of {}", tagged);
        proptest::prop_assert_eq!(name(&tagged), name(plain), "name of {}", tagged);
        proptest::prop_assert_ne!(normalized(&tagged), normalized(plain), "{} as written keeps its tag", tagged);
        if part_ordinal("Show", plain) != Some(1) {
            proptest::prop_assert_eq!(part_ordinal("Show", &tagged), part_ordinal("Show", plain), "part of Show in {}", tagged);
        } else {
            proptest::prop_assert_ne!(part_ordinal("Show", &tagged), Some(1), "{} is not the stem as written", tagged);
        }
        let e = entry(&["Show"]);
        proptest::prop_assert_eq!(e.admits(&tagged), e.admits(plain), "Show admits {}", tagged);
    }
}

// ── a number glued to a word ────────────────────────────────────────

/// Words a number may be glued to, ending on a Japanese letter: kana,
/// a kanji, the iteration mark, half-width katakana.
const JAPANESE_HEADS: &[&str] = &["ショー", "怪獣", "時々", "ｼｮｰ", "しょう"];

/// Words ending on a letter outside Japanese, ASCII or not: a Latin
/// word, an accented one, a full-width Latin one, Cyrillic, Hangul.
const OTHER_HEADS: &[&str] = &["show", "café", "ｓｈｏｗ", "шоу", "나혼자만레벨업"];

proptest::proptest! {
    /// A number glued to the end of a word is a number only after a
    /// Japanese letter, as Japanese writes one ("ショー2"); after any
    /// other letter it is the word's own ("Show2", "ＳＨＯＷ２").
    #[test]
    fn a_number_is_glued_only_to_a_japanese_word(
        h in 0..JAPANESE_HEADS.len(),
        o in 0..OTHER_HEADS.len(),
        n in 0u32..100,
    ) {
        use super::super::play_native_title_grammar::glued_number;
        proptest::prop_assert_eq!(glued_number(&format!("{}{n}", JAPANESE_HEADS[h])), Some(n));
        proptest::prop_assert_eq!(glued_number(&format!("{}{n}", OTHER_HEADS[o])), None);
    }
}

/// Letters a number may stand glued before: Japanese ones only.
const JAPANESE_AFTER: &[char] = &['第', 'シ', '期', 'ｼ'];
const OTHER_AFTER: &[char] = &['é', 'ｓ', 'ш', '나', 'a'];

proptest::proptest! {
    /// Digits stand as a number before a space, the end, or a Japanese
    /// letter glued to them ("20第2部"); before any other letter they
    /// are a word's ("1é", "20a").
    #[test]
    fn a_number_stands_before_a_japanese_letter_only(
        j in 0..JAPANESE_AFTER.len(),
        o in 0..OTHER_AFTER.len(),
        n in 0u32..100,
    ) {
        use super::super::play_native_title_reading::leading_number;
        proptest::prop_assert_eq!(leading_number(&format!("{n}")).map(|x| x.0), Some(n));
        proptest::prop_assert_eq!(leading_number(&format!("{n} season 1")).map(|x| x.0), Some(n));
        proptest::prop_assert_eq!(leading_number(&format!("{n}{}1部", JAPANESE_AFTER[j])).map(|x| x.0), Some(n));
        proptest::prop_assert_eq!(leading_number(&format!("{n}{} season 1", OTHER_AFTER[o])), None);
    }
}

proptest::proptest! {
    /// An entry names a number its title writes the way Japanese glues
    /// one — straight before a Japanese letter ("86第4部") or straight
    /// after one ("ショー2") — and never digits glued to any other
    /// letter, which are that word's own ("Arc2", "Café2"): a title the
    /// entry carries only so does not admit a sequel numbered by them.
    #[test]
    fn an_entry_names_a_number_glued_only_as_japanese_writes_one(
        h in 0..JAPANESE_HEADS.len(),
        o in 0..OTHER_HEADS.len(),
        n in 0u32..100,
    ) {
        let before = format!("{n}第4部");
        proptest::prop_assert!(named_ordinals(&before).contains(&n), "{before}");
        let after = format!("{}{n}", JAPANESE_HEADS[h]);
        proptest::prop_assert!(named_ordinals(&after).contains(&n), "{after}");
        let latin = format!("{}{n}", OTHER_HEADS[o]);
        proptest::prop_assert!(!named_ordinals(&latin).contains(&n), "{latin}");
    }
}

/// The admission those numbers decide: "86☆" ends on the 86 that
/// "86第４部" names, while "Show: Arc: 2" ends on a 2 that "Show: Arc2"
/// does not.
#[test]
fn admission_reads_a_number_glued_only_as_japanese_writes_one() {
    assert!(entry(&["86第４部"]).admits("86☆"));
    assert!(!entry(&["Show: Arc2"]).admits("Show: Arc: 2"));
    assert!(!entry(&["Gintama4 [OVA]"]).admits("Gintama~ 4"));
    assert!(entry(&["ショー２"]).admits("ショー: 2"));
    // a span is a word of its own however it is glued before: its
    // numbers stand apart, "+" between them
    assert!(entry(&["推しの子Part ４+5"]).admits("推しの子 Part ４+5"));
}

/// Spelled and suffixed ordinals that pair with the marker word after
/// them ("Third Stage", "12th Cour").
const PAIRED: &[(&str, u32)] = &[("Third", 3), ("Seventh", 7), ("12th", 12), ("4th", 4)];

proptest::proptest! {
    /// A title's tail is one reading of it, first word to last: a
    /// spelled or suffixed ordinal pairs with the marker after it, so
    /// "Third Stage 2" is a third stage and then a bare 2 — the title
    /// ends on that number, not on a "Stage 2" — and "12th Cour 3rd" a
    /// twelfth cour and then a third season standing alone. The stem
    /// and the markers are what that one reading ends on.
    #[test]
    fn the_tail_is_the_reading_s_own_parse(
        p in 0..PAIRED.len(),
        n in 0u32..10,
    ) {
        use super::super::play_native_title_grammar::{stem_number, trailing_markers};
        let (ordinal, k) = PAIRED[p];
        let numbered = format!("Show {ordinal} Stage {n}");
        proptest::prop_assert!(trailing_markers(&numbered).is_empty(), "{}", numbered);
        proptest::prop_assert_eq!(stem_number(&numbered), Some(n));
        let spelled = format!("Show {ordinal} Cour 3rd");
        let got: Vec<(Kind, Vec<u32>)> = trailing_markers(&spelled).into_iter().map(|m| (m.kind, m.ordinals)).collect();
        proptest::prop_assert_eq!(got, vec![(Kind::Season, vec![3]), (Kind::Part, vec![k])]);
        // the entry known by such a title has its own words, retagged, for
        // its own listing, never for the season before it
        let titles = [numbered.as_str()];
        let tagged = format!("Show {ordinal} (2011) Stage {n}");
        proptest::prop_assert!(!entry(&titles).another_entry(&tagged), "{}", tagged);
    }
}

/// A "Half" is no marker word: "Show Second Half" ends on no marker,
/// and its stem is the whole title.
#[test]
fn a_half_ends_no_marker_run() {
    use super::super::play_native_title_grammar::trailing_markers;
    assert!(trailing_markers("Show Second Half").is_empty());
    assert_eq!(stem("Show Second Half"), vec!["show", "second", "half"]);
}
