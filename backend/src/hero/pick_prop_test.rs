//! The pick's properties over generated trending lists, under both
//! values of the airing preference.
//!
//! Off, the pick is the one the renderer made before the pick moved
//! here: [`before`] is that function, ported line for line from
//! `splitTrending` as it stood, and the pick must equal it for every
//! list. On, it is the same rule with the shows that have art ordered
//! airing first ([`airing_first`]). And under either value the walk
//! returned with a pick describes that pick: what it took, in which
//! slot, and what it left to the row.

use std::collections::HashSet;

use proptest::prelude::*;

use super::*;

/// The renderer's `splitTrending` before the airing preference:
/// first entries only, the first `count` with art in trending order,
/// then the first without art to fill, and the rest as the row.
fn before(shows: &[HeroShow], count: usize) -> (Vec<String>, Vec<String>) {
    split_by(shows, count, |art_ranks, _| art_ranks)
}

/// The same with the shows that have art ordered airing first, each
/// group keeping trending order.
fn airing_first(shows: &[HeroShow], count: usize) -> (Vec<String>, Vec<String>) {
    split_by(shows, count, |art_ranks, firsts| {
        let (airing, rest): (Vec<usize>, Vec<usize>) =
            art_ranks.into_iter().partition(|&i| is_airing(firsts[i]));
        airing.into_iter().chain(rest).collect()
    })
}

fn split_by(
    shows: &[HeroShow],
    count: usize,
    order: impl Fn(Vec<usize>, &[&HeroShow]) -> Vec<usize>,
) -> (Vec<String>, Vec<String>) {
    let mut seen = HashSet::new();
    let firsts: Vec<&HeroShow> = shows.iter().filter(|s| seen.insert(s.id.clone())).collect();
    let has_art = |s: &HeroShow| {
        let real = |v: &Option<String>| v.as_deref().is_some_and(|u| !u.is_empty());
        real(&s.anilist_banner)
            || s.cover_image
                .as_ref()
                .is_some_and(|c| real(&c.large) || real(&c.original) || real(&c.small))
    };
    let ranks: Vec<usize> = (0..firsts.len()).collect();
    let art_ranks: Vec<usize> = ranks
        .iter()
        .copied()
        .filter(|&i| has_art(firsts[i]))
        .collect();
    let with_art: Vec<usize> = order(art_ranks, &firsts).into_iter().take(count).collect();
    let fill: Vec<usize> = ranks
        .iter()
        .copied()
        .filter(|&i| !has_art(firsts[i]))
        .take(count.saturating_sub(with_art.len()))
        .collect();
    let taken: HashSet<usize> = with_art.iter().chain(&fill).copied().collect();
    let id = |i: &usize| firsts[*i].id.clone();
    (
        with_art.iter().chain(&fill).map(id).collect(),
        ranks
            .iter()
            .filter(|i| !taken.contains(i))
            .map(id)
            .collect(),
    )
}

fn url() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some(String::new())),
        Just(Some("https://art.example/a.jpg".to_string())),
    ]
}

fn status() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some("current".to_string())),
        Just(Some("finished".to_string())),
        Just(Some("upcoming".to_string())),
        Just(Some("tba".to_string())),
    ]
}

fn cover() -> impl Strategy<Value = Option<CoverSlots>> {
    prop_oneof![
        Just(None),
        (url(), url(), url()).prop_map(|(large, original, small)| Some(CoverSlots {
            large,
            original,
            small
        })),
    ]
}

/// Trending lists whose shows may repeat: ids come from a small pool,
/// and each entry carries its own art and status, so a repeat can
/// differ from its first entry.
fn trending() -> impl Strategy<Value = Vec<HeroShow>> {
    prop::collection::vec((0..10u8, status(), url(), cover()), 0..30).prop_map(|entries| {
        entries
            .into_iter()
            .map(|(id, status, anilist_banner, cover_image)| HeroShow {
                id: id.to_string(),
                title: format!("Show {id}"),
                status,
                anilist_banner,
                cover_image,
            })
            .collect()
    })
}

/// The walk's taken steps, by slot.
fn taken_by_slot(walk: &Walk) -> Vec<String> {
    let mut taken: Vec<(usize, String)> = walk
        .steps
        .iter()
        .filter_map(|s| match s.verdict {
            Verdict::Taken { slot, .. } => Some((slot, s.id.clone())),
            Verdict::Skipped { .. } => None,
        })
        .collect();
    taken.sort();
    taken.into_iter().map(|(_, id)| id).collect()
}

proptest! {
    #[test]
    fn off_it_is_the_renderers_pick_before_the_preference(t in trending(), n in 0usize..7) {
        let p = pick(&t, n, false);
        prop_assert_eq!((p.hero, p.row), before(&t, n));
    }

    #[test]
    fn on_it_is_the_same_rule_with_airing_shows_with_art_first(t in trending(), n in 0usize..7) {
        let p = pick(&t, n, true);
        prop_assert_eq!((p.hero, p.row), airing_first(&t, n));
    }

    #[test]
    fn on_with_nothing_airing_it_is_the_pick_off(t in trending(), n in 0usize..7) {
        let t: Vec<HeroShow> = t
            .into_iter()
            .filter(|s| !is_airing(s))
            .collect();
        let (a, b) = (pick(&t, n, true), pick(&t, n, false));
        prop_assert_eq!((a.hero, a.row), (b.hero, b.row));
    }

    #[test]
    fn either_way_every_show_is_on_home_once(t in trending(), n in 0usize..7, on in any::<bool>()) {
        let p = pick(&t, n, on);
        let all: Vec<&String> = p.hero.iter().chain(&p.row).collect();
        let distinct: HashSet<&String> = t.iter().map(|s| &s.id).collect();
        prop_assert_eq!(all.len(), distinct.len());
        prop_assert_eq!(all.into_iter().collect::<HashSet<_>>(), distinct);
    }

    #[test]
    fn the_walk_takes_exactly_the_hero_in_its_slots(t in trending(), n in 0usize..7, on in any::<bool>()) {
        let p = pick(&t, n, on);
        prop_assert_eq!(taken_by_slot(&p.walk), p.hero);
    }

    #[test]
    fn the_walk_is_the_lists_first_entries_in_order(t in trending(), n in 0usize..7, on in any::<bool>()) {
        let p = pick(&t, n, on);
        prop_assert_eq!(p.walk.airing_first, on);
        prop_assert_eq!(p.walk.count, n);
        prop_assert_eq!(p.walk.entries, t.len());
        prop_assert!(p.walk.steps.len() <= t.len());
        for (i, step) in p.walk.steps.iter().enumerate() {
            let entry = &t[i];
            prop_assert_eq!(step.rank, i + 1);
            prop_assert_eq!(&step.id, &entry.id);
            prop_assert_eq!(&step.title, &entry.title);
            prop_assert_eq!(&step.status, &entry.status);
            prop_assert_eq!(step.art, art(entry));
            prop_assert_eq!(step.airing, is_airing(entry));
            let repeat = t[..i].iter().any(|e| e.id == entry.id);
            let says_repeat = step.verdict == Verdict::Skipped { because: SkippedBecause::Repeat };
            prop_assert_eq!(says_repeat, repeat);
        }
    }

    #[test]
    fn what_the_walk_does_not_reach_is_in_the_row(t in trending(), n in 0usize..7, on in any::<bool>()) {
        let p = pick(&t, n, on);
        let reached = p.walk.steps.len();
        for (i, entry) in t.iter().enumerate().skip(reached) {
            let first = !t[..i].iter().any(|e| e.id == entry.id);
            if first {
                prop_assert!(!p.hero.contains(&entry.id));
                prop_assert!(p.row.contains(&entry.id));
            }
        }
    }

    #[test]
    fn the_preference_decides_a_step_only_when_on(t in trending(), n in 0usize..7, on in any::<bool>()) {
        let p = pick(&t, n, on);
        for step in &p.walk.steps {
            let by_airing = matches!(
                step.verdict,
                Verdict::Taken { because: TakenBecause::AiringArt, .. }
                    | Verdict::Skipped { because: SkippedBecause::NotAiring }
            );
            prop_assert_eq!(step.preference_applied, by_airing);
            if !on {
                prop_assert!(!by_airing);
            }
        }
    }

    #[test]
    fn a_show_passed_over_for_not_airing_is_one_the_pick_off_took(t in trending(), n in 0usize..7) {
        let p = pick(&t, n, true);
        let off = pick(&t, n, false);
        for step in &p.walk.steps {
            if step.verdict == (Verdict::Skipped { because: SkippedBecause::NotAiring }) {
                prop_assert!(off.hero.contains(&step.id));
                prop_assert!(!p.hero.contains(&step.id));
            }
        }
    }
}
