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
        vec![Marker { ordinals: vec![2] }]
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

proptest::proptest! {
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
