//! The dated-name rule: an entry with a year and a count above one
//! takes a probed candidate only when a name vouches for it — the
//! title searched, or a name of the show the entry names or shares a
//! name with: the listing's title, its card's romanized name, or a name
//! its entry page gives. Every other entry is left exactly as it was.

use super::{may_win, named_for_entry, needs_a_name, unnamed_dated};
use crate::commands::play_native_title_grammar::normalized;
use crate::commands::play_native_title_marker::EntryTitles;
use crate::commands::play_native_wide_listing::UNFIT;
use crate::commands::play_native_year::PageNames;
use crate::scraper::provider::BrowseHit;
use proptest::prelude::*;

const ENTRY: [&str; 2] = ["Show: Arc", "Shou Arc"];
const NEEDLE: &str = "show: arc";

fn hit(slug: &str, title: &str, jname: Option<&str>) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: None,
        jname: jname.map(str::to_string),
    }
}

fn pages(rows: &[(&str, &[&str])]) -> PageNames {
    rows.iter()
        .map(|(slug, names)| {
            (
                slug.to_string(),
                names.iter().map(|n| n.to_string()).collect(),
            )
        })
        .collect()
}

/// Whether `title`, known by `jname` and the page names `page`, is
/// named for an entry known as `entry` searched as `searched`.
fn named(entry: &[&str], searched: &str, title: &str, jname: Option<&str>, page: &[&str]) -> bool {
    let h = hit("x-1", title, jname);
    named_for_entry(
        &h,
        &normalized(searched),
        EntryTitles::new(entry),
        &pages(&[("x-1", page)]),
    )
}

/// Names of the show, each one the entry `["Show: Arc", "Shou Arc"]`
/// searched as "Show: Arc" is or is not vouched for by: the title
/// searched, an alias, a name sharing the entry's, and names the entry
/// carries none of.
const NAMES: [(&str, bool); 6] = [
    ("Show: Arc", true),
    ("Shou Arc", true),
    ("Show: Arc - Second Half", true),
    ("Other", false),
    ("Arc", false),
    ("Another Show: Arc", false),
];

/// One probed row: the index of its title, of its romanized name (or
/// none) and of its page name (or none), a count distance (or
/// [`UNFIT`]), and whether its year matched the entry's.
type Row = (usize, Option<usize>, Option<usize>, u32, bool);

fn rows() -> impl Strategy<Value = Vec<Row>> {
    proptest::collection::vec(
        (
            0..NAMES.len(),
            proptest::option::of(0..NAMES.len()),
            proptest::option::of(0..NAMES.len()),
            prop_oneof![0u32..8, Just(UNFIT)],
            any::<bool>(),
        ),
        0..8,
    )
}

fn hits(rows: &[Row]) -> Vec<BrowseHit> {
    rows.iter()
        .enumerate()
        .map(|(i, (t, j, _, _, _))| BrowseHit {
            slug: format!("hit-{i}"),
            title: NAMES[*t].0.to_string(),
            kind: None,
            jname: j.map(|j| NAMES[j].0.to_string()),
        })
        .collect()
}

fn page_names(rows: &[Row]) -> PageNames {
    rows.iter()
        .enumerate()
        .filter_map(|(i, (_, _, p, _, _))| {
            p.map(|p| (format!("hit-{i}"), vec![NAMES[p].0.to_string()]))
        })
        .collect()
}

proptest! {
    /// A name is needed exactly for a count above one with a year.
    #[test]
    fn a_name_is_needed_with_a_year_and_a_count_above_one(
        expected in 0u32..2000,
        year in proptest::option::of(1900u32..2100),
    ) {
        prop_assert_eq!(needs_a_name(expected, year), expected > 1 && year.is_some());
    }

    /// An undated entry, or one a single video long or counted at zero,
    /// is left exactly as it was, and nothing is reported refused: the
    /// count-alone and the single-video rules read those pools.
    #[test]
    fn an_undated_or_single_video_entry_is_untouched(
        rows in rows(),
        expected in 0u32..30,
        year in proptest::option::of(1900u32..2100),
    ) {
        let expected = if year.is_some() { expected.min(1) } else { expected };
        let hits = hits(&rows);
        let names = page_names(&rows);
        let mut probed: Vec<_> = hits.iter().zip(&rows).map(|(h, r)| (h, Vec::new(), r.3, r.4)).collect();
        let before: Vec<u32> = probed.iter().map(|r| r.2).collect();
        let refused = unnamed_dated(&mut probed, expected, NEEDLE, EntryTitles::new(&ENTRY), year, &names);
        prop_assert!(!refused);
        prop_assert_eq!(probed.iter().map(|r| r.2).collect::<Vec<_>>(), before);
    }

    /// For a dated entry with a count above one, a row keeps its
    /// distance exactly when one of its names vouches for it — its year
    /// matching the entry's vouches for nothing — or it was already
    /// out; every other row is scored out, keeping its place, listing
    /// and year; and the refusal is reported exactly when a row scored
    /// out here sat within the count tolerance.
    #[test]
    fn a_dated_counted_entry_keeps_only_a_named_candidate(
        rows in rows(),
        expected in 2u32..30,
        year in 1900u32..2100,
    ) {
        let hits = hits(&rows);
        let names = page_names(&rows);
        let mut probed: Vec<_> = hits.iter().zip(&rows).map(|(h, r)| (h, Vec::new(), r.3, r.4)).collect();
        let refused = unnamed_dated(&mut probed, expected, NEEDLE, EntryTitles::new(&ENTRY), Some(year), &names);
        let tolerance = crate::commands::play_native::ep_count_threshold(expected);
        let mut want_refused = false;
        for ((row, (t, j, p, d, confirmed)), h) in probed.iter().zip(&rows).zip(&hits) {
            prop_assert_eq!(row.0.slug.as_str(), h.slug.as_str());
            prop_assert_eq!(row.3, *confirmed);
            let vouched = [Some(*t), *j, *p].into_iter().flatten().any(|i| NAMES[i].1);
            if vouched || *d == UNFIT {
                prop_assert_eq!(row.2, *d, "{} kept", NAMES[*t].0);
            } else {
                prop_assert_eq!(row.2, UNFIT, "{} scored out", NAMES[*t].0);
                want_refused |= *d <= tolerance;
            }
        }
        prop_assert_eq!(refused, want_refused);
    }
}

/// HenSemi (TV)'s titles carry none of Medaka Box Abnormal's names,
/// which its alias "Abnormal Physiology Seminar" surfaced; nor do
/// HenSemi's carry the Sayonara Zetsubou Sensei OVA's.
#[test]
fn a_same_year_show_of_the_right_length_is_named_by_nothing() {
    let tv = [
        "HenSemi (TV)",
        "変ゼミ",
        "Hentai Seiri Seminar",
        "Hen Zemi",
        "Abnormal Physiology Seminar",
    ];
    assert!(!named(
        &tv,
        "Abnormal Physiology Seminar",
        "Medaka Box Abnormal",
        Some("Medaka Box Abnormal"),
        &["めだかボックス アブノーマル"],
    ));
    let ova = [
        "HenSemi",
        "変ゼミ",
        "Hentai Seiri Seminar",
        "Hen Semi",
        "Abnormal Physiology Seminar",
        "Hen Zemi OVA",
    ];
    assert!(!named(
        &ova,
        "Hen Semi",
        "Zan Sayonara Zetsubou Sensei Bangaichi",
        Some("Zan Sayonara Zetsubou Sensei Bangaichi"),
        &["懺・さよなら絶望先生 番外地"],
    ));
}

/// The listing named by none of the entry's titles is still the show
/// when a name its card or page carries is: JoJo's "(Uncensored)"
/// listing by its Japanese title, "My Star" by 【推しの子】.
#[test]
fn a_name_on_the_card_or_the_page_vouches() {
    let jojo = [
        "JoJo no Kimyou na Bouken (TV)",
        "ジョジョの奇妙な冒険",
        "JoJo's Bizarre Adventure (2012)",
    ];
    assert!(named(
        &jojo,
        "JoJo no Kimyou na Bouken (TV)",
        "JoJo's Bizarre Adventure (Uncensored)",
        None,
        &["ジョジョの奇妙な冒険"],
    ));
    let oshi = ["[Oshi no Ko]", "【推しの子】"];
    assert!(named(
        &oshi,
        "[Oshi no Ko]",
        "My Star",
        Some("Oshi no Ko"),
        &[]
    ));
    assert!(named(
        &oshi,
        "【推しの子】",
        "My Star",
        None,
        &["【推しの子】"]
    ));
    assert!(!named(&oshi, "[Oshi no Ko]", "My Star", None, &[]));
}

/// A name the entry's title shares — the provider's subtitle or the
/// story part it numbers mid-title aside — vouches for the show:
/// Gintama.'s second Silver Soul half, and Stone Ocean Part 2 under
/// the provider's "Part 6".
#[test]
fn a_shared_name_vouches_for_the_show() {
    let gintama = [
        "Gintama.: Shirogane no Tamashii-hen 2",
        "銀魂. 銀ノ魂篇 2",
        "Gintama.: Silver Soul Arc 2",
    ];
    assert!(named(
        &gintama,
        "Gintama.: Shirogane no Tamashii-hen 2",
        "Gintama.: Silver Soul Arc - Second Half War",
        None,
        &[],
    ));
    let stone_ocean = [
        "JoJo no Kimyou na Bouken: Stone Ocean Part 2",
        "JoJo's Bizarre Adventure: Stone Ocean Part 2",
    ];
    assert!(named(
        &stone_ocean,
        "JoJo no Kimyou na Bouken: Stone Ocean Part 2",
        "JoJo's Bizarre Adventure Part 6: Stone Ocean Part 2",
        None,
        &[],
    ));
}

/// A year is no name: a candidate whose own page names the entry's
/// year, and that no name vouches for, is scored out all the same.
#[test]
fn a_matched_year_vouches_for_nothing() {
    let h = hit("other-1", "Other", Some("Other"));
    let mut probed = vec![(&h, Vec::new(), 0, true)];
    let refused = unnamed_dated(
        &mut probed,
        12,
        NEEDLE,
        EntryTitles::new(&ENTRY),
        Some(2020),
        &PageNames::new(),
    );
    assert!(refused);
    assert_eq!(probed[0].2, UNFIT);
}

/// A title that is nothing but a division has a name of no words,
/// which would start every title: it vouches for nothing.
#[test]
fn a_name_of_no_words_vouches_for_nothing() {
    assert!(!named(&["Season 2"], "season 2 x", "Other Show", None, &[]));
    assert!(!named(
        &["Show"],
        "show x",
        "Other",
        Some("Season 2"),
        &["第2期"]
    ));
}

/// A candidate the rule rules out cannot win; one it does not read, or
/// one a name vouches for, can.
#[test]
fn only_a_dated_counted_entry_s_unnamed_candidate_cannot_win() {
    let entry = EntryTitles::new(&ENTRY);
    let other = hit("other-1", "Other", None);
    let named = hit("shou-2", "Other", Some("Shou Arc"));
    let none = PageNames::new();
    assert!(!may_win(&other, 12, NEEDLE, entry, Some(2020), &none));
    assert!(may_win(&named, 12, NEEDLE, entry, Some(2020), &none));
    assert!(may_win(&other, 12, NEEDLE, entry, None, &none));
    assert!(may_win(&other, 1, NEEDLE, entry, Some(2020), &none));
}
