//! The show page on a page outside the main story, drawn on its chain
//! (ported from the renderer's `chain-page.test.ts`).

use proptest::prelude::*;

use super::chain_view::Seam;
use super::fixtures::ids;
use super::page_model::{watch_order_page, Eyebrow, Seams, StripCaption, WatchOrderPage};
use super::strip_view::StripKind;
use super::{Chain, Franchise, RunLink, WatchEntry, WatchOrder};

use WatchOrder::{Linked, Released};

fn e(id: &str, start: &str, subtype: &str) -> WatchEntry {
    WatchEntry {
        id: id.into(),
        title: format!("T{id}"),
        subtype: Some(subtype.into()),
        start: Some(start.into()),
        episodes: Some(1),
        status: Some("finished".into()),
        poster: None,
        cover: None,
    }
}

fn film(id: &str, start: &str) -> WatchEntry {
    e(id, start, "movie")
}

fn l(a: &str, b: &str) -> RunLink {
    (a.into(), b.into())
}

// Fate's main story, and the Heaven's Feel films that branch off after
// Fate/Zero 2nd Season (7658): the shape the walk serves them in.
fn fz() -> WatchEntry {
    e("6028", "2011-10-02", "TV")
}
fn fz2() -> WatchEntry {
    e("7658", "2012-04-08", "TV")
}
fn fsn() -> WatchEntry {
    e("323", "2006-01-07", "TV")
}
fn hf1() -> WatchEntry {
    film("8675", "2017-10-14")
}
fn hf2() -> WatchEntry {
    film("12007", "2019-01-12")
}
fn hf3() -> WatchEntry {
    film("12008", "2020-08-15")
}

fn fate(anchor: &str, chain: Option<Chain>, complete: bool) -> Franchise {
    Franchise {
        anchor: anchor.into(),
        name: "N".into(),
        name_from: String::new(),
        entries: vec![fsn(), fz(), fz2()],
        links: vec![l("6028", "7658"), l("7658", "323")],
        versions: Vec::new(),
        films: vec![hf1(), hf2(), hf3()],
        sides: Vec::new(),
        recaps: Vec::new(),
        parents: Default::default(),
        complete,
        chain,
    }
}

fn heavens_feel() -> Chain {
    Chain {
        entries: vec![fz2(), hf1(), hf2(), hf3()],
        links: vec![l("7658", "8675"), l("8675", "12007"), l("12007", "12008")],
        seams: vec!["7658".into()],
    }
}

fn page(f: &Franchise, order: WatchOrder) -> WatchOrderPage {
    watch_order_page(f, &[], &f.anchor, order).expect("a strip")
}

const NO_SEAMS: Seams = Seams {
    before: None,
    after: None,
};

#[test]
fn numbers_the_chains_own_titles_among_themselves() {
    let p = page(&fate("12007", Some(heavens_feel()), true), Linked);
    assert!(p.outside);
    assert_eq!(
        p.caption,
        StripCaption::Position {
            position: 2,
            total: 3
        }
    );
    assert_eq!(
        p.eyebrow,
        Some(Eyebrow {
            position: 2,
            total: 3
        })
    );
    assert_eq!(ids(&p.view.before), ["8675"]);
    assert_eq!(ids(&p.view.after), ["12008"]);
    assert_eq!(p.seams, NO_SEAMS);
}

#[test]
fn stacks_the_franchise_link_from_the_main_story_and_hides_the_whole_franchise_from_similar_titles()
{
    let p = page(&fate("12007", Some(heavens_feel()), true), Linked);
    assert_eq!(ids(&p.main_run.entries), ["323", "6028", "7658"]);
    assert_eq!(ids(&p.run.entries), ["7658", "8675", "12007", "12008"]);
    for other in ["323", "6028", "7658", "8675", "12008"] {
        assert!(p.similar_hidden.contains(other), "{other}");
    }
    let main = page(&fate("7658", None, true), Linked);
    assert_eq!(main.main_run, main.run);
}

#[test]
fn shows_the_main_story_title_it_branches_off_from_uncounted_as_where_it_comes_from() {
    let p = page(&fate("8675", Some(heavens_feel()), true), Linked);
    assert_eq!(
        p.caption,
        StripCaption::Position {
            position: 1,
            total: 3
        }
    );
    assert_eq!(ids(&p.view.before), ["7658"]);
    assert_eq!(
        p.seams,
        Seams {
            before: Some(Seam::From),
            after: None
        }
    );
}

#[test]
fn ends_at_its_own_last_title_when_it_leads_nowhere() {
    let p = page(&fate("12008", Some(heavens_feel()), true), Linked);
    assert_eq!(
        p.caption,
        StripCaption::Position {
            position: 3,
            total: 3
        }
    );
    assert!(p.view.after.is_empty());
}

#[test]
fn goes_on_into_the_main_story_where_it_leads_into_it_still_counting_only_its_own() {
    let (f1, f2, f3) = (
        film("6084", "2012-02-04"),
        film("6664", "2012-06-23"),
        film("6665", "2013-02-01"),
    );
    let tv = e("11655", "2016-07-01", "TV");
    let franchise = Franchise {
        anchor: "6665".into(),
        entries: vec![e("24", "1997-10-08", "TV"), tv.clone()],
        links: vec![l("24", "11655")],
        films: vec![f1.clone(), f2.clone(), f3.clone()],
        chain: Some(Chain {
            entries: vec![f1, f2, f3, tv],
            links: vec![l("6084", "6664"), l("6664", "6665"), l("6665", "11655")],
            seams: vec!["11655".into()],
        }),
        ..fate("6665", None, true)
    };
    let p = page(&franchise, Linked);
    assert_eq!(
        p.caption,
        StripCaption::Position {
            position: 3,
            total: 3
        }
    );
    assert_eq!(ids(&p.view.after), ["11655"]);
    assert_eq!(
        p.seams,
        Seams {
            before: None,
            after: Some(Seam::Into)
        }
    );
}

#[test]
fn keys_a_seam_by_the_column_it_stands_in_in_either_order() {
    let tv = e("7158", "2012-10-06", "TV");
    let old = e("610", "1993-11-19", "OVA");
    let ova = e("7730", "2000-05-25", "OVA");
    let franchise = Franchise {
        anchor: "7730".into(),
        entries: vec![tv.clone(), e("8063", "2014-04-05", "TV")],
        links: vec![l("7158", "8063")],
        versions: vec![old.clone(), ova.clone()],
        films: Vec::new(),
        chain: Some(Chain {
            entries: vec![old, ova, tv],
            links: vec![l("7158", "7730"), l("7730", "610")],
            seams: vec!["7158".into()],
        }),
        ..fate("7730", None, true)
    };
    let linked = page(&franchise, Linked);
    assert_eq!(
        linked.caption,
        StripCaption::Position {
            position: 1,
            total: 2
        }
    );
    assert_eq!(ids(&linked.view.before), ["7158"]);
    assert_eq!(
        linked.seams,
        Seams {
            before: Some(Seam::From),
            after: None
        }
    );
    let released = page(&franchise, Released);
    assert_eq!(
        released.caption,
        StripCaption::Position {
            position: 2,
            total: 2
        }
    );
    assert_eq!(ids(&released.view.before), ["610"]);
    assert_eq!(ids(&released.view.after), ["7158"]);
    assert_eq!(
        released.seams,
        Seams {
            before: None,
            after: Some(Seam::Into)
        }
    );
}

#[test]
fn reads_a_chain_of_one_film_as_a_film_with_no_number() {
    let chain = Chain {
        entries: vec![fz2(), film("4788", "2010-01-23")],
        links: vec![l("7658", "4788")],
        seams: vec!["7658".into()],
    };
    let p = page(&fate("4788", Some(chain), true), Linked);
    assert_eq!(p.caption, StripCaption::Film);
    assert_eq!(p.eyebrow, None);
    assert_eq!(
        p.seams,
        Seams {
            before: Some(Seam::From),
            after: None
        }
    );
}

#[test]
fn reads_a_chain_of_one_title_that_is_not_a_film_with_no_number() {
    let chain = Chain {
        entries: vec![fz2(), e("5000", "2013-01-01", "OVA")],
        links: vec![l("7658", "5000")],
        seams: vec!["7658".into()],
    };
    let p = page(&fate("5000", Some(chain), true), Linked);
    assert_eq!(p.caption, StripCaption::Alone);
    assert_eq!(p.eyebrow, None);
    assert!(p.outside);
}

fn with_special(chain: Chain) -> Franchise {
    Franchise {
        sides: vec![e("9000", "2018-01-01", "special")],
        parents: [("9000".to_string(), vec!["8675".to_string()])]
            .into_iter()
            .collect(),
        ..fate("9000", Some(chain), true)
    }
}

#[test]
fn draws_a_side_story_of_a_chain_title_under_it_on_the_chain_outside_the_main_story() {
    let p = page(&with_special(heavens_feel()), Linked);
    assert_eq!(p.view.kind, StripKind::Side);
    assert_eq!(ids(&p.view.before), ["8675"]);
    assert_eq!(p.caption, StripCaption::Side);
    assert!(p.outside);
    assert_eq!(p.seams, NO_SEAMS);
}

#[test]
fn draws_a_side_story_under_a_parent_chained_to_nothing_the_parent_its_chain_alone() {
    // The walk draws a side story on its parent alone where the parent's
    // only link is to a version of it: one title, no seam.
    let chain = Chain {
        entries: vec![hf1()],
        links: Vec::new(),
        seams: Vec::new(),
    };
    let p = page(&with_special(chain), Linked);
    assert_eq!(p.view.kind, StripKind::Side);
    assert_eq!(ids(&p.view.before), ["8675"]);
    assert_eq!(p.caption, StripCaption::Side);
    assert_eq!(p.seams, NO_SEAMS);
}

#[test]
fn keys_a_column_that_holds_a_seam_beside_an_own_title_as_usual_and_counts_both() {
    // Heaven's Feel I forks: on to II, and back into the 2006 series.
    let chain = Chain {
        entries: vec![fsn(), fz2(), hf1(), hf2(), hf3()],
        links: vec![
            l("7658", "8675"),
            l("8675", "12007"),
            l("8675", "323"),
            l("12007", "12008"),
        ],
        seams: vec!["7658".into(), "323".into()],
    };
    let p = page(&fate("8675", Some(chain), true), Linked);
    let mut after = ids(&p.view.after);
    after.sort();
    assert_eq!(after, ["12007", "323"]);
    assert_eq!(p.seams.after, None);
    // Two entries in a column read whole: the strip counts its ways.
    assert!(p.complete && p.view.kind == StripKind::Run);
    assert_eq!(
        p.caption,
        StripCaption::Position {
            position: 1,
            total: 3
        }
    );
}

#[test]
fn in_release_order_places_a_seam_by_date_between_two_own_titles_where_it_falls() {
    let (a, b, s) = (
        film("100", "2001-01-01"),
        film("101", "2015-01-01"),
        e("1", "2010-01-01", "TV"),
    );
    let franchise = |anchor: &str| Franchise {
        entries: vec![s.clone()],
        links: Vec::new(),
        films: vec![a.clone(), b.clone()],
        chain: Some(Chain {
            entries: vec![a.clone(), s.clone(), b.clone()],
            links: vec![l("100", "1"), l("100", "101")],
            seams: vec!["1".into()],
        }),
        ..fate(anchor, None, true)
    };
    let first = page(&franchise("100"), Released);
    assert_eq!(
        first.caption,
        StripCaption::Position {
            position: 1,
            total: 2
        }
    );
    assert_eq!(ids(&first.view.after), ["1"]);
    assert_eq!(first.seams.after, Some(Seam::Into));
    let second = page(&franchise("101"), Released);
    assert_eq!(
        second.caption,
        StripCaption::Position {
            position: 2,
            total: 2
        }
    );
    assert_eq!(ids(&second.view.before), ["1"]);
    assert_eq!(second.seams.before, Some(Seam::From));
}

#[test]
fn reads_no_number_in_a_franchise_read_in_part_and_still_says_it_is_outside() {
    let p = page(&fate("12007", Some(heavens_feel()), false), Linked);
    assert!(p.outside);
    assert_eq!(p.caption, StripCaption::Partial);
    assert_eq!(p.eyebrow, None);
}

#[test]
fn is_not_outside_on_a_main_story_page() {
    let p = page(&fate("7658", None, true), Linked);
    assert!(!p.outside);
    assert_eq!(p.seams, NO_SEAMS);
}

proptest! {
    #[test]
    fn counts_every_own_title_of_a_line_and_never_a_seam(
        n in 2usize..=6,
        from in any::<bool>(),
        into in any::<bool>(),
        order in prop::sample::select(&[Linked, Released][..]),
    ) {
        let own: Vec<WatchEntry> = (0..n).map(|i| film(&format!("{}", 100 + i), &format!("{}-01-01", 2001 + i))).collect();
        let before = e("1", "2000-01-01", "TV");
        let after = e("2", "2010-01-01", "TV");
        let mut links: Vec<RunLink> = own.windows(2).map(|w| l(&w[0].id, &w[1].id)).collect();
        if from {
            links.insert(0, l("1", &own[0].id));
        }
        if into {
            links.push(l(&own[n - 1].id, "2"));
        }
        let mut entries = Vec::new();
        let mut seams = Vec::new();
        if from {
            entries.push(before.clone());
            seams.push("1".to_string());
        }
        entries.extend(own.iter().cloned());
        if into {
            entries.push(after.clone());
            seams.push("2".to_string());
        }
        let franchise = Franchise {
            entries: vec![before, after],
            links: vec![l("1", "2")],
            films: own.clone(),
            chain: Some(Chain { entries, links, seams }),
            ..fate("", None, true)
        };
        for (i, x) in own.iter().enumerate() {
            let f = Franchise { anchor: x.id.clone(), ..franchise.clone() };
            let p = page(&f, order);
            prop_assert_eq!(p.caption, StripCaption::Position { position: i + 1, total: n });
            prop_assert_eq!(p.seams.before, (i == 0 && from).then_some(Seam::From));
            prop_assert_eq!(p.seams.after, (i == n - 1 && into).then_some(Seam::Into));
        }
    }
}
