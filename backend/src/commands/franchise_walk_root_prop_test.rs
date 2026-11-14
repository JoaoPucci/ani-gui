//! Properties of the main story over random franchises shaped like
//! Kitsu's: a main line of TV seasons, and other lines (films, OVAs, a
//! remake) whose entries are versions of the main line's, that fork
//! from it, join it, or both (`franchise_walk_root.rs`). Every link is
//! listed from both sides, or, in the second generator, some from one
//! side only, as Kitsu sometimes lists them.
//!
//! Every strategy returns a `BoxedStrategy`, for the Windows test
//! build's PDB limit (see `meta/kitsu_relations_prop_test.rs`).

use std::collections::BTreeSet;

use proptest::prelude::*;

use super::corpus_page_tests::page;
use super::prop_tests::drive_with;
use super::tests::Catalogue;
use super::*;

/// One other line: its format, its first year (folded into the years
/// its links leave it), its length, the main entry each of its entries
/// is a version of (as an offset into the main line), whether it forks
/// from the main entry before its first version and whether it joins
/// the main entry after its last. Kitsu dates a sequel after its
/// prequel, and so does the line.
type Line = (usize, u16, usize, Vec<usize>, bool, bool);

const FORMATS: [&str; 3] = ["movie", "OVA", "TV"];

fn line() -> BoxedStrategy<Line> {
    (
        0..FORMATS.len(),
        1985u16..2030,
        1usize..4,
        prop::collection::vec(0usize..6, 3),
        any::<bool>(),
        any::<bool>(),
    )
        .boxed()
}

/// A franchise: the catalogue and every entry of it, the main line's
/// first, every link listed from both sides.
fn franchise() -> BoxedStrategy<(Catalogue, Vec<String>)> {
    franchise_listed(Just(Vec::new()).boxed())
}

/// A franchise whose prequel and sequel links are each listed from both
/// sides, from the first side only, or from the second only.
fn one_sided_franchise() -> BoxedStrategy<(Catalogue, Vec<String>)> {
    franchise_listed(prop::collection::vec(0u8..3, 24).boxed())
}

/// A franchise, its `n`th prequel or sequel link listed from both sides
/// when `sides[n]` is 0 or absent, from the first side only when it is
/// 1, and from the second only when it is 2. Alternative-version links
/// are always listed from both.
fn franchise_listed(sides: BoxedStrategy<Vec<u8>>) -> BoxedStrategy<(Catalogue, Vec<String>)> {
    (
        2usize..6,
        1985u16..2020,
        prop::collection::vec(line(), 1..4),
        sides,
    )
        .prop_map(|(main_len, main_year, lines, sides)| {
            let mut c = Catalogue::default();
            let mut all = Vec::new();
            let main: Vec<String> = (0..main_len).map(|i| format!("{}", i + 1)).collect();
            for (i, id) in main.iter().enumerate() {
                let year = main_year + 2 * u16::try_from(i).expect("small");
                c.add(id, "TV", Some(&format!("{year}-01-01")));
                all.push(id.clone());
            }
            let mut nth = 0usize;
            let mut both = |c: &mut Catalogue, a: &str, role: &str, b: &str, back: &str| {
                let side = if role == "alternative_version" {
                    0
                } else {
                    nth += 1;
                    sides.get(nth - 1).copied().unwrap_or(0)
                };
                if side != 2 {
                    c.rel(a, role, b);
                }
                if side != 1 {
                    c.rel(b, back, a);
                }
            };
            for w in main.windows(2) {
                both(&mut c, &w[0], "sequel", &w[1], "prequel");
            }
            let year_of = |i: usize| main_year + 2 * u16::try_from(i).expect("small");
            for (l, (format, year, len, of, fork, join)) in lines.into_iter().enumerate() {
                let ids: Vec<String> = (0..len).map(|i| format!("{}", 100 * (l + 1) + i)).collect();
                let mut versions: Vec<usize> =
                    of[..len].iter().map(|v| (*v).min(main_len - 1)).collect();
                versions.sort_unstable();
                let (first, last) = (versions[0], versions[len - 1]);
                let fork = fork && first > 0;
                // A line starts after the entry it forks from, and ends
                // before the one it joins: one that cannot is not joined.
                let lo = if fork { year_of(first - 1) + 1 } else { 1985 };
                let len16 = u16::try_from(len).expect("small");
                let join = join && last + 1 < main_len && year_of(last + 1) >= lo + len16;
                let hi = if join {
                    year_of(last + 1) - len16
                } else {
                    2030
                };
                let year = lo + year % (hi - lo + 1);
                for (i, id) in ids.iter().enumerate() {
                    let y = year + u16::try_from(i).expect("small");
                    c.add(id, FORMATS[format], Some(&format!("{y}-06-01")));
                    all.push(id.clone());
                }
                for (id, v) in ids.iter().zip(&versions) {
                    both(
                        &mut c,
                        id,
                        "alternative_version",
                        &main[*v],
                        "alternative_version",
                    );
                }
                for w in ids.windows(2) {
                    both(&mut c, &w[0], "sequel", &w[1], "prequel");
                }
                if fork {
                    both(&mut c, &main[first - 1], "sequel", &ids[0], "prequel");
                }
                if join {
                    both(&mut c, &ids[len - 1], "sequel", &main[last + 1], "prequel");
                }
            }
            (c, all)
        })
        .boxed()
}

/// The entries joined to `from` by prequel and sequel links, as `from`'s
/// page can find them: the links each listing it reaches names.
fn joined(cat: &Catalogue, from: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::from([from.to_string()]);
    let mut todo = vec![from.to_string()];
    while let Some(at) = todo.pop() {
        for r in &cat.listings[&at].relations {
            if (r.role == "prequel" || r.role == "sequel") && seen.insert(r.id.clone()) {
                todo.push(r.id.clone());
            }
        }
    }
    seen
}

fn main_story(f: &Franchise) -> BTreeSet<String> {
    f.entries.iter().map(|e| e.id.clone()).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Every entry joined to the main line by prequel and sequel links
    /// answers with the same main story, name and front entry, and the
    /// same franchise page: they are the franchise's, not the page's. A
    /// line joined to it only as a version (a run of films beside the
    /// series) is a franchise of its own, as the walk reads one.
    #[test]
    fn every_page_of_a_franchise_has_one_main_story((cat, all) in franchise()) {
        let (first, _) = drive_with(&all[0], &cat, &[]);
        for id in joined(&cat, &all[0]).iter().filter(|id| **id != all[0]) {
            let (f, _) = drive_with(id, &cat, &[]);
            prop_assert_eq!(main_story(&f), main_story(&first), "from {} and from {}", id, &all[0]);
            prop_assert_eq!(&f.name, &first.name);
            prop_assert_eq!(&f.name_from, &first.name_from);
            prop_assert_eq!(page(&f), page(&first), "from {} and from {}", id, &all[0]);
        }
    }

    /// Where a link is listed on one side only, a page that cannot reach
    /// the listing that names it cannot count it, so two pages of one
    /// franchise can answer differently. What holds is the weaker claim:
    /// two pages that reach the same entries answer with the same main
    /// story, name, front entry and franchise page.
    #[test]
    fn pages_that_reach_the_same_franchise_answer_the_same((cat, all) in one_sided_franchise()) {
        let pages: Vec<(BTreeSet<String>, Franchise)> = all
            .iter()
            .map(|id| (joined(&cat, id), drive_with(id, &cat, &[]).0))
            .collect();
        for (i, (seen, f)) in pages.iter().enumerate() {
            for (other, g) in &pages[i + 1..] {
                if seen != other {
                    continue;
                }
                prop_assert_eq!(main_story(f), main_story(g), "from {} and from {}", &f.anchor, &g.anchor);
                prop_assert_eq!(&f.name, &g.name);
                prop_assert_eq!(&f.name_from, &g.name_from);
                prop_assert_eq!(page(f), page(g), "from {} and from {}", &f.anchor, &g.anchor);
            }
        }
    }
}
