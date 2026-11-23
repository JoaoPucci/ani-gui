//! The run as the strip reads it, over the fixture franchises (ported
//! from the renderer's `strip-model.test.ts` and `strip-view.test.ts`).

use super::fixtures::{ids, payload, run, run_with_progress};
use super::place::placement;
use super::run::run_of;
use super::strip_view::{
    current_in, franchise_summary, strip_for, strip_view, watching_for, StripKind,
};
use super::{WatchEntry, WatchOrder, WatchRun};

use WatchOrder::{Linked, Released};

fn link(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

#[test]
fn run_of_finds_the_first_entry_and_keeps_the_sections() {
    let got = run_of(&payload("jojo", "46598")).expect("a run");
    let jojo = run("jojo");
    assert_eq!(got.name_id, "7158");
    assert_eq!(ids(&got.entries), ids(&jojo.entries));
    assert_eq!(got.links, jojo.links);
    assert_eq!(ids(&got.versions), ids(&jojo.versions));
}

#[test]
fn run_of_keeps_only_links_and_parents_that_name_members_once_each_and_no_self_link() {
    let mut p = payload("bleach", "244");
    let original = p.links.clone();
    p.links
        .extend([link("244", "244"), link("244", "1"), original[0].clone()]);
    p.parents = [
        ("684".to_string(), vec!["244".to_string(), "1".to_string()]),
        ("739".to_string(), vec!["1".to_string()]),
    ]
    .into_iter()
    .collect();
    let got = run_of(&p).expect("a run");
    assert_eq!(got.links, original);
    assert_eq!(
        got.parents,
        [("684".to_string(), vec!["244".to_string()])]
            .into_iter()
            .collect()
    );
}

#[test]
fn run_of_is_none_for_an_empty_run() {
    let mut p = payload("bleach", "244");
    p.entries.clear();
    assert_eq!(run_of(&p), None);
}

#[test]
fn strip_for_fronts_the_poster_stack_with_the_entry_the_walk_names_the_franchise_after() {
    let mut f = payload("fate", "6028");
    f.name_from = "323".into();
    assert_eq!(strip_for(&f, "6028", Linked).expect("a strip").front, "323");
}

#[test]
fn strip_for_carries_whether_the_franchise_was_read_whole() {
    assert!(
        strip_for(&payload("bleach", "48015"), "48015", Linked)
            .expect("a strip")
            .complete
    );
    let mut part = payload("bleach", "48015");
    part.complete = false;
    assert!(!strip_for(&part, "48015", Linked).expect("a strip").complete);
}

#[test]
fn strip_for_places_a_member_of_a_straight_run_with_the_runs_length() {
    let v = strip_for(&payload("bleach", "48015"), "48015", Linked)
        .expect("a strip")
        .view;
    assert_eq!(
        (v.kind, v.position, v.total),
        (StripKind::Run, Some(4), Some(5))
    );
    assert_eq!(ids(&v.before), ["46903"]);
    assert_eq!(ids(&v.after), ["49444"]);
}

#[test]
fn strip_for_shows_both_ways_on_from_a_fork() {
    let v = strip_for(&payload("fate", "7658"), "7658", Linked)
        .expect("a strip")
        .view;
    assert_eq!(ids(&v.after), ["323", "9718"]);
    assert_eq!(v.total, None);
}

#[test]
fn strip_for_shows_a_side_storys_parent_stories() {
    let v = strip_for(&payload("fate", "6928"), "6928", Linked)
        .expect("a strip")
        .view;
    assert_eq!(v.kind, StripKind::Side);
    assert_eq!(ids(&v.before), ["6028", "7658"]);
    assert!(v.after.is_empty());
}

#[test]
fn strip_for_follows_the_order_setting() {
    let p = payload("monogatari", "6546");
    let released = strip_for(&p, "6546", Released).expect("a strip").view;
    assert_eq!((released.position, released.total), (Some(2), Some(16)));
    assert_eq!(
        strip_for(&p, "6546", Linked).expect("a strip").view.total,
        None
    );
}

#[test]
fn strip_for_shows_no_strip_for_a_payload_that_answers_another_id() {
    assert_eq!(strip_for(&payload("bleach", "244"), "48015", Linked), None);
}

#[test]
fn strip_for_shows_no_strip_for_an_entry_with_no_run_and_no_parent_story() {
    let film = run("bleach").films[0].clone();
    let mut alone = payload("bleach", &film.id);
    alone.entries = vec![film.clone()];
    alone.links.clear();
    alone.parents.clear();
    for order in [Linked, Released] {
        assert_eq!(strip_for(&alone, &film.id, order), None);
    }
}

#[test]
fn strip_for_shows_no_strip_for_an_entry_around_the_run_with_no_parent_story() {
    let film = run("bleach").films[0].id.clone();
    assert_eq!(strip_for(&payload("bleach", &film), &film, Linked), None);
}

#[test]
fn strip_for_places_a_member_as_the_runs_ordering_does() {
    let g = run("gintama");
    for e in &g.entries {
        let v = strip_for(&payload("gintama", &e.id), &e.id, Linked)
            .expect("a strip")
            .view;
        let p = placement(&g, &e.id, Linked);
        assert_eq!(v.kind, StripKind::Run);
        assert_eq!(
            (v.position, v.total, v.in_cycle),
            (p.position, p.total, p.in_cycle)
        );
        assert_eq!((v.before, v.after), (p.before, p.after));
    }
}

#[test]
fn strip_view_is_a_members_placement_in_its_run() {
    for order in [Linked, Released] {
        let v = strip_view(&run("fate"), "7658", order);
        let p = placement(&run("fate"), "7658", order);
        assert_eq!(
            (v.position, v.total, v.before, v.after),
            (p.position, p.total, p.before, p.after)
        );
    }
}

#[test]
fn strip_view_shows_a_side_storys_parent_stories_before_it_nothing_after_and_no_number() {
    for order in [Linked, Released] {
        let v = strip_view(&run("fate"), "6928", order);
        assert_eq!(v.kind, StripKind::Side);
        assert_eq!(ids(&v.before), ["6028", "7658"]);
        assert!(v.after.is_empty());
        assert_eq!((v.position, v.total, v.in_cycle), (None, None, false));
    }
    assert_eq!(
        ids(&strip_view(&run("gintama"), "11759", Linked).before),
        ["10083"]
    );
}

#[test]
fn franchise_summary_counts_the_main_story_films_with_other_versions_and_side_stories() {
    let fate = franchise_summary(&run("fate"), &run("fate").name_id);
    assert_eq!((fate.main, fate.films, fate.sides), (6, 3, 1));
    let bleach = franchise_summary(&run("bleach"), &run("bleach").name_id);
    assert_eq!((bleach.main, bleach.films, bleach.sides), (5, 4, 3));
}

#[test]
fn franchise_summary_totals_every_entry_the_franchise_page_lists_recaps_included() {
    // JoJo: 9 in the run, 4 other versions, 1 side story, 1 recap.
    assert_eq!(
        franchise_summary(&run("jojo"), &run("jojo").name_id).total,
        15
    );
    assert_eq!(
        franchise_summary(&run("fate"), &run("fate").name_id).total,
        12
    );
    assert_eq!(
        franchise_summary(&run("jojo"), &run("jojo").name_id).recaps,
        1
    );
    assert_eq!(
        franchise_summary(&run("fate"), &run("fate").name_id).recaps,
        2
    );
}

#[test]
fn franchise_summary_fans_the_front_entry_a_film_and_a_side_story_filling_from_the_main_story() {
    assert_eq!(
        ids(&franchise_summary(&run("fate"), &run("fate").name_id).posters),
        ["6028", "4788", "6928"]
    );
    let mono = run("monogatari");
    assert_eq!(
        ids(&franchise_summary(&mono, &mono.name_id).posters),
        ["3919", "49128", "6546"]
    );
}

#[test]
fn franchise_summary_fans_the_entry_the_franchise_is_named_after_first() {
    // Fate is named after Fate/stay night (323), not Fate/Zero (6028).
    assert_eq!(
        ids(&franchise_summary(&run("fate"), "323").posters),
        ["323", "4788", "6928"]
    );
    let jojo = run("jojo");
    let last = jojo.entries.last().expect("an entry").id.clone();
    assert_eq!(franchise_summary(&jojo, &last).posters[0].id, last);
}

#[test]
fn franchise_summary_fronts_the_first_entry_when_the_front_entry_is_not_in_the_run() {
    let fate = run("fate");
    assert_eq!(
        franchise_summary(&fate, "999").posters[0].id,
        fate.entries[0].id
    );
}

#[test]
fn franchise_summary_fans_only_entries_that_have_a_poster() {
    let fate = run("fate");
    let blank = |list: &[WatchEntry], all: bool| -> Vec<WatchEntry> {
        list.iter()
            .map(|e| {
                let mut e = e.clone();
                if all || e.id == fate.name_id {
                    e.poster = None;
                }
                e
            })
            .collect()
    };
    let no_first = WatchRun {
        entries: blank(&fate.entries, false),
        ..fate.clone()
    };
    assert_eq!(
        ids(&franchise_summary(&no_first, &fate.name_id).posters),
        ["4788", "6928", "7658"]
    );
    let none = WatchRun {
        entries: blank(&fate.entries, true),
        versions: blank(&fate.versions, true),
        films: blank(&fate.films, true),
        sides: blank(&fate.sides, true),
        recaps: blank(&fate.recaps, true),
        ..fate.clone()
    };
    assert!(franchise_summary(&none, &fate.name_id).posters.is_empty());
}

#[test]
fn release_order_puts_every_monogatari_entry_on_one_numbered_line() {
    let mono = run("monogatari");
    let mut positions: Vec<usize> = mono
        .entries
        .iter()
        .map(|e| {
            let v = strip_view(&mono, &e.id, Released);
            assert_eq!(v.total, Some(16));
            assert!(!v.in_cycle && v.before.len() <= 1 && v.after.len() <= 1);
            v.position.expect("numbered")
        })
        .collect();
    positions.sort_unstable();
    assert_eq!(positions, (1..=16).collect::<Vec<_>>());
    let nise = strip_view(&mono, "6546", Released);
    assert_eq!((nise.position, nise.total), (Some(2), Some(16)));
    for e in &mono.entries {
        assert_eq!(strip_view(&mono, &e.id, Linked).total, None);
    }
}

#[test]
fn current_in_names_the_entry_the_viewer_is_on() {
    let (jojo, progress) = run_with_progress("jojo");
    let now = current_in(&jojo, &progress, Linked).expect("watching");
    assert_eq!((now.entry.id.as_str(), now.episode), ("49847", 1.0));
    let (bleach, progress) = run_with_progress("bleach");
    assert_eq!(current_in(&bleach, &progress, Linked), None);
}

fn watching(key: &str, id: &str) -> Option<(String, f64)> {
    let (r, progress) = run_with_progress(key);
    let view = strip_view(&r, id, Linked);
    watching_for(&r, &progress, id, &view, Linked).map(|w| (w.entry.id, w.episode))
}

#[test]
fn watching_for_names_the_entry_the_viewer_is_part_way_through_elsewhere_in_the_run() {
    // On Golden Wind, the viewer is on Steel Ball Run, episode 1.
    assert_eq!(watching("jojo", "41410"), Some(("49847".into(), 1.0)));
    assert_eq!(watching("gintama", "43683"), Some(("818".into(), 6.0)));
}

#[test]
fn watching_for_says_nothing_on_that_entry_or_where_a_tile_already_shows_it_or_nothing_is_part_way()
{
    assert_eq!(watching("jojo", "49847"), None);
    // Steel Ball Run is Stone Ocean Part 3's sequel tile.
    assert_eq!(watching("jojo", "46598"), None);
    // Fate/Zero 2nd Season is one of the side story's parent tiles.
    assert_eq!(watching("fate", "6928"), None);
    assert_eq!(watching("bleach", "244"), None);
}
