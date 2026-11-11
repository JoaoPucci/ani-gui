//! The franchise's name, as the answer carries it: the title of the
//! main story's first series by start (`name_from`), else of its first
//! entry, with Kitsu's format suffix dropped unless another entry of
//! the franchise would then read the same. The franchise page's
//! heading and artwork, and the front card of the strip's poster
//! stack, come from that entry.

use proptest::prelude::*;

use super::prop_tests::{catalogue, drive_with};
use super::tests::{drive, Catalogue};
use super::*;

/// The answer's `name` and `name_from`.
fn named(f: &Franchise) -> (String, String) {
    let v = serde_json::to_value(f).expect("json");
    let field = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    (field("name"), field("name_from"))
}

/// Add `id` with its format, start, count and title.
fn add(
    c: &mut Catalogue,
    id: &str,
    subtype: &str,
    start: &str,
    episodes: Option<u32>,
    title: &str,
) {
    c.add(id, subtype, Some(start));
    let e = c.entries.get_mut(id).expect("entry");
    e.episodes = episodes;
    e.title = title.into();
}

/// `a` is the prequel of `b`, from both sides.
fn link(c: &mut Catalogue, a: &str, b: &str) {
    c.rel(a, "sequel", b).rel(b, "prequel", a);
}

/// Lupin III: the 1969 pilot film, the 1971 series, and LUPIN ZERO, a
/// 2022 ONA Kitsu lists as the 1971 series' prequel.
#[test]
fn lupin_iii_is_named_after_its_first_series_not_its_pilot_film() {
    let mut c = Catalogue::default();
    add(
        &mut c,
        "3997",
        "movie",
        "1969-01-01",
        Some(1),
        "Lupin III: Pilot Film",
    );
    add(&mut c, "1265", "TV", "1971-10-24", Some(23), "Lupin III");
    add(&mut c, "46654", "ONA", "2022-12-16", Some(6), "LUPIN ZERO");
    link(&mut c, "3997", "1265");
    link(&mut c, "46654", "1265");
    for anchor in ["3997", "1265", "46654"] {
        assert_eq!(
            named(&drive(anchor, &c)),
            ("Lupin III".into(), "1265".into())
        );
    }
}

/// FLCL: the 2000 OVA of six episodes, then the 2018 TV series. An OVA
/// of more than one episode is a series.
#[test]
fn flcl_is_named_after_its_ova() {
    let mut c = Catalogue::default();
    add(&mut c, "202", "OVA", "2000-04-26", Some(6), "FLCL");
    add(
        &mut c,
        "11961",
        "TV",
        "2018-06-02",
        Some(6),
        "FLCL Progressive",
    );
    link(&mut c, "202", "11961");
    assert_eq!(named(&drive("11961", &c)), ("FLCL".into(), "202".into()));
}

/// One-offs before the first series are skipped: a film, a special of
/// any count, a one-episode OVA or ONA, a music video.
#[test]
fn one_offs_before_the_first_series_are_skipped() {
    let one_offs = [
        ("movie", Some(1)),
        ("special", Some(12)),
        ("OVA", Some(1)),
        ("ONA", Some(1)),
        ("music", Some(3)),
    ];
    for (subtype, episodes) in one_offs {
        let mut c = Catalogue::default();
        add(&mut c, "1", subtype, "2000-01-01", episodes, "Before");
        add(&mut c, "2", "TV", "2003-01-01", Some(12), "Series (TV)");
        link(&mut c, "1", "2");
        assert_eq!(
            named(&drive("1", &c)),
            ("Series".into(), "2".into()),
            "{subtype} {episodes:?}"
        );
    }
}

/// A series whose count Kitsu does not give (still airing, or only
/// announced) is a series; a count of zero is no count.
#[test]
fn a_series_of_unknown_count_is_a_series() {
    for episodes in [None, Some(0)] {
        let mut c = Catalogue::default();
        add(&mut c, "1", "ONA", "2000-01-01", episodes, "First");
        add(&mut c, "2", "TV", "2003-01-01", Some(12), "Second");
        link(&mut c, "1", "2");
        assert_eq!(named(&drive("2", &c)), ("First".into(), "1".into()));
    }
}

/// With no series in the main story, its first entry by start names it.
#[test]
fn a_main_story_of_one_offs_is_named_after_its_first_entry() {
    let mut c = Catalogue::default();
    add(
        &mut c,
        "2357",
        "movie",
        "2007-12-01",
        Some(1),
        "Kara no Kyoukai 1",
    );
    add(
        &mut c,
        "3248",
        "movie",
        "2007-12-29",
        Some(1),
        "Kara no Kyoukai 2",
    );
    link(&mut c, "2357", "3248");
    assert_eq!(
        named(&drive("3248", &c)),
        ("Kara no Kyoukai 1".into(), "2357".into())
    );
}

/// Only the main story names the franchise: a series among the other
/// versions does not (Patlabor's TV series).
#[test]
fn a_series_among_the_versions_does_not_name_it() {
    let mut c = Catalogue::default();
    add(&mut c, "1162", "OVA", "1988-04-25", Some(7), "Patlabor");
    add(
        &mut c,
        "980",
        "movie",
        "1989-07-15",
        Some(1),
        "Patlabor the Movie",
    );
    add(
        &mut c,
        "291",
        "TV",
        "1987-10-11",
        Some(47),
        "Patlabor on Television",
    );
    link(&mut c, "1162", "980");
    c.rel("1162", "alternative_version", "291");
    assert_eq!(
        named(&drive("1162", &c)),
        ("Patlabor".into(), "1162".into())
    );
}

/// The format suffix is dropped unless another entry of the franchise,
/// any section and any case, would then read the same.
#[test]
fn the_suffix_is_dropped_unless_another_entry_would_read_the_same() {
    let mut c = Catalogue::default();
    add(&mut c, "1", "TV", "2000-01-01", Some(12), "Bleach (TV)");
    add(&mut c, "2", "TV", "2005-01-01", Some(12), "Bleach 2");
    link(&mut c, "1", "2");
    assert_eq!(named(&drive("1", &c)).0, "Bleach");
    add(
        &mut c,
        "3",
        "movie",
        "2006-01-01",
        Some(1),
        "BLEACH (Movie)",
    );
    c.rel("1", "side_story", "3");
    assert_eq!(named(&drive("1", &c)).0, "Bleach (TV)");
}

/// Every suffix Kitsu writes, in any case, is dropped; anything else in
/// brackets at the end stays, and a title that is nothing but a suffix
/// keeps it, so the name is never empty.
#[test]
fn every_format_suffix_is_dropped_in_any_case_and_nothing_else() {
    let name = |title: &str| {
        let mut c = Catalogue::default();
        add(&mut c, "1", "TV", "2000-01-01", Some(12), title);
        named(&drive("1", &c)).0
    };
    for sfx in [
        "(TV)",
        "(movie)",
        "(OVA)",
        "(Ona)",
        "(Special)",
        "(SPECIAL)",
    ] {
        assert_eq!(name(&format!("Saga {sfx}")), "Saga", "{sfx}");
    }
    assert_eq!(name("Saga (2020)"), "Saga (2020)");
    assert_eq!(name("Saga (TV) Extra"), "Saga (TV) Extra");
    assert_eq!(name("(TV)"), "(TV)");
}

/// `title` without a trailing Kitsu format suffix: the test's own
/// reading, kept apart from the code under test.
fn bare(title: &str) -> String {
    for s in ["TV", "Movie", "OVA", "ONA", "Special"] {
        let lower = title.to_lowercase();
        let tail = format!("({})", s.to_lowercase());
        if lower.ends_with(&tail) {
            return title[..title.len() - tail.len()].trim_end().to_string();
        }
    }
    title.to_string()
}

fn series(e: &FranchiseEntry) -> bool {
    matches!(e.subtype.as_deref(), Some("TV" | "OVA" | "ONA")) && e.episodes != Some(1)
}

/// A random catalogue, with some titles carrying a format suffix and
/// some entries of one episode.
fn titled() -> BoxedStrategy<(Catalogue, String)> {
    (
        catalogue(),
        prop::collection::vec((0usize..4, 0usize..7, prop::bool::ANY), 14),
    )
        .prop_map(|((mut c, anchor), looks)| {
            const NAMES: [&str; 4] = ["Bleach", "BLEACH", "Gintama", "JoJo"];
            const SUFFIX: [&str; 7] = [
                "",
                " (TV)",
                " (movie)",
                " (OVA)",
                " (Ona)",
                " (SPECIAL)",
                " (Special)",
            ];
            let ids: Vec<String> = c.entries.keys().cloned().collect();
            for (id, (n, s, one)) in ids.iter().zip(looks) {
                let e = c.entries.get_mut(id).expect("entry");
                e.title = format!("{}{}", NAMES[n], SUFFIX[s]);
                if one {
                    e.episodes = Some(1);
                }
            }
            // Relations carry the entry as it was added: refresh them.
            let entries = c.entries.clone();
            for l in c.listings.values_mut() {
                for r in &mut l.relations {
                    r.entry = entries.get(&r.id).cloned();
                }
            }
            (c, anchor)
        })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The name comes from the main story's first series by start, else
    /// its first entry.
    #[test]
    fn the_name_comes_from_the_first_series_else_the_first_entry((cat, anchor) in titled()) {
        let (f, _) = drive_with(&anchor, &cat, &[]);
        let want = f.entries.iter().find(|e| series(e)).or(f.entries.first());
        prop_assert_eq!(Some(named(&f).1), want.map(|e| e.id.clone()));
    }

    /// The name is that entry's title, or its title without the format
    /// suffix where no other entry would then read the same.
    #[test]
    fn the_name_drops_a_suffix_only_where_nothing_clashes((cat, anchor) in titled()) {
        let (f, _) = drive_with(&anchor, &cat, &[]);
        let (name, from) = named(&f);
        let all: Vec<&FranchiseEntry> = f.entries.iter()
            .chain(&f.versions).chain(&f.films).chain(&f.sides).chain(&f.recaps)
            .collect();
        let title = &all.iter().find(|e| e.id == from).expect("named entry").title;
        let clash = all.iter()
            .filter(|e| e.id != from)
            .any(|e| bare(&e.title).to_lowercase() == bare(title).to_lowercase());
        let keep = clash || bare(title).trim().is_empty();
        prop_assert_eq!(name, if keep { title.clone() } else { bare(title) });
    }
}
