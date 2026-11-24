//! The franchise page (ported from the renderer's
//! `franchise-page.test.ts`; the breadcrumb's wording stays with the
//! renderer, the title it links back to is `opened_from`).

use std::collections::BTreeSet;

use proptest::prelude::*;

use super::fixtures::{ids, payload, run, runs};
use super::franchise_page::{franchise_page, FranchisePageModel};
use super::place::placement;
use super::rail::{path_note, PathNote};
use super::run::run_of;
use super::sections::{FranchiseRow, FranchiseSection};
use super::segments::{segments, Segment};
use super::sequence::sequence;
use super::strategies::{franchise_arb, franchise_around_arb};
use super::strip_view::current_in;
use super::{EntryProgress, Franchise, HistoryRow, Progress, WatchEntry, WatchOrder};

use WatchOrder::{Linked, Released};

fn row(kitsu_id: &str, ep_no: &str) -> HistoryRow {
    HistoryRow {
        kitsu_id: Some(kitsu_id.into()),
        ep_no: ep_no.into(),
    }
}

fn named(key: &str, anchor: &str, complete: bool, name: &str, name_from: &str) -> Franchise {
    Franchise {
        complete,
        name: name.into(),
        name_from: name_from.into(),
        ..payload(key, anchor)
    }
}

fn page_of(key: &str, anchor: &str, rows: &[HistoryRow], order: WatchOrder) -> FranchisePageModel {
    franchise_page(&payload(key, anchor), rows, anchor, order).expect("a page")
}

fn partial(key: &str, anchor: &str) -> Franchise {
    Franchise {
        complete: false,
        ..payload(key, anchor)
    }
}

/// Every main-story row, in the page's reading order.
fn rows_of(page: &FranchisePageModel) -> Vec<&FranchiseRow> {
    page.sections
        .iter()
        .flat_map(|s| match s {
            FranchiseSection::Ways { ways } => ways.iter().flatten().collect::<Vec<_>>(),
            FranchiseSection::Line { rows, .. } | FranchiseSection::Group { rows } => {
                rows.iter().collect()
            }
        })
        .collect()
}

fn kind(s: &FranchiseSection) -> &'static str {
    match s {
        FranchiseSection::Line { .. } => "line",
        FranchiseSection::Ways { .. } => "ways",
        FranchiseSection::Group { .. } => "group",
    }
}

fn kinds(page: &FranchisePageModel) -> Vec<&'static str> {
    page.sections.iter().map(kind).collect()
}

fn line(s: &FranchiseSection) -> (&[FranchiseRow], bool) {
    match s {
        FranchiseSection::Line { rows, unplaced } => (rows, *unplaced),
        other => panic!("a line, not {other:?}"),
    }
}

#[test]
fn has_no_page_when_the_franchise_answers_another_title_or_is_empty() {
    assert_eq!(
        franchise_page(&payload("jojo", "7158"), &[], "8063", Linked),
        None
    );
    let empty = Franchise {
        entries: Vec::new(),
        links: Vec::new(),
        ..payload("jojo", "7158")
    };
    assert_eq!(franchise_page(&empty, &[], "7158", Linked), None);
}

#[test]
fn carries_the_name_the_walk_gives_the_franchise_whatever_the_order_setting() {
    for order in [Linked, Released] {
        let f = named(
            "jojo",
            "46598",
            true,
            "JoJo no Kimyou na Bouken (TV)",
            "7158",
        );
        let page = franchise_page(&f, &[], "46598", order).expect("a page");
        assert_eq!(page.name, "JoJo no Kimyou na Bouken (TV)");
        assert_eq!(page.art.id, "7158");
    }
}

#[test]
fn takes_its_artwork_from_the_entry_the_walk_names_it_after_not_its_first_by_links() {
    // Fate/Zero comes first by its links; the walk names the franchise
    // after Fate/stay night, its first series by start.
    let f = named("fate", "6028", true, "Fate/stay night", "323");
    for order in [Linked, Released] {
        assert_eq!(
            franchise_page(&f, &[], "6028", order)
                .expect("a page")
                .art
                .id,
            "323"
        );
    }
}

#[test]
fn counts_every_title_it_lists() {
    let page = page_of("jojo", "46598", &[], Linked);
    // 9 in the main story, 4 other versions, 1 side story, 1 recap.
    assert_eq!(page.total, Some(15));
    let jojo = run("jojo");
    assert_eq!(ids(&page.run.versions), ids(&jojo.versions));
    assert_eq!(ids(&page.run.sides), ids(&jojo.sides));
    assert_eq!(ids(&page.run.recaps), ids(&jojo.recaps));
}

#[test]
fn resumes_the_title_being_watched_with_its_place_and_episode() {
    let page = page_of(
        "jojo",
        "41410",
        &[row("7158", "26"), row("49847", "1")],
        Linked,
    );
    let resume = page.resume.expect("a resume");
    assert_eq!(
        (resume.entry.id.as_str(), resume.episode, resume.position),
        ("49847", 1.0, Some(9))
    );
    assert!(!resume.entry.title.is_empty());
}

#[test]
fn has_nothing_to_resume_when_nothing_is_part_way_and_starts_from_the_first_title() {
    let page = page_of("jojo", "41410", &[], Linked);
    assert_eq!(page.resume, None);
    assert_eq!(page.start.map(|e| e.id).as_deref(), Some("7158"));
}

#[test]
fn starts_from_the_earliest_release_in_release_order() {
    // Fate/stay night (2006) came out before Fate/Zero (2011).
    assert_eq!(
        page_of("fate", "6028", &[], Released)
            .start
            .map(|e| e.id)
            .as_deref(),
        Some("323")
    );
}

#[test]
fn says_which_title_a_stray_one_leads_into_by_its_whole_title() {
    for r in runs() {
        let titles: BTreeSet<&str> = r.run.entries.iter().map(|x| x.title.as_str()).collect();
        for e in &r.run.entries {
            let Some(page) =
                franchise_page(&payload(&r.key, &e.id), &[row(&e.id, "1")], &e.id, Linked)
            else {
                continue;
            };
            for row in rows_of(&page) {
                if let Some(to) = &row.leads_into {
                    assert!(titles.contains(to.as_str()), "{to}");
                }
            }
        }
    }
    // Gintama: The Semi-Final leads into The Final from outside the main line.
    let page = page_of("gintama", "818", &[], Linked);
    let rows = rows_of(&page);
    let semi = rows
        .iter()
        .find(|r| r.entry.id == "43829")
        .expect("The Semi-Final");
    let fin = rows
        .iter()
        .find(|r| r.entry.id == "43683")
        .expect("The Final");
    assert_eq!(semi.leads_into.as_deref(), Some(fin.entry.title.as_str()));
}

#[test]
fn numbers_a_straight_run_of_seasons_from_one_marking_the_title_it_was_opened_from() {
    let page = page_of("jojo", "46598", &[], Linked);
    assert_eq!(kinds(&page), ["line"]);
    let rows = rows_of(&page);
    assert_eq!(
        rows.iter().map(|r| r.position).collect::<Vec<_>>(),
        (1..=9).map(Some).collect::<Vec<_>>()
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r.here)
            .map(|r| r.entry.id.as_str())
            .collect::<Vec<_>>(),
        ["46598"]
    );
}

#[test]
fn shows_progress_per_title_a_watched_one_in_full() {
    let page = page_of(
        "jojo",
        "46598",
        &[row("7158", "26"), row("8063", "6")],
        Linked,
    );
    let rows = rows_of(&page);
    let at = |id: &str| {
        rows.iter()
            .find(|r| r.entry.id == id)
            .copied()
            .expect("a row")
    };
    assert_eq!(
        (at("7158").progress, at("7158").share),
        (Some(EntryProgress::Watched), 1.0)
    );
    assert_eq!(at("8063").progress, Some(EntryProgress::Episode(6.0)));
    assert!((at("8063").share - 6.0 / 24.0).abs() < 1e-9);
    assert_eq!((at("8739").progress, at("8739").share), (None, 0.0));
}

#[test]
fn lays_monogataris_titles_with_no_clear_order_out_as_one_group_unnumbered() {
    let page = page_of("monogatari", "3919", &[], Linked);
    assert_eq!(kinds(&page), ["line", "group", "line"]);
    let FranchiseSection::Group { rows } = &page.sections[1] else {
        panic!("a group");
    };
    assert_eq!(rows.len(), 10);
    assert!(rows.iter().all(|r| r.position.is_none()));
    assert!(line(&page.sections[2]).1);
    assert!(!line(&page.sections[0]).1);
}

#[test]
fn puts_the_ways_to_continue_side_by_side_unnumbered_where_the_story_branches() {
    let page = page_of("fate", "6028", &[], Linked);
    let Some(FranchiseSection::Ways { ways }) = page.sections.iter().find(|s| kind(s) == "ways")
    else {
        panic!("ways");
    };
    assert!(ways.len() > 1);
    assert!(ways.iter().flatten().all(|r| r.position.is_none()));
}

/// A payload of `ids` linked by `links`, anchored on the first.
fn shaped(list: &[&str], links: &[(&str, &str)]) -> Franchise {
    let entries: Vec<WatchEntry> = list
        .iter()
        .enumerate()
        .map(|(i, id)| WatchEntry {
            id: (*id).into(),
            title: format!("T{id}"),
            subtype: Some("TV".into()),
            start: Some(format!("20{}-01-01", 10 + i)),
            episodes: Some(12),
            status: Some("finished".into()),
            poster: None,
            cover: None,
        })
        .collect();
    Franchise {
        anchor: list[0].into(),
        name: entries[0].title.clone(),
        name_from: list[0].into(),
        entries,
        links: links
            .iter()
            .map(|(a, b)| ((*a).into(), (*b).into()))
            .collect(),
        versions: Vec::new(),
        films: Vec::new(),
        sides: Vec::new(),
        recaps: Vec::new(),
        parents: Default::default(),
        complete: true,
        chain: None,
    }
}

fn shaped_page(list: &[&str], links: &[(&str, &str)]) -> FranchisePageModel {
    let f = shaped(list, links);
    franchise_page(&f, &[], &f.anchor, Linked).expect("a page")
}

const ONE_WAY: [(&str, &str); 4] = [("1", "2"), ("1", "3"), ("4", "3"), ("2", "5")];

#[test]
fn heads_the_titles_after_ways_that_join_back_up_as_having_no_clear_place() {
    let page = shaped_page(
        &["1", "2", "3", "4", "5"],
        &[("1", "2"), ("1", "3"), ("2", "4"), ("3", "4"), ("4", "5")],
    );
    assert_eq!(kinds(&page), ["line", "ways", "line"]);
    let (rows, unplaced) = line(&page.sections[2]);
    assert_eq!(
        rows.iter().map(|r| r.position).collect::<Vec<_>>(),
        [None, None]
    );
    assert!(unplaced);
}

#[test]
fn never_draws_ways_to_continue_with_only_one_way_and_explains_every_unnumbered_title() {
    let page = shaped_page(&["1", "2", "3", "4", "5"], &ONE_WAY);
    for s in &page.sections {
        match s {
            FranchiseSection::Ways { ways } => assert!(ways.len() > 1),
            FranchiseSection::Line { rows, unplaced } => {
                for r in rows.iter().filter(|r| r.position.is_none()) {
                    assert!(r.leads_into.is_some() || *unplaced);
                }
            }
            FranchiseSection::Group { .. } => {}
        }
    }
    // One heading over unnumbered stretches that follow each other.
    for pair in page.sections.windows(2) {
        if let (
            FranchiseSection::Line { unplaced: a, .. },
            FranchiseSection::Line { unplaced: b, .. },
        ) = (&pair[0], &pair[1])
        {
            assert!(!(*a && *b));
        }
    }
}

#[test]
fn gathers_the_titles_with_no_clear_place_at_the_end_with_the_titles_that_only_lead_into_them() {
    // 1→2→J and R→S→J: S has no number, and R only leads into S.
    let page = shaped_page(
        &["1", "2", "R", "S", "J"],
        &[("1", "2"), ("2", "J"), ("R", "S"), ("S", "J")],
    );
    let (last, unplaced) = line(page.sections.last().expect("a section"));
    assert!(unplaced);
    assert_eq!(
        last.iter().map(|r| r.entry.id.as_str()).collect::<Vec<_>>(),
        ["R", "S"]
    );
    let (first, _) = line(&page.sections[0]);
    let got: Vec<(&str, Option<usize>)> = first
        .iter()
        .map(|r| (r.entry.id.as_str(), r.position))
        .collect();
    assert_eq!(got, [("1", Some(1)), ("2", Some(2)), ("J", Some(3))]);
}

#[test]
fn keeps_numbered_titles_that_follow_each_other_in_one_list() {
    // 11 → 51 → 14, and 59 → 51, 59 → 31: 59 and 31 have no clear place,
    // and 11, 51 and 14 read as one numbered list. Opened from 31.
    let page = shaped_page(
        &["31", "51", "14", "11", "59"],
        &[("59", "51"), ("51", "14"), ("59", "31"), ("11", "51")],
    );
    let (first, _) = line(&page.sections[0]);
    let got: Vec<(&str, Option<usize>)> = first
        .iter()
        .map(|r| (r.entry.id.as_str(), r.position))
        .collect();
    assert_eq!(got, [("11", Some(1)), ("51", Some(2)), ("14", Some(3))]);
}

#[test]
fn is_alone_only_when_the_one_title_it_lists_is_the_show_and_never_when_read_in_part() {
    let f = payload("jojo", "7158");
    let only = |id: &str, complete: bool| Franchise {
        entries: f.entries.iter().filter(|e| e.id == id).cloned().collect(),
        links: Vec::new(),
        versions: Vec::new(),
        films: Vec::new(),
        sides: Vec::new(),
        recaps: Vec::new(),
        complete,
        ..f.clone()
    };
    let alone = |f: &Franchise| {
        franchise_page(f, &[], "7158", Linked)
            .expect("a page")
            .alone
    };
    assert!(!alone(&only("8063", true)));
    assert!(alone(&only("7158", true)));
    assert!(!alone(&only("7158", false)));
}

#[test]
fn keeps_a_title_that_says_what_it_leads_into_among_the_numbered_ones() {
    // Gintama: The Semi-Final leads into The Final, with no heading.
    let page = page_of("gintama", "818", &[], Linked);
    assert!(!page
        .sections
        .iter()
        .any(|s| matches!(s, FranchiseSection::Line { unplaced: true, .. })));
}

#[test]
fn lists_every_title_in_one_numbered_line_in_release_order() {
    let page = page_of("monogatari", "3919", &[], Released);
    assert_eq!(page.sections.len(), 1);
    let want: Vec<Option<usize>> = (1..=run("monogatari").entries.len()).map(Some).collect();
    assert_eq!(
        rows_of(&page)
            .iter()
            .map(|r| r.position)
            .collect::<Vec<_>>(),
        want
    );
}

#[test]
fn says_when_the_franchise_read_stopped_early() {
    assert!(
        franchise_page(&partial("jojo", "7158"), &[], "7158", Linked)
            .expect("a page")
            .partial
    );
    assert!(!page_of("jojo", "7158", &[], Linked).partial);
}

#[test]
fn claims_no_place_total_or_beginning_for_a_franchise_read_in_part() {
    // The walk goes both ways from the show, so it may have missed
    // earlier titles: nothing counted from the first title it reached is
    // sure.
    for order in [Linked, Released] {
        let page = franchise_page(&partial("jojo", "7158"), &[row("7158", "3")], "7158", order)
            .expect("a page");
        assert!(rows_of(&page).iter().all(|r| r.position.is_none()));
        assert_eq!((page.total, page.start.as_ref()), (None, None));
        let resume = page.resume.expect("a resume");
        assert_eq!((resume.entry.id.as_str(), resume.position), ("7158", None));
    }
}

#[test]
fn counts_the_recaps_only_for_a_franchise_read_whole() {
    let whole = page_of("jojo", "7158", &[], Linked);
    assert_eq!(whole.recap_count, Some(whole.run.recaps.len()));
    let part = franchise_page(&partial("jojo", "7158"), &[], "7158", Linked).expect("a page");
    assert_eq!(part.recap_count, None);
    assert_eq!(part.run.recaps, whole.run.recaps);
}

/// The page's sections as kinds and ids, numbers left out.
fn shape(p: &FranchisePageModel) -> Vec<String> {
    p.sections
        .iter()
        .map(|s| match s {
            FranchiseSection::Line { rows, unplaced } => {
                format!("line {unplaced} {:?}", ids(rows.iter().map(|r| &r.entry)))
            }
            FranchiseSection::Ways { ways } => {
                format!(
                    "ways {:?}",
                    ways.iter()
                        .map(|w| ids(w.iter().map(|r| &r.entry)))
                        .collect::<Vec<_>>()
                )
            }
            FranchiseSection::Group { rows } => {
                format!("group {:?}", ids(rows.iter().map(|r| &r.entry)))
            }
        })
        .collect()
}

#[test]
fn keeps_the_layout_of_a_franchise_read_whole_only_without_its_numbers_when_read_in_part() {
    let whole = page_of("jojo", "46598", &[], Linked);
    let part = franchise_page(&partial("jojo", "46598"), &[], "46598", Linked).expect("a page");
    assert_eq!(shape(&part), shape(&whole));
}

#[test]
fn opens_for_a_side_story_which_sits_among_the_side_stories_rather_than_the_main_story() {
    let page = page_of("fate", "6928", &[], Linked);
    assert!(ids(&page.run.sides).contains(&"6928".to_string()));
    assert!(!rows_of(&page).iter().any(|r| r.here));
}

#[test]
fn names_the_title_it_was_opened_from_for_the_trail_even_outside_the_main_story() {
    let page = page_of("jojo", "41410", &[], Linked);
    assert_eq!(
        page.opened_from.map(|e| e.title).as_deref(),
        Some("JoJo's Bizarre Adventure: Golden Wind")
    );
    let side = run("fate")
        .sides
        .into_iter()
        .find(|e| e.id == "6928")
        .expect("the side story");
    assert_eq!(page_of("fate", "6928", &[], Linked).opened_from, Some(side));
}

/// Mostly payloads that answer the page's own id, with titles around
/// the run; sometimes any payload, malformed links included.
fn any_page() -> BoxedStrategy<(Franchise, String, WatchOrder, Vec<HistoryRow>)> {
    prop_oneof![franchise_around_arb(), franchise_arb()]
        .prop_flat_map(|f| {
            let mut ids: Vec<String> = f.entries.iter().map(|e| e.id.clone()).collect();
            let mut named = ids.clone();
            named.push("0".into());
            ids.push(f.anchor.clone());
            let id = prop_oneof![3 => Just(f.anchor.clone()), 1 => prop::sample::select(ids)];
            let rows = prop::collection::vec(
                (
                    prop::option::of(prop::sample::select(named)),
                    prop::sample::select(&["1", "2", "12", "30", "x"][..]),
                )
                    .prop_map(|(kitsu_id, ep)| HistoryRow {
                        kitsu_id,
                        ep_no: ep.into(),
                    }),
                0..=6,
            );
            (
                Just(f),
                id,
                prop::sample::select(&[Linked, Released][..]),
                rows,
            )
        })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn never_sets_two_lists_of_the_same_kind_side_by_side((f, id, order, rows) in any_page()) {
        let Some(page) = franchise_page(&f, &rows, &id, order) else { return Ok(()) };
        for pair in page.sections.windows(2) {
            if let (FranchiseSection::Line { unplaced: a, .. }, FranchiseSection::Line { unplaced: b, .. }) = (&pair[0], &pair[1]) {
                prop_assert_ne!(a, b);
            }
        }
    }
}

proptest! {
    #[test]
    fn keeps_the_sections_of_a_read_whole_when_read_in_part_only_without_numbers((f, id, order, rows) in any_page()) {
        let part = franchise_page(&Franchise { complete: false, ..f.clone() }, &rows, &id, order);
        let whole = franchise_page(&Franchise { complete: true, ..f }, &rows, &id, order);
        if let (Some(part), Some(whole)) = (part, whole) {
            prop_assert_eq!(shape(&part), shape(&whole));
        }
    }

    #[test]
    fn has_a_page_exactly_where_the_payload_answers_this_title_with_a_run((f, id, order, rows) in any_page()) {
        let got = franchise_page(&f, &rows, &id, order);
        prop_assert_eq!(got.is_none(), f.anchor != id || run_of(&f).is_none());
    }

    #[test]
    fn lists_each_title_of_the_run_once_numbered_as_its_placement_says_when_read_whole((f, id, order, rows) in any_page()) {
        let Some(page) = franchise_page(&f, &rows, &id, order) else { return Ok(()) };
        let listed = rows_of(&page);
        let listed_ids: BTreeSet<String> = listed.iter().map(|r| r.entry.id.clone()).collect();
        prop_assert_eq!(listed_ids, page.run.entries.iter().map(|e| e.id.clone()).collect::<BTreeSet<_>>());
        prop_assert_eq!(listed.len(), page.run.entries.len());
        for s in &page.sections {
            if let FranchiseSection::Line { rows, .. } = s {
                for r in rows {
                    let want = if f.complete { placement(&page.run, &r.entry.id, order).position } else { None };
                    prop_assert_eq!(r.position, want);
                }
            }
        }
        for r in &listed {
            prop_assert_eq!(r.here, r.entry.id == id);
            let note = path_note(&page.run, &r.entry.id, order);
            prop_assert_eq!(r.leads_into.is_some(), matches!(note, Some(PathNote::LeadsInto { .. })));
            prop_assert!((0.0..=1.0).contains(&r.share));
        }
    }

    #[test]
    fn follows_the_ordering_its_sections_are_the_segments_in_release_order_one_line((f, id, order, rows) in any_page()) {
        let Some(page) = franchise_page(&f, &rows, &id, order) else { return Ok(()) };
        // The ways and groups come in the segments' order. A fork left
        // with one way is drawn as a line, and the lines' titles may move
        // to the stretch with no clear place.
        let got: Vec<&str> = page.sections.iter().map(kind).filter(|k| *k != "line").collect();
        let want: Vec<&str> = segments(&page.run, order)
            .iter()
            .map(|s| match s {
                Segment::Cycle { .. } => "group",
                Segment::Fork { branches, .. } if branches.len() > 1 => "ways",
                _ => "line",
            })
            .filter(|k| *k != "line")
            .collect();
        prop_assert_eq!(got, want);
        if order == Released {
            prop_assert_eq!(page.sections.len(), 1);
        }
        let start = if f.complete { sequence(&page.run, order).first().map(|e| (*e).clone()) } else { None };
        prop_assert_eq!(page.start, start);
    }

    #[test]
    fn explains_every_title_it_cannot_number_and_draws_ways_only_in_twos_or_more((f, id, order, rows) in any_page()) {
        let Some(page) = franchise_page(&f, &rows, &id, order) else { return Ok(()) };
        let unplaced: Vec<usize> = page.sections.iter().enumerate()
            .filter(|(_, s)| matches!(s, FranchiseSection::Line { unplaced: true, .. }))
            .map(|(i, _)| i)
            .collect();
        // At most one stretch with no clear place, and it comes last.
        prop_assert!(unplaced.len() <= 1);
        if let Some(&at) = unplaced.first() {
            prop_assert_eq!(at, page.sections.len() - 1);
        }
        let loose: BTreeSet<&str> = unplaced.first().map(|&i| line(&page.sections[i]).0.iter().map(|r| r.entry.id.as_str()).collect()).unwrap_or_default();
        for s in &page.sections {
            match s {
                FranchiseSection::Ways { ways } => prop_assert!(ways.len() > 1),
                FranchiseSection::Line { rows, unplaced: false } => {
                    // Outside it, a title with no number leads into a title
                    // that has a place (read in part, no title is numbered).
                    for r in rows.iter().filter(|r| r.position.is_none() && f.complete) {
                        let Some(PathNote::LeadsInto { entry }) = path_note(&page.run, &r.entry.id, order) else {
                            return Err(TestCaseError::fail("an unnumbered title with no note"));
                        };
                        prop_assert!(!loose.contains(entry.id.as_str()));
                    }
                }
                _ => {}
            }
        }
    }

    #[test]
    fn says_the_franchise_is_the_show_alone_only_when_it_lists_the_show_and_nothing_else((f, id, order, rows) in any_page()) {
        let Some(page) = franchise_page(&f, &rows, &id, order) else { return Ok(()) };
        let listed: Vec<&WatchEntry> = page.run.all().collect();
        prop_assert_eq!(page.alone, f.complete && listed.len() == 1 && listed[0].id == id);
    }

    #[test]
    fn carries_the_name_the_walk_gives_it_and_that_entrys_artwork_and_counts_what_it_lists((f, id, order, rows) in any_page()) {
        let Some(page) = franchise_page(&f, &rows, &id, order) else { return Ok(()) };
        prop_assert_eq!(&page.name, &f.name);
        prop_assert_eq!(&page.art.id, &f.name_from);
        prop_assert_eq!(page.total, f.complete.then(|| page.run.all().count()));
        prop_assert_eq!(page.partial, !f.complete);
    }

    #[test]
    fn resumes_the_title_the_strip_calls_the_one_being_watched((f, id, order, rows) in any_page()) {
        let Some(page) = franchise_page(&f, &rows, &id, order) else { return Ok(()) };
        let progress: Progress = rows_of(&page).iter().filter_map(|r| Some((r.entry.id.clone(), r.progress?))).collect();
        let now = current_in(&page.run, &progress, order);
        prop_assert_eq!(page.resume.as_ref().map(|r| (&r.entry.id, r.episode)), now.as_ref().map(|n| (&n.entry.id, n.episode)));
        if let Some(r) = &page.resume {
            let want = if f.complete { placement(&page.run, &r.entry.id, order).position } else { None };
            prop_assert_eq!(r.position, want);
        }
    }

    #[test]
    fn names_the_title_it_was_opened_from_where_the_page_lists_it((f, id) in prop_oneof![franchise_around_arb(), franchise_arb()].prop_flat_map(|f| {
        let mut ids: Vec<String> = f.entries.iter().map(|e| e.id.clone()).collect();
        ids.push(f.anchor.clone());
        (Just(f), prop::sample::select(ids))
    })) {
        let Some(page) = franchise_page(&f, &[], &id, Linked) else { return Ok(()) };
        let listed = f.entries.iter().chain(&f.versions).chain(&f.films).chain(&f.sides).chain(&f.recaps).find(|e| e.id == id);
        prop_assert_eq!(page.opened_from.as_ref().map(|e| &e.id), listed.map(|e| &e.id));
    }
}
