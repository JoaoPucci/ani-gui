//! The run's ordering over the fixture franchises (ported from the
//! renderer's `order.test.ts`).

use super::by_start::sort_by_start;
use super::fixtures::{entry, ids, run, runs};
use super::graph::link_graph;
use super::place::placement;
use super::rail::{path_note, PathNote};
use super::segments::{loop_size, segments, Segment};
use super::sequence::sequence;
use super::{WatchOrder, WatchRun};

use WatchOrder::{Linked, Released};

fn next_ids(r: &WatchRun, id: &str) -> Vec<String> {
    ids(link_graph(r).after(id).iter().copied())
}

#[test]
fn by_start_puts_unknown_dates_last_and_breaks_ties_by_id() {
    let mut list = vec![
        entry("9", None),
        entry("3", Some("2020-01-01")),
        entry("2", Some("2020-01-01")),
        entry("1", Some("2019-01-01")),
        entry("8", None),
    ];
    sort_by_start(&mut list);
    assert_eq!(ids(&list), ["1", "2", "3", "8", "9"]);
}

#[test]
fn a_linear_run_numbers_every_entry_and_shows_a_total() {
    let p = placement(&run("bleach"), "48015", Linked);
    assert_eq!((p.position, p.total, p.in_cycle), (Some(4), Some(5), false));
    assert_eq!(ids(&p.before), ["46903"]);
    assert_eq!(ids(&p.after), ["49444"]);
}

#[test]
fn ignores_the_shortcut_from_enchousen_straight_to_gintama_degree() {
    let g = run("gintama");
    let graph = link_graph(&g);
    assert_eq!(ids(graph.after("7253").iter().copied()), ["7241"]);
    assert_eq!(ids(graph.before("10083").iter().copied()), ["7241"]);
}

#[test]
fn the_final_is_a_join_with_no_total_numbered_along_the_earliest_prequel() {
    let p = placement(&run("gintama"), "43683", Linked);
    assert_eq!(ids(&p.before), ["41079", "43829"]);
    assert_eq!(p.total, None);
    assert_eq!(p.position, Some(10));
}

#[test]
fn fate_zero_second_season_forks_and_entries_after_the_fork_are_unnumbered() {
    let fate = run("fate");
    assert_eq!(next_ids(&fate, "7658"), ["323", "9718"]);
    assert_eq!(placement(&fate, "7658", Linked).position, Some(2));
    assert_eq!(
        ids(&placement(&fate, "7658", Linked).after),
        ["323", "9718"]
    );
    assert_eq!(placement(&fate, "7882", Linked).position, None);
}

#[test]
fn monogataris_loop_is_a_cycle_whose_members_are_unnumbered() {
    let mono = run("monogatari");
    assert_eq!(link_graph(&mono).cycle.len(), 10);
    let p = placement(&mono, "6546", Linked);
    assert_eq!((p.position, p.in_cycle), (None, true));
    assert_eq!(placement(&mono, "3919", Linked).position, Some(1));
}

#[test]
fn the_rail_puts_bakemonogatari_first_and_the_loop_as_one_block() {
    let mono = run("monogatari");
    let seq = ids(sequence(&mono, Linked));
    assert_eq!(seq[0], "3919");
    assert_eq!(seq.last().map(String::as_str), Some("50013"));
    assert_eq!(seq.len(), 16);
}

#[test]
fn release_order_is_one_dated_line_with_neighbours_on_either_side() {
    let p = placement(&run("gintama"), "43683", Released);
    assert_eq!(ids(&p.before), ["41079"]);
    assert_eq!(ids(&p.after), ["43829"]);
    assert_eq!((p.position, p.total), (Some(10), Some(11)));
}

#[test]
fn the_first_entry_in_release_order_has_nothing_before_it() {
    assert!(placement(&run("jojo"), "7158", Released).before.is_empty());
}

#[test]
fn every_fixture_run_lists_every_member_once_in_either_order() {
    for r in runs() {
        for order in [Linked, Released] {
            let seq = ids(sequence(&r.run, order));
            let unique: std::collections::BTreeSet<_> = seq.iter().collect();
            assert_eq!(unique.len(), r.run.entries.len(), "{} {order:?}", r.key);
            assert_eq!(seq.len(), r.run.entries.len(), "{} {order:?}", r.key);
        }
    }
}

#[test]
fn link_order_keeps_every_link_of_a_linear_run() {
    let aot = run("aot");
    let kept: usize = link_graph(&aot).next.values().map(Vec::len).sum();
    assert_eq!(kept, aot.links.len());
}

#[test]
fn lays_fate_zero_out_as_a_line_then_a_fork_of_two_branches() {
    let segs = segments(&run("fate"), Linked);
    let Segment::Fork { from, branches } = &segs[1] else {
        panic!("a fork second: {segs:?}");
    };
    assert!(matches!(segs[0], Segment::Line { .. }));
    assert_eq!(segs.len(), 2);
    assert_eq!(from.id, "7658");
    let branches: Vec<Vec<String>> = branches.iter().map(ids).collect();
    assert_eq!(branches, [vec!["323"], vec!["9718", "7882", "10052"]]);
}

fn kinds(segs: &[Segment]) -> Vec<&'static str> {
    segs.iter()
        .map(|s| match s {
            Segment::Line { .. } => "line",
            Segment::Fork { .. } => "fork",
            Segment::Cycle { .. } => "cycle",
        })
        .collect()
}

#[test]
fn takes_monogataris_loop_as_one_block_with_no_empty_fork_before_it() {
    assert_eq!(
        kinds(&segments(&run("monogatari"), Linked)),
        ["line", "cycle", "line"]
    );
}

#[test]
fn segments_are_one_line_in_release_order() {
    assert_eq!(segments(&run("monogatari"), Released).len(), 1);
}

#[test]
fn does_not_number_the_semi_final_which_leads_into_the_final_from_outside_the_main_line() {
    let g = run("gintama");
    assert_eq!(placement(&g, "43829", Linked).position, None);
    let Some(PathNote::LeadsInto { entry }) = path_note(&g, "43829", Linked) else {
        panic!("a leads-into note");
    };
    assert_eq!(entry.id, "43683");
    assert_eq!(path_note(&g, "41079", Linked), None);
    assert_eq!(path_note(&g, "43829", Released), None);
}

#[test]
fn names_a_branch_head_and_a_loop_member() {
    assert_eq!(
        path_note(&run("fate"), "9718", Linked),
        Some(PathNote::Branch)
    );
    assert_eq!(path_note(&run("fate"), "7882", Linked), None);
    assert_eq!(
        path_note(&run("monogatari"), "6546", Linked),
        Some(PathNote::Cycle)
    );
}

#[test]
fn follows_monogataris_links_from_where_the_run_enters_the_loop() {
    let seq = ids(sequence(&run("monogatari"), Linked));
    assert_eq!(
        seq[1..11],
        ["6546", "7294", "7573", "8032", "9961", "11182", "11613", "5649", "11406", "11407"]
    );
}

#[test]
fn a_cycle_carries_on_from_the_earliest_unlisted_member_when_the_links_run_out() {
    let link = |a: &str, b: &str| (a.to_string(), b.to_string());
    let r = WatchRun {
        name_id: "a".into(),
        entries: vec![
            entry("a", Some("2001")),
            entry("b", Some("2002")),
            entry("c", Some("2003")),
            entry("d", Some("2004")),
        ],
        links: vec![
            link("b", "c"),
            link("c", "b"),
            link("b", "d"),
            link("d", "b"),
        ],
        ..run("bleach")
    };
    assert_eq!(ids(sequence(&r, Linked)), ["a", "b", "c", "d"]);
}

#[test]
fn loop_size_counts_the_loop_an_entry_sits_in_and_nothing_outside_one() {
    assert_eq!(loop_size(&run("monogatari"), "6546", Linked), 10);
    assert_eq!(loop_size(&run("monogatari"), "3919", Linked), 0);
}

#[test]
fn the_order_setting_reads_anything_but_released_as_linked() {
    assert_eq!(WatchOrder::of("released"), Released);
    for other in ["linked", "", "Released", "start"] {
        assert_eq!(WatchOrder::of(other), Linked);
    }
}
