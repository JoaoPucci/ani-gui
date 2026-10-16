//! Property coverage for the countless pick: what vouches for a hit
//! when no count can — the title searched, the hit's own year, or the
//! entry's name for it — whatever the entry's year said of the rest of
//! the pool.

use crate::commands::play_native::{pick_candidate_titled, PickedShow};
use crate::scraper::provider::BrowseHit;
use crate::scraper::table_provider::TableProvider;
use proptest::prelude::*;

const ENTRY: [&str; 2] = ["Show 2nd Season", "Show Zoku"];
const YEAR: u32 = 2021;

/// Hit titles the pick considers, with whether the entry names them:
/// the entry's own titles, a division written otherwise ("Show Season
/// 2"), and titles it does not name — another show, a title carrying a
/// subtitle the entry does not, and an unrelated one. Every title here
/// is admitted and is not another entry of the same show, so each
/// reaches the pick.
const TITLES: [(&str, bool); 6] = [
    ("Show 2nd Season", true),
    ("Show Zoku", true),
    ("Show Season 2", true),
    ("Other Show", false),
    ("Show 2nd Season: Extra Arc", false),
    ("Unrelated Thing", false),
];

/// What a hit's entry page names as its year: the entry's own, a year
/// off either way (the December allowance), no year, or one the year
/// filter excludes.
const PAGE_YEARS: [Option<u32>; 5] = [Some(YEAR), Some(YEAR - 1), Some(YEAR + 1), None, Some(1999)];

fn pool() -> impl Strategy<Value = Vec<(usize, usize)>> {
    proptest::collection::vec((0..TITLES.len(), 0..PAGE_YEARS.len()), 1..6)
}

fn hits(pool: &[(usize, usize)]) -> Vec<BrowseHit> {
    pool.iter()
        .enumerate()
        .map(|(i, (t, _))| BrowseHit {
            slug: format!("hit-{}", i + 1),
            title: TITLES[*t].0.to_string(),
            kind: None,
            jname: None,
        })
        .collect()
}

/// The countless pick over `pool`, searched as `ENTRY[searched]`, for
/// an entry premiering in `entry_year`.
fn pick(
    pool: &[(usize, usize)],
    searched: usize,
    entry_year: Option<u32>,
) -> crate::error::Result<PickedShow> {
    let shows: Vec<(u64, u32, Option<u32>)> = pool
        .iter()
        .enumerate()
        .map(|(i, (_, y))| (i as u64 + 1, 1, PAGE_YEARS[*y]))
        .collect();
    let client = TableProvider::counted_with_years(&shows);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(pick_candidate_titled(
            &client,
            &hits(pool),
            None,
            ENTRY[searched],
            &ENTRY,
            entry_year,
            None,
        ))
}

/// Whether the year filter keeps a hit whose page names `year`.
fn survives(year: Option<u32>) -> bool {
    year.is_none_or(|y| y.abs_diff(YEAR) <= 1)
}

/// Whether something vouches for a hit: it is the title searched, its
/// own year is the entry's, or the entry names it.
fn vouched(title: usize, year: Option<u32>, searched: usize) -> bool {
    TITLES[title].0 == ENTRY[searched] || year == Some(YEAR) || TITLES[title].1
}

proptest! {
    /// With a year, a hit is taken only when something vouches for it
    /// — the title searched, its own year, or the entry's name — and
    /// whenever the year filter leaves such a hit, one is taken.
    /// Whether the year disproved some other hit changes nothing.
    #[test]
    fn a_hit_is_taken_only_when_something_vouches_for_it(pool in pool(), searched in 0..ENTRY.len()) {
        let any = pool.iter().any(|(t, y)| survives(PAGE_YEARS[*y]) && vouched(*t, PAGE_YEARS[*y], searched));
        match pick(&pool, searched, Some(YEAR)) {
            Ok(p) => {
                let i = p.hit.slug.strip_prefix("hit-").and_then(|n| n.parse::<usize>().ok()).expect("a pool slug") - 1;
                let (t, y) = pool[i];
                prop_assert!(survives(PAGE_YEARS[y]), "took {} from {:?}", p.hit.title, PAGE_YEARS[y]);
                prop_assert!(vouched(t, PAGE_YEARS[y], searched), "took {} on position", p.hit.title);
            }
            Err(e) => {
                prop_assert!(!any, "refused a pool holding a vouched hit: {e:?}");
                prop_assert!(matches!(e, crate::error::AniError::NoResults), "{e:?}");
            }
        }
    }

    /// With no year, the pick takes a hit only when the entry names
    /// one, and then takes one it names, wherever the provider ranks
    /// it — whichever of the entry's titles was searched.
    #[test]
    fn an_entry_with_no_year_takes_only_a_hit_it_names(pool in pool(), searched in 0..ENTRY.len()) {
        let any_named = pool.iter().any(|(t, _)| TITLES[*t].1);
        match pick(&pool, searched, None) {
            Ok(p) => {
                let t = TITLES.iter().find(|(t, _)| *t == p.hit.title).expect("a pool title");
                prop_assert!(t.1, "took {} which the entry does not name", p.hit.title);
            }
            Err(e) => {
                prop_assert!(!any_named, "refused a pool holding a hit the entry names");
                prop_assert!(matches!(e, crate::error::AniError::NoResults));
            }
        }
    }

    /// The title searched comes first: wherever the pool holds it with
    /// a year the filter keeps, it is the pick, whatever else is there.
    #[test]
    fn the_searched_title_is_taken_first(pool in pool(), searched in 0..ENTRY.len()) {
        let exact = pool.iter().any(|(t, y)| TITLES[*t].0 == ENTRY[searched] && survives(PAGE_YEARS[*y]));
        if exact {
            let p = pick(&pool, searched, Some(YEAR)).expect("the searched title is taken");
            prop_assert_eq!(p.hit.title.as_str(), ENTRY[searched]);
        }
    }
}
