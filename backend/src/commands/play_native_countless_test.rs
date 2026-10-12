//! Property coverage for the countless pick: what the entry's year
//! says about a pool, and what an entry with no year may take.

use super::{pick_without_count, YearEvidence};
use crate::commands::play_native_title_grammar::normalized;
use crate::commands::play_native_title_marker::EntryTitles;
use crate::scraper::provider::BrowseHit;
use crate::scraper::table_provider::TableProvider;
use proptest::prelude::*;

const ENTRY: [&str; 2] = ["Show 2nd Season", "Show Zoku"];

/// Hit titles with whether the entry names them: the entry's own
/// titles, a division written otherwise ("Show Season 2"), and titles
/// it does not name — the season before, another show, a later season,
/// and a title carrying a subtitle the entry does not.
const TITLES: [(&str, bool); 8] = [
    ("Show 2nd Season", true),
    ("Show Zoku", true),
    ("Show Season 2", true),
    ("Show", false),
    ("Other Show", false),
    ("Show Season 3", false),
    ("Show 2nd Season: Extra Arc", false),
    ("Unrelated Thing", false),
];

fn evidence(year: Option<u32>, excluded_any: bool) -> YearEvidence {
    YearEvidence::of(year, excluded_any)
}

fn pool() -> impl Strategy<Value = Vec<usize>> {
    proptest::collection::vec(0..TITLES.len(), 1..6)
}

fn hits(pool: &[usize]) -> Vec<BrowseHit> {
    pool.iter()
        .enumerate()
        .map(|(i, t)| BrowseHit {
            slug: format!("hit-{}", i + 1),
            title: TITLES[*t].0.to_string(),
            kind: None,
        })
        .collect()
}

fn pick(
    hits: &[BrowseHit],
    needle: &str,
    year: YearEvidence,
) -> crate::error::Result<super::PickedShow> {
    pick_confirmed(hits, &vec![false; hits.len()], needle, year)
}

/// [`pick`] with each hit's own year matched or not.
fn pick_confirmed(
    hits: &[BrowseHit],
    confirmed: &[bool],
    needle: &str,
    year: YearEvidence,
) -> crate::error::Result<super::PickedShow> {
    let client = TableProvider::counted(
        &(1..=hits.len() as u64)
            .map(|id| (id, 1))
            .collect::<Vec<_>>(),
    );
    let head: Vec<(&BrowseHit, bool)> = hits.iter().zip(confirmed).map(|(h, c)| (h, *c)).collect();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(pick_without_count(
            &client,
            &head,
            &normalized(needle),
            year,
            EntryTitles::new(&ENTRY),
        ))
}

proptest! {
    /// The evidence is unknown exactly when the entry has no year,
    /// and otherwise says whether the year disproved any hit.
    #[test]
    fn the_year_s_evidence_reads_the_year_and_the_filter(year in proptest::option::of(1960u32..2030), excluded in any::<bool>()) {
        let want = match (year, excluded) {
            (None, _) => YearEvidence::Unknown,
            (Some(_), true) => YearEvidence::DisprovedSome,
            (Some(_), false) => YearEvidence::DisprovedNone,
        };
        prop_assert_eq!(evidence(year, excluded), want);
    }

    /// With no year, the pick takes a hit only when the entry names
    /// one, and then takes one it names, wherever the provider ranks
    /// it — whichever of the entry's titles was searched.
    #[test]
    fn an_entry_with_no_year_takes_only_a_hit_it_names(pool in pool(), searched in 0..ENTRY.len()) {
        let hits = hits(&pool);
        let any_named = pool.iter().any(|t| TITLES[*t].1);
        match pick(&hits, ENTRY[searched], YearEvidence::Unknown) {
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

    /// A year that disproved nothing keeps the provider's order: some
    /// hit is always taken, the searched title first when it is there.
    #[test]
    fn a_year_that_disproved_nothing_keeps_the_fallback(pool in pool(), searched in 0..ENTRY.len()) {
        let hits = hits(&pool);
        let p = pick(&hits, ENTRY[searched], YearEvidence::DisprovedNone).expect("a hit is taken");
        if hits.iter().any(|h| h.title == ENTRY[searched]) {
            prop_assert_eq!(p.hit.title.as_str(), ENTRY[searched]);
        }
    }

    /// When the year disproved part of the pool, a hit is taken only
    /// when it is the title searched or its own year matched — never
    /// on the provider's order — and one such is always taken.
    #[test]
    fn a_year_that_disproved_some_takes_only_the_searched_title_or_a_matched_year(
        pool in pool(),
        searched in 0..ENTRY.len(),
        confirmed in proptest::collection::vec(any::<bool>(), 6),
    ) {
        let hits = hits(&pool);
        let confirmed = &confirmed[..hits.len()];
        let vouched = |h: &BrowseHit, c: bool| c || h.title == ENTRY[searched];
        let any = hits.iter().zip(confirmed).any(|(h, c)| vouched(h, *c));
        match pick_confirmed(&hits, confirmed, ENTRY[searched], YearEvidence::DisprovedSome) {
            Ok(p) => {
                let i = hits.iter().position(|h| h.slug == p.hit.slug).expect("a pool hit");
                prop_assert!(vouched(&hits[i], confirmed[i]), "took {} on position", p.hit.title);
            }
            Err(_) => prop_assert!(!any, "refused a pool holding a vouched hit"),
        }
    }
}
