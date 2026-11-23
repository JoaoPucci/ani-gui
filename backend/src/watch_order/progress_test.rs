//! The run's first entry, progress from the history, and one entry's
//! readings (ported from the renderer's `name-id.test.ts`,
//! `progress.test.ts` and `entry.test.ts`).

use proptest::prelude::*;

use super::by_start::sort_by_start;
use super::entry::{is_unaired, progress_of, progress_share, year_of, ProgressView};
use super::fixtures::{entry, run, run_with_progress, runs};
use super::progress::{history_episode, progress_from_history};
use super::run::name_id;
use super::strategies::{entries_arb, links_arb};
use super::{EntryProgress, HistoryRow, Progress, RunLink, WatchEntry};

const TODAY: &str = "2026-11-02";

#[test]
fn name_id_finds_each_fixture_franchises_first_entry() {
    // The 2004 Bleach series, the 2012 JoJo series, the 2006 Gintama
    // (not The Semi-Final, which no prequel reaches but which started
    // later), Fate/Zero, Bakemonogatari.
    for r in runs() {
        assert_eq!(
            name_id(&r.run.entries, &r.run.links),
            Some(r.run.name_id.clone()),
            "{}",
            r.key
        );
    }
}

#[test]
fn name_id_takes_the_earliest_member_of_a_run_that_is_one_loop() {
    let bleach = run("bleach");
    let (a, b) = (bleach.entries[0].clone(), bleach.entries[1].clone());
    let links = vec![(a.id.clone(), b.id.clone()), (b.id.clone(), a.id.clone())];
    assert_eq!(name_id(&[b, a.clone()], &links), Some(a.id));
}

#[test]
fn name_id_names_nothing_in_an_empty_run() {
    assert_eq!(name_id(&[], &[]), None);
}

fn sized(id: &str, episodes: Option<u32>) -> WatchEntry {
    WatchEntry {
        episodes,
        start: Some("2020-01-01".into()),
        ..entry(id, None)
    }
}

fn row(kitsu_id: Option<&str>, ep_no: &str) -> HistoryRow {
    HistoryRow {
        kitsu_id: kitsu_id.map(Into::into),
        ep_no: ep_no.into(),
    }
}

fn four() -> Vec<WatchEntry> {
    vec![
        sized("7158", Some(26)),
        sized("46598", Some(14)),
        sized("49847", None),
        sized("1", Some(1)),
    ]
}

fn progress(list: &[(&str, EntryProgress)]) -> Progress {
    list.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
}

#[test]
fn reads_watched_at_the_last_episode_and_part_way_before_it() {
    let rows = [
        row(Some("7158"), "26"),
        row(Some("46598"), "13"),
        row(Some("49847"), "1"),
        row(Some("1"), "1"),
    ];
    let want = progress(&[
        ("7158", EntryProgress::Watched),
        ("46598", EntryProgress::Episode(13.0)),
        ("49847", EntryProgress::Episode(1.0)),
        ("1", EntryProgress::Watched),
    ]);
    assert_eq!(progress_from_history(&rows, &four()), want);
}

#[test]
fn takes_the_furthest_of_several_rows_for_one_entry() {
    let rows = [row(Some("46598"), "3"), row(Some("46598"), "9")];
    assert_eq!(
        progress_from_history(&rows, &four()),
        progress(&[("46598", EntryProgress::Episode(9.0))])
    );
    let rows = [row(Some("46598"), "14"), row(Some("46598"), "2")];
    assert_eq!(
        progress_from_history(&rows, &four()),
        progress(&[("46598", EntryProgress::Watched)])
    );
}

#[test]
fn leaves_out_rows_with_no_kitsu_id_no_episode_or_an_entry_outside_the_franchise() {
    let rows = [
        row(None, "5"),
        row(Some("46598"), "movie"),
        row(Some("46598"), ""),
        row(Some("999"), "4"),
    ];
    assert!(progress_from_history(&rows, &four()).is_empty());
}

#[test]
fn reads_a_recap_and_a_range_the_way_the_history_does() {
    assert_eq!(
        progress_from_history(&[row(Some("46598"), "7.5")], &four()),
        progress(&[("46598", EntryProgress::Episode(7.5))])
    );
    assert_eq!(
        progress_from_history(&[row(Some("46598"), "1-12")], &four()),
        progress(&[("46598", EntryProgress::Episode(1.0))])
    );
}

#[test]
fn history_episode_reads_the_episode_a_row_starts_with() {
    assert_eq!(history_episode(" 12"), Some(12.0));
    assert_eq!(history_episode("007"), Some(7.0));
    assert_eq!(history_episode("7.5"), Some(7.5));
    assert_eq!(history_episode("7."), Some(7.0));
    assert_eq!(history_episode("1-12"), Some(1.0));
    for none in ["", "0", "0.0", "x", "-1", ".5"] {
        assert_eq!(history_episode(none), None, "{none:?}");
    }
}

#[test]
fn reads_progress_as_none_watched_or_part_way() {
    let (jojo, p) = run_with_progress("jojo");
    let at = |id: &str| {
        let e = jojo.all().find(|e| e.id == id).expect("an entry");
        progress_of(e, p.get(id).copied())
    };
    assert_eq!(at("610"), ProgressView::None);
    assert_eq!(at("7158"), ProgressView::Watched);
    assert_eq!(
        at("46598"),
        ProgressView::Partway {
            episode: 13.0,
            total: Some(14)
        }
    );
    assert_eq!(progress_share(at("7158")), 1.0);
    assert_eq!(progress_share(at("610")), 0.0);
    assert_eq!(
        progress_share(ProgressView::Partway {
            episode: 3.0,
            total: None
        }),
        0.0
    );
    assert_eq!(
        progress_share(ProgressView::Partway {
            episode: 7.0,
            total: Some(14)
        }),
        0.5
    );
    assert_eq!(
        progress_share(ProgressView::Partway {
            episode: 20.0,
            total: Some(14)
        }),
        1.0
    );
}

#[test]
fn knows_aired_from_unaired() {
    let mono = run("monogatari");
    let e = |id: &str| mono.all().find(|e| e.id == id).cloned().expect("an entry");
    assert!(is_unaired(&e("50013"), TODAY));
    assert!(!is_unaired(&e("3919"), TODAY));
    assert!(!is_unaired(
        &WatchEntry {
            start: Some(TODAY.into()),
            ..e("3919")
        },
        TODAY
    ));
    assert_eq!(year_of(&e("3919")).as_deref(), Some("2009"));
    assert_eq!(year_of(&e("50013")), None);
}

/// Entries and links between them.
fn scenario() -> BoxedStrategy<(Vec<WatchEntry>, Vec<RunLink>)> {
    entries_arb(1, 7, 1..=60)
        .prop_flat_map(|entries| {
            let ids = entries.iter().map(|e| e.id.clone()).collect();
            (Just(entries), links_arb(ids, 10))
        })
        .boxed()
}

fn ep_no() -> BoxedStrategy<String> {
    prop_oneof![
        (1u32..=40).prop_map(|n| n.to_string()),
        prop::sample::select(&["", "x", "0"][..]).prop_map(String::from)
    ]
    .boxed()
}

proptest! {
    #[test]
    fn name_id_is_a_member_no_prequel_reaches_the_earliest_of_them_whenever_there_is_one((entries, links) in scenario()) {
        let got = name_id(&entries, &links).expect("a first entry");
        let reached: std::collections::BTreeSet<&str> = links.iter().filter(|(a, b)| a != b).map(|(_, b)| b.as_str()).collect();
        let roots: Vec<&WatchEntry> = entries.iter().filter(|e| !reached.contains(e.id.as_str())).collect();
        let mut pool = if roots.is_empty() { entries.iter().collect() } else { roots };
        sort_by_start(&mut pool);
        prop_assert_eq!(got, pool[0].id.clone());
    }

    #[test]
    fn name_id_does_not_depend_on_the_order_entries_and_links_are_listed_in(
        ((entries, links), shuffled) in scenario().prop_flat_map(|s| {
            let shuffled = (Just(s.0.clone()).prop_shuffle(), Just(s.1.clone()).prop_shuffle());
            (Just(s), shuffled)
        })
    ) {
        prop_assert_eq!(name_id(&shuffled.0, &shuffled.1), name_id(&entries, &links));
    }

    #[test]
    fn progress_is_the_furthest_episode_per_entry_watched_once_it_reaches_a_known_count(
        (entries, rows) in entries_arb(1, 7, 1..=60).prop_flat_map(|list| {
            let mut ids: Vec<String> = list.iter().map(|e| e.id.clone()).collect();
            ids.push("999".into());
            let rows = prop::collection::vec(
                (prop::option::of(prop::sample::select(ids)), ep_no())
                    .prop_map(|(kitsu_id, ep_no)| HistoryRow { kitsu_id, ep_no }),
                0..=12,
            );
            (Just(list), rows)
        })
    ) {
        let got = progress_from_history(&rows, &entries);
        for e in &entries {
            let eps: Vec<f64> = rows
                .iter()
                .filter(|r| r.kitsu_id.as_deref() == Some(e.id.as_str()))
                .filter_map(|r| r.ep_no.parse::<f64>().ok())
                .filter(|n| *n > 0.0)
                .collect();
            if eps.is_empty() {
                prop_assert!(!got.contains_key(&e.id));
                continue;
            }
            let furthest = eps.iter().copied().fold(0.0, f64::max);
            let done = e.episodes.is_some_and(|n| furthest >= f64::from(n));
            prop_assert_eq!(got.get(&e.id), Some(&if done { EntryProgress::Watched } else { EntryProgress::Episode(furthest) }));
        }
        prop_assert!(got.keys().all(|id| entries.iter().any(|e| &e.id == id)));
    }

    #[test]
    fn progress_does_not_depend_on_the_order_of_the_rows(
        (rows, shuffled) in prop::collection::vec(
            (prop::sample::select(&["1", "2", "3"][..]), 1u32..=15)
                .prop_map(|(k, n)| HistoryRow { kitsu_id: Some(k.into()), ep_no: n.to_string() }),
            0..=10,
        ).prop_flat_map(|r| (Just(r.clone()), Just(r).prop_shuffle()))
    ) {
        let list = [sized("1", Some(12)), sized("2", None), sized("3", Some(3))];
        prop_assert_eq!(progress_from_history(&shuffled, &list), progress_from_history(&rows, &list));
    }

    #[test]
    fn progress_share_is_between_none_and_all(episode in 0.0f64..100.0, total in prop::option::of(0u32..=50)) {
        let share = progress_share(ProgressView::Partway { episode, total });
        prop_assert!((0.0..=1.0).contains(&share));
    }
}
