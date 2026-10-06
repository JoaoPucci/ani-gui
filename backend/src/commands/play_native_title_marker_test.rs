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
