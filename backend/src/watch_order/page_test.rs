//! The show page's reading of the watch order (ported from the
//! renderer's `page-model.test.ts`).

use std::collections::BTreeSet;

use proptest::prelude::*;

use super::fixtures::{ids, payload, run};
use super::franchise_link::franchise_link_only;
use super::franchise_page::franchise_page;
use super::page_model::{caption_of, watch_order_page, Eyebrow, StripCaption};
use super::strategies::{franchise_arb, franchise_around_arb, rows_arb};
use super::strip_view::{strip_for, StripKind, StripView};
use super::{EntryProgress, Franchise, HistoryRow, WatchEntry, WatchOrder};

use WatchOrder::{Linked, Released};

fn row(kitsu_id: Option<&str>, ep_no: &str) -> HistoryRow {
    HistoryRow {
        kitsu_id: kitsu_id.map(Into::into),
        ep_no: ep_no.into(),
    }
}

fn page(
    key: &str,
    anchor: &str,
    rows: &[HistoryRow],
    order: WatchOrder,
) -> Option<super::page_model::WatchOrderPage> {
    watch_order_page(&payload(key, anchor), rows, anchor, order)
}

#[test]
fn shows_no_strip_for_an_entry_with_no_run_and_no_parent_story() {
    let mut alone = payload("bleach", "244");
    alone.entries.truncate(1);
    alone.links.clear();
    assert_eq!(watch_order_page(&alone, &[], "244", Linked), None);
}

#[test]
fn places_a_member_of_a_straight_run_in_the_caption_and_the_eyebrow() {
    let got = page("bleach", "48015", &[], Linked).expect("a strip");
    assert_eq!(got.order, Linked);
    assert_eq!(
        got.caption,
        StripCaption::Position {
            position: 4,
            total: 5
        }
    );
    assert_eq!(
        got.eyebrow,
        Some(Eyebrow {
            position: 4,
            total: 5
        })
    );
    assert_eq!(ids(&got.view.before), ["46903"]);
    assert_eq!(ids(&got.view.after), ["49444"]);
}

#[test]
fn numbers_a_loop_member_nowhere_and_says_the_size_of_its_loop() {
    let mono = run("monogatari");
    let looped = mono
        .entries
        .iter()
        .find(|e| page("monogatari", &e.id, &[], Linked).is_some_and(|p| p.view.in_cycle))
        .expect("a loop member");
    let got = page("monogatari", &looped.id, &[], Linked).expect("a strip");
    assert_eq!(got.caption, StripCaption::Cycle { count: 10 });
    assert_eq!(got.eyebrow, None);
}

#[test]
fn gives_every_monogatari_entry_its_place_of_sixteen_in_release_order() {
    let got = page("monogatari", "6546", &[], Released).expect("a strip");
    assert_eq!(
        got.caption,
        StripCaption::Position {
            position: 2,
            total: 16
        }
    );
    assert_eq!(
        got.eyebrow,
        Some(Eyebrow {
            position: 2,
            total: 16
        })
    );
}

#[test]
fn names_a_side_storys_place_with_no_number_in_the_eyebrow() {
    let got = page("fate", "6928", &[], Linked).expect("a strip");
    assert_eq!(got.caption, StripCaption::Side);
    assert_eq!(got.eyebrow, None);
    assert_eq!(ids(&got.view.before), ["6028", "7658"]);
}

#[test]
fn calls_a_side_story_the_payload_lists_under_films_a_film() {
    // The Fate/Zero special, served as a film: what the walk does for
    // any movie that names a parent story.
    let mut p = payload("fate", "6928");
    let mut special = p
        .sides
        .iter()
        .find(|e| e.id == "6928")
        .expect("the special")
        .clone();
    p.sides.retain(|e| e.id != "6928");
    special.subtype = Some("movie".into());
    p.films.push(special);
    let got = watch_order_page(&p, &[], "6928", Linked).expect("a strip");
    assert_eq!(got.caption, StripCaption::Film);
    assert_eq!(got.eyebrow, None);
    assert_eq!(ids(&got.view.before), ["6028", "7658"]);
}

#[test]
fn reads_progress_from_the_history_for_the_run_only() {
    let rows = [
        row(Some("244"), "366"),
        row(Some("43078"), "4"),
        row(Some("99999"), "2"),
        row(None, "5"),
    ];
    let got = page("bleach", "48015", &rows, Linked).expect("a strip");
    let want = [
        ("244".to_string(), EntryProgress::Watched),
        ("43078".to_string(), EntryProgress::Episode(4.0)),
    ];
    assert_eq!(got.progress, want.into_iter().collect());
}

#[test]
fn names_the_entry_being_watched_elsewhere_in_the_run_which_similar_titles_leaves_out() {
    // On Golden Wind, the viewer is on Steel Ball Run, episode 1.
    let rows = [row(Some("49847"), "1")];
    let got = page("jojo", "41410", &rows, Linked).expect("a strip");
    let w = got.watching.expect("watching");
    assert_eq!((w.entry.id.as_str(), w.episode), ("49847", 1.0));
    assert!(got.similar_hidden.contains("49847"));
    // Steel Ball Run is Stone Ocean Part 3's sequel tile.
    assert_eq!(
        page("jojo", "46598", &rows, Linked)
            .expect("a strip")
            .watching,
        None
    );
}

#[test]
fn leaves_the_whole_franchise_out_of_similar_titles_not_only_the_tiles() {
    let b = run("bleach");
    let got = page("bleach", "48015", &[], Linked).expect("a strip");
    let want: BTreeSet<String> = b
        .all()
        .map(|e| e.id.clone())
        .filter(|x| x != "48015")
        .collect();
    assert_eq!(got.similar_hidden, want);
    assert!(got.similar_hidden.contains("244"));
}

#[test]
fn changes_the_neighbours_with_the_order_where_release_dates_disagree_with_the_links() {
    let linked = page("fate", "6028", &[], Linked).expect("a strip");
    let released = page("fate", "6028", &[], Released).expect("a strip");
    assert_ne!(ids(&linked.view.before), ids(&released.view.before));
}

fn one_main(complete: bool) -> Franchise {
    let mut f = payload("bleach", "244");
    f.entries.truncate(1);
    f.links.clear();
    f.complete = complete;
    f
}

fn alone(complete: bool) -> Franchise {
    Franchise {
        versions: Vec::new(),
        films: Vec::new(),
        sides: Vec::new(),
        recaps: Vec::new(),
        ..one_main(complete)
    }
}

#[test]
fn links_a_show_that_is_its_run_alone_to_the_franchise_page_that_lists_its_films() {
    let link = franchise_link_only(&one_main(true), "244").expect("a link");
    assert!(link.complete);
    assert_eq!(ids(&link.run.entries), ["244"]);
    let b = run("bleach");
    let want: BTreeSet<String> = b
        .films
        .iter()
        .chain(&b.sides)
        .chain(&b.recaps)
        .map(|e| e.id.clone())
        .collect();
    assert_eq!(link.similar_hidden, want);
}

#[test]
fn the_lone_link_fronts_its_stack_with_the_entry_the_walk_names_the_franchise_after() {
    let film = run("bleach").films[0].id.clone();
    let mut f = payload("bleach", &film);
    f.name_from = "46903".into();
    assert_eq!(
        franchise_link_only(&f, &film).expect("a link").front,
        "46903"
    );
}

#[test]
fn links_there_from_a_franchise_read_in_part_too_carrying_that_it_was() {
    assert!(
        !franchise_link_only(&one_main(false), "244")
            .expect("a link")
            .complete
    );
}

#[test]
fn gives_no_link_for_a_show_that_is_the_franchise_alone_read_whole_or_in_part() {
    assert_eq!(franchise_link_only(&alone(true), "244"), None);
    assert_eq!(franchise_link_only(&alone(false), "244"), None);
}

#[test]
fn links_a_films_page_outside_the_run_to_the_franchise_page_that_lists_the_run() {
    let b = run("bleach");
    let film = b.films[0].id.clone();
    let link = franchise_link_only(&payload("bleach", &film), &film).expect("a link");
    assert_eq!(
        watch_order_page(&payload("bleach", &film), &[], &film, Linked),
        None
    );
    assert!(!link.similar_hidden.contains(&film));
    assert!(b
        .entries
        .iter()
        .all(|e| link.similar_hidden.contains(&e.id)));
}

#[test]
fn gives_no_link_alone_where_the_strip_shows_or_for_another_show() {
    assert_eq!(franchise_link_only(&payload("fate", "6028"), "6028"), None);
    assert_eq!(franchise_link_only(&one_main(true), "43078"), None);
}

fn view() -> StripView {
    StripView {
        kind: StripKind::Run,
        position: None,
        total: None,
        before: Vec::new(),
        after: Vec::new(),
        in_cycle: false,
    }
}

#[test]
fn caption_reads_a_numbered_place_with_or_without_a_total() {
    let numbered = StripView {
        position: Some(3),
        total: Some(9),
        ..view()
    };
    assert_eq!(
        caption_of(&numbered, 0, true, false),
        StripCaption::Position {
            position: 3,
            total: 9
        }
    );
    let open = StripView {
        position: Some(3),
        ..view()
    };
    assert_eq!(
        caption_of(&open, 0, true, false),
        StripCaption::Numeral { position: 3 }
    );
}

#[test]
fn caption_reads_no_number_for_a_franchise_read_in_part() {
    let numbered = StripView {
        position: Some(3),
        total: Some(9),
        ..view()
    };
    assert_eq!(
        caption_of(&numbered, 0, false, false),
        StripCaption::Partial
    );
    let open = StripView {
        position: Some(3),
        ..view()
    };
    assert_eq!(caption_of(&open, 0, false, false), StripCaption::Partial);
    // A cut walk can leave the show off any fork it really sits on, and
    // leave a loop's count short.
    assert_eq!(caption_of(&view(), 0, false, false), StripCaption::Partial);
    let looped = StripView {
        in_cycle: true,
        ..view()
    };
    assert_eq!(caption_of(&looped, 10, false, false), StripCaption::Partial);
    let side = StripView {
        kind: StripKind::Side,
        ..view()
    };
    assert_eq!(caption_of(&side, 0, false, false), StripCaption::Side);
}

#[test]
fn caption_reads_an_unnumbered_place_as_a_loop_or_a_branch_and_a_side_story_as_one() {
    let looped = StripView {
        in_cycle: true,
        ..view()
    };
    assert_eq!(
        caption_of(&looped, 10, true, false),
        StripCaption::Cycle { count: 10 }
    );
    assert_eq!(caption_of(&view(), 0, true, false), StripCaption::Branch);
    let side = StripView {
        kind: StripKind::Side,
        ..view()
    };
    assert_eq!(caption_of(&side, 0, true, false), StripCaption::Side);
}

/// A payload, an id on it, an order and some rows.
fn any_page() -> BoxedStrategy<(Franchise, String, WatchOrder, Vec<HistoryRow>)> {
    franchise_arb()
        .prop_flat_map(|f| {
            let mut ids: Vec<String> = f.entries.iter().map(|e| e.id.clone()).collect();
            let rows = rows_arb(ids.clone());
            ids.push(f.anchor.clone());
            (
                Just(f),
                prop::sample::select(ids),
                prop::sample::select(&[Linked, Released][..]),
                rows,
            )
        })
        .boxed()
}

/// A payload around a side story `999`, listed under films or sides, and
/// a member of its run.
fn side_page() -> BoxedStrategy<(Franchise, Vec<String>, String, WatchOrder, bool)> {
    franchise_arb()
        .prop_filter("a run", |f| !f.entries.is_empty())
        .prop_flat_map(|f| {
            let ids: Vec<String> = f.entries.iter().map(|e| e.id.clone()).collect();
            (
                Just(f),
                prop::sample::subsequence(ids.clone(), 1..=ids.len().min(3)),
                prop::sample::select(ids),
                prop::sample::select(&[Linked, Released][..]),
                any::<bool>(),
            )
        })
        .boxed()
}

fn listed_but(f: &Franchise, id: &str) -> BTreeSet<String> {
    f.entries
        .iter()
        .chain(&f.versions)
        .chain(&f.films)
        .chain(&f.sides)
        .chain(&f.recaps)
        .map(|e| e.id.clone())
        .filter(|x| x != id)
        .collect()
}

proptest! {
    #[test]
    fn shows_a_strip_exactly_where_the_strip_model_has_one((f, id, order, rows) in any_page()) {
        let got = watch_order_page(&f, &rows, &id, order);
        prop_assert_eq!(got.is_none(), strip_for(&f, &id, order).is_none());
    }

    #[test]
    fn never_hides_the_entry_itself_and_hides_the_rest_of_what_the_franchise_page_lists((f, id, order, rows) in any_page()) {
        let Some(got) = watch_order_page(&f, &rows, &id, order) else { return Ok(()) };
        prop_assert!(!got.similar_hidden.contains(&id));
        prop_assert_eq!(&got.similar_hidden, &listed_but(&f, &id));
        for e in got.view.before.iter().chain(&got.view.after) {
            prop_assert!(got.similar_hidden.contains(&e.id));
        }
        if let Some(w) = &got.watching {
            prop_assert!(got.similar_hidden.contains(&w.entry.id));
        }
    }

    #[test]
    fn puts_a_position_in_the_eyebrow_only_with_a_total((f, id, order, rows) in any_page()) {
        let Some(got) = watch_order_page(&f, &rows, &id, order) else { return Ok(()) };
        if let Some(e) = got.eyebrow {
            prop_assert_eq!(got.caption, StripCaption::Position { position: e.position, total: e.total });
            prop_assert!(e.position >= 1 && e.position <= e.total);
        }
        if order == Released && got.view.kind == StripKind::Run && f.complete {
            prop_assert_eq!(got.eyebrow.map(|e| e.total), Some(got.run.entries.len()));
        }
    }

    #[test]
    fn carries_the_read_being_partial_and_then_claims_no_number((f, id, order, rows) in any_page()) {
        let Some(got) = watch_order_page(&f, &rows, &id, order) else { return Ok(()) };
        prop_assert_eq!(got.complete, f.complete);
        if !f.complete {
            let numbered = matches!(got.caption, StripCaption::Position { .. });
            prop_assert!(!numbered);
            if got.view.kind == StripKind::Run {
                prop_assert_eq!(got.caption, StripCaption::Partial);
            }
            prop_assert_eq!(got.eyebrow, None);
        }
    }

    #[test]
    fn calls_the_shown_entry_a_film_exactly_where_it_is_a_side_story_listed_under_films(
        (f, parents, member, order, film) in side_page()
    ) {
        let entry = WatchEntry {
            id: "999".into(),
            title: "T999".into(),
            subtype: Some(if film { "movie" } else { "OVA" }.into()),
            start: None,
            episodes: None,
            status: None,
            poster: None,
            cover: None,
        };
        let placed = Franchise {
            anchor: "999".into(),
            parents: [("999".to_string(), parents)].into_iter().collect(),
            films: if film { vec![entry.clone()] } else { Vec::new() },
            sides: if film { Vec::new() } else { vec![entry] },
            ..f
        };
        let got = watch_order_page(&placed, &[], "999", order).expect("a strip");
        prop_assert_eq!(got.view.kind, StripKind::Side);
        prop_assert_eq!(got.caption, if film { StripCaption::Film } else { StripCaption::Side });
        let on_member = Franchise { anchor: member.clone(), ..placed };
        if let Some(p) = watch_order_page(&on_member, &[], &member, order) {
            prop_assert_ne!(p.caption, StripCaption::Film);
        }
    }

    #[test]
    fn names_as_being_watched_only_another_member_part_way_that_no_tile_shows((f, id, order, rows) in any_page()) {
        let Some(got) = watch_order_page(&f, &rows, &id, order) else { return Ok(()) };
        let Some(w) = &got.watching else { return Ok(()) };
        prop_assert_ne!(&w.entry.id, &id);
        prop_assert!(got.run.entries.iter().any(|e| e.id == w.entry.id));
        prop_assert_eq!(got.progress.get(&w.entry.id), Some(&EntryProgress::Episode(w.episode)));
        prop_assert!(!got.view.before.iter().chain(&got.view.after).any(|e| e.id == w.entry.id));
    }

    #[test]
    fn strip_for_never_throws_and_shows_only_members_of_the_run_it_returns(
        f in franchise_arb(),
        order in prop::sample::select(&[Linked, Released][..])
    ) {
        let Some(got) = strip_for(&f, &f.anchor, order) else { return Ok(()) };
        let members: BTreeSet<&str> = got.run.entries.iter().map(|e| e.id.as_str()).collect();
        prop_assert!(members.contains(got.run.name_id.as_str()));
        for e in got.view.before.iter().chain(&got.view.after) {
            prop_assert!(members.contains(e.id.as_str()));
        }
        for (a, b) in &got.run.links {
            prop_assert!(a != b && members.contains(a.as_str()) && members.contains(b.as_str()));
        }
        if got.view.kind == StripKind::Run {
            prop_assert!(members.contains(f.anchor.as_str()));
        } else {
            prop_assert!(!got.view.before.is_empty());
        }
    }

    #[test]
    fn shows_no_strip_for_a_run_that_is_the_anchor_alone_with_no_parents(f in franchise_arb()) {
        let Some(solo) = f.entries.iter().find(|e| e.id == f.anchor).cloned() else { return Ok(()) };
        let anchor = f.anchor.clone();
        let alone = Franchise { entries: vec![solo], parents: Default::default(), ..f };
        prop_assert_eq!(strip_for(&alone, &anchor, Linked), None);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn reaches_the_franchise_page_by_the_strip_or_alone_exactly_where_it_lists_more_than_the_show(
        (f, id, order) in prop_oneof![franchise_around_arb(), franchise_arb()].prop_flat_map(|f| {
            let mut ids: Vec<String> = f.entries.iter().map(|e| e.id.clone()).collect();
            ids.push(f.anchor.clone());
            let id = prop_oneof![3 => Just(f.anchor.clone()), 1 => prop::sample::select(ids)];
            (Just(f), id, prop::sample::select(&[Linked, Released][..]))
        })
    ) {
        let strip = watch_order_page(&f, &[], &id, order);
        let link = franchise_link_only(&f, &id);
        prop_assert!(!(strip.is_some() && link.is_some()));
        let beyond = franchise_page(&f, &[], &id, order).is_some_and(|p| p.run.all().any(|e| e.id != id));
        prop_assert_eq!(strip.is_some() || link.is_some(), beyond);
        if let Some(link) = link {
            prop_assert_eq!(link.complete, f.complete);
            prop_assert!(!link.similar_hidden.contains(&id));
            prop_assert_eq!(link.similar_hidden, listed_but(&f, &id));
        }
    }
}
