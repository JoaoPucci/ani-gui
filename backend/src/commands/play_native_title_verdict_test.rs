//! Property coverage for the probe head: which hits the pick
//! considers, in the provider's order, with a count and without.

use super::*;
use crate::scraper::provider::BrowseHit;
use proptest::prelude::*;

const ENTRY: [&str; 1] = ["Show 2"];

/// What a hit is to the entry "Show 2".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Admitted and not another entry: one of the five.
    Pickable,
    /// Admitted, but the entry before this one: never picked.
    AnotherEntry,
    /// Refused by the entry's titles.
    Refused,
}

const TITLES: [(&str, Kind); 8] = [
    ("Show 2", Kind::Pickable),
    ("Show 2nd Season", Kind::Pickable),
    ("Show Second Season", Kind::Pickable),
    ("Show", Kind::AnotherEntry),
    ("Show Season 1", Kind::AnotherEntry),
    ("Show Season 3", Kind::Refused),
    ("Show 3", Kind::Refused),
    ("Other Show Season 5", Kind::Refused),
];

/// The table says what the entry's titles say.
#[test]
fn the_table_reads_as_the_entry_s_titles_do() {
    let entry = EntryTitles::new(&ENTRY);
    for (title, kind) in TITLES {
        let want = match kind {
            Kind::Pickable => (true, false),
            Kind::AnotherEntry => (true, true),
            Kind::Refused => (false, false),
        };
        assert_eq!(
            (entry.admits(title), entry.another_entry(title)),
            want,
            "{title}"
        );
    }
}

fn pool() -> impl Strategy<Value = Vec<usize>> {
    proptest::collection::vec(0..TITLES.len(), 0..14)
}

fn hits(pool: &[usize]) -> Vec<BrowseHit> {
    pool.iter()
        .enumerate()
        .map(|(i, t)| BrowseHit {
            slug: format!("hit-{i}"),
            title: TITLES[*t].0.to_string(),
            kind: None,
        })
        .collect()
}

fn kind_of(hit: &BrowseHit) -> Kind {
    TITLES
        .iter()
        .find(|(t, _)| *t == hit.title)
        .expect("a table title")
        .1
}

fn position(hit: &BrowseHit) -> usize {
    hit.slug
        .strip_prefix("hit-")
        .and_then(|n| n.parse().ok())
        .expect("a pool slug")
}

proptest! {
    /// With a count, the head is the first five hits that can be
    /// picked and, before the fifth of them, the first five that
    /// cannot — so at most ten, in the provider's order, every hit
    /// among the first five positions still read, and no refused hit
    /// reported dropped.
    #[test]
    fn with_a_count_the_evidence_reaches_as_far_as_the_five(pool in pool()) {
        let all = hits(&pool);
        let (head, refused_dropped) = probe_head(all.clone(), true, EntryTitles::new(&ENTRY)).expect("never refused with a count");
        prop_assert!(!refused_dropped);
        prop_assert!(head.len() <= 2 * MAX_PROBED_CANDIDATES);
        let positions: Vec<usize> = head.iter().map(position).collect();
        prop_assert!(positions.windows(2).all(|w| w[0] < w[1]), "order kept: {:?}", positions);
        let pickable_before = |pos: usize| all[..pos].iter().filter(|h| kind_of(h) == Kind::Pickable).count();
        let unpickable_before = |pos: usize| all[..pos].iter().filter(|h| kind_of(h) != Kind::Pickable).count();
        for (pos, h) in all.iter().enumerate() {
            let kept = positions.contains(&pos);
            let want = if kind_of(h) == Kind::Pickable {
                pickable_before(pos) < MAX_PROBED_CANDIDATES
            } else {
                pickable_before(pos) < MAX_PROBED_CANDIDATES && unpickable_before(pos) < MAX_PROBED_CANDIDATES
            };
            prop_assert_eq!(kept, want, "{} at {}", h.title, pos);
            if pos < MAX_PROBED_CANDIDATES {
                prop_assert!(kept, "the old window is still read: {} at {}", h.title, pos);
            }
        }
    }

    /// Without a count, the head is the first five hits that can be
    /// picked and nothing else; a refused hit is reported dropped, and
    /// a head left empty by one is the refusal by title.
    #[test]
    fn without_a_count_only_the_five_are_read(pool in pool()) {
        let all = hits(&pool);
        let any_refused = all.iter().any(|h| kind_of(h) == Kind::Refused);
        let any_pickable = all.iter().any(|h| kind_of(h) == Kind::Pickable);
        match probe_head(all.clone(), false, EntryTitles::new(&ENTRY)) {
            Ok((head, refused_dropped)) => {
                prop_assert_eq!(refused_dropped, any_refused);
                prop_assert!(any_pickable || !any_refused, "an empty head after a refusal is the refusal by title");
                let want: Vec<String> = all
                    .iter()
                    .filter(|h| kind_of(h) == Kind::Pickable)
                    .take(MAX_PROBED_CANDIDATES)
                    .map(|h| h.slug.clone())
                    .collect();
                let got: Vec<String> = head.iter().map(|h| h.slug.clone()).collect();
                prop_assert_eq!(got, want);
            }
            Err(e) => {
                prop_assert!(!any_pickable && any_refused, "refused a pool with a hit to pick: {:?}", e);
                prop_assert!(matches!(e, crate::error::AniError::Upstream { status: 404 }), "{:?}", e);
            }
        }
    }
}
