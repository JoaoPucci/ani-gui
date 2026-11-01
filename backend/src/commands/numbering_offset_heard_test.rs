//! Two walks of a show can overlap, and the one that heard its listing
//! first can repair the show's numbering stamps last. The stamps end
//! as the listing heard last repairs them; a repair from a listing
//! heard earlier than the last one taken writes nothing; two whose
//! hearings overlap name no former tag either listing gives a row.

use super::*;
use crate::commands::listing_tags::{Begun, Hearing};
use crate::scraper::provider::EpisodeRef;

const SHOW: &str = "hianime:the-show-1";

fn state() -> AppState {
    let dir = Box::leak(Box::new(tempfile::tempdir().expect("tmp")));
    let mut state = crate::commands::play::tests::state_with_proxy_origin();
    state.history_path = dir.path().join("history");
    state
}

fn row(number: u32, tag: Option<&str>, former: Option<&str>) -> EpisodeRef {
    EpisodeRef {
        id: u64::from(number),
        number,
        number2: tag.map(str::to_string),
        former_tag: former.map(str::to_string),
        title: None,
    }
}

/// Twelve episodes, then the stitched recap in slot 13 that an earlier
/// version tagged "24.5" and that is "12.5" now. `carries` adds a row
/// that is "24.5" itself, so the recap's former tag names it now.
fn listing(carries: bool) -> Vec<EpisodeRef> {
    let mut rows: Vec<EpisodeRef> = (1..=12).map(|n| row(n, None, None)).collect();
    rows.push(row(13, Some("12.5"), Some("24.5")));
    if carries {
        rows.push(row(14, Some("24.5"), None));
    }
    rows
}

/// The recap's stamp as the earlier version wrote it.
fn stamped(state: &AppState) {
    crate::commands::numbering_offset::put_display(state, SHOW, 0, 13, "24.5");
}

fn repair(state: &AppState, listing: &[EpisodeRef], heard: Hearing) {
    retag_from_listing(state, SHOW, listing, None, Some(heard));
}

/// Two hearings, the first wholly before the second.
fn one_after_the_other() -> (Hearing, Hearing) {
    let first = Begun::now().picked();
    (first, Begun::now().picked())
}

/// Two hearings whose spans overlap.
fn overlapping() -> (Hearing, Hearing) {
    let (a, b) = (Begun::now(), Begun::now());
    (a.picked(), b.picked())
}

fn former(state: &AppState) -> Option<(u32, String)> {
    former_stamp(state, SHOW)
}

#[test]
fn a_listing_heard_later_puts_right_the_former_tag_one_heard_earlier_kept() {
    let state = state();
    stamped(&state);
    let (earlier, later) = one_after_the_other();
    repair(&state, &listing(false), earlier);
    repair(&state, &listing(true), later);
    assert_eq!(display_stamp(&state, SHOW), Some((13, "12.5".into())));
    assert_eq!(former(&state), None, "a row is \"24.5\" now");
}

#[test]
fn a_listing_heard_earlier_that_repairs_last_leaves_the_later_one_s_former_tag() {
    // The later listing tags another row "24.5": the earlier one,
    // repairing after it, does not name the former tag again.
    let state = state();
    stamped(&state);
    let (earlier, later) = one_after_the_other();
    repair(&state, &listing(true), later);
    repair(&state, &listing(false), earlier);
    assert_eq!(former(&state), None);
}

#[test]
fn overlapping_hearings_name_no_former_tag_either_listing_gives_a_row() {
    for carrying_first in [false, true] {
        let state = state();
        stamped(&state);
        let (a, b) = overlapping();
        repair(&state, &listing(carrying_first), a);
        repair(&state, &listing(!carrying_first), b);
        assert_eq!(
            display_stamp(&state, SHOW),
            Some((13, "12.5".into())),
            "carrying first: {carrying_first}"
        );
        assert_eq!(former(&state), None, "carrying first: {carrying_first}");
    }
}

/// The show's episode 14 watched before rows recorded the app's own
/// number: a history row holding slot 15, past the recap, which reads
/// through the show's stamp once a listing moves it there.
fn legacy_row(state: &AppState) {
    let row = crate::history::parse(&format!("15\t{SHOW}\tThe Show\n"))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
}

/// [`listing`] with slots 14 and 15 after the recap, slot 15 tagged
/// `fifteenth`.
fn listing_past_the_recap(fifteenth: &str) -> Vec<EpisodeRef> {
    let mut rows = listing(false);
    rows.push(row(14, Some("13"), None));
    rows.push(row(15, Some(fifteenth), None));
    rows
}

#[test]
fn a_listing_heard_later_puts_the_stamp_where_it_reads_the_row() {
    let state = state();
    stamped(&state);
    legacy_row(&state);
    let (earlier, later) = one_after_the_other();
    repair(&state, &listing_past_the_recap("13.5"), earlier);
    repair(&state, &listing_past_the_recap("14"), later);
    assert_eq!(display_stamp(&state, SHOW), Some((15, "14".into())));
}

#[test]
fn a_listing_heard_earlier_landing_inside_a_later_one_s_repair_leaves_its_stamp() {
    // The later listing's repair reads the store before it decides,
    // under the store's locks; the earlier listing's repair lands just
    // before that read. Its decision is made against the stamps as they
    // stand under the locks, so the later listing's stamp stands. (A
    // repair that read the store twice, deciding between the reads, let
    // the earlier one land between its steps.)
    let state = std::rc::Rc::new(state());
    stamped(&state);
    legacy_row(&state);
    let (earlier, later) = one_after_the_other();
    let inside = std::rc::Rc::clone(&state);
    let landed = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = std::rc::Rc::clone(&landed);
    super::super::read_tap::before_read(1, move || {
        repair(&inside, &listing_past_the_recap("13.5"), earlier);
        seen.set(true);
    });
    repair(&state, &listing_past_the_recap("14"), later);
    super::super::read_tap::clear();
    assert!(
        landed.get(),
        "the earlier listing's repair landed inside the later one's"
    );
    assert_eq!(display_stamp(&state, SHOW), Some((15, "14".into())));
}

/// [`listing`] as a provider that stopped stitching lists it: the recap
/// is "12.5" with no tag an earlier version gave it otherwise.
fn listing_without_a_former_tag() -> Vec<EpisodeRef> {
    let mut rows: Vec<EpisodeRef> = (1..=12).map(|n| row(n, None, None)).collect();
    rows.push(row(13, Some("12.5"), None));
    rows
}

#[test]
fn a_later_listing_that_repairs_nothing_leaves_an_earlier_one_s_repair_whichever_finishes_first() {
    for earlier_first in [true, false] {
        let state = state();
        stamped(&state);
        let (earlier, later) = one_after_the_other();
        let repairs = [
            (listing(false), earlier),
            (listing_without_a_former_tag(), later),
        ];
        let order: [usize; 2] = if earlier_first { [0, 1] } else { [1, 0] };
        for i in order {
            repair(&state, &repairs[i].0, repairs[i].1);
        }
        assert_eq!(
            display_stamp(&state, SHOW),
            Some((13, "12.5".into())),
            "earlier first: {earlier_first}"
        );
        assert_eq!(
            former(&state),
            Some((13, "24.5".into())),
            "earlier first: {earlier_first}"
        );
    }
}

/// [`listing`] with the recap tagged `now`.
fn listing_tagging_the_recap(now: &str) -> Vec<EpisodeRef> {
    let mut rows: Vec<EpisodeRef> = (1..=12).map(|n| row(n, None, None)).collect();
    rows.push(row(13, Some(now), Some("24.5")));
    rows
}

#[test]
fn three_overlapping_listings_that_disagree_leave_the_stamp_as_it_was_in_any_order() {
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for order in orders {
        let state = state();
        stamped(&state);
        let begun = [Begun::now(), Begun::now(), Begun::now()];
        let heard = begun.map(Begun::picked);
        let listings = ["12.5", "12.25", "12.75"].map(listing_tagging_the_recap);
        for i in order {
            repair(&state, &listings[i], heard[i]);
        }
        assert_eq!(
            display_stamp(&state, SHOW),
            Some((13, "24.5".into())),
            "order {order:?}"
        );
        assert_eq!(former(&state), None, "order {order:?}");
    }
}

#[test]
fn a_listing_heard_after_one_and_alongside_another_that_agrees_stands_in_any_order() {
    // The recap tagged "12.25" by a listing heard first; "12.5" by one
    // heard wholly after it, and by one heard alongside both. The first
    // is outranked, and the two left agree.
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for order in orders {
        let state = state();
        stamped(&state);
        let (first, alongside) = (Begun::now(), Begun::now());
        let first = first.picked();
        let after = Begun::now().picked();
        let alongside = alongside.picked();
        let repairs = [
            (listing_tagging_the_recap("12.25"), first),
            (listing_tagging_the_recap("12.5"), after),
            (listing_tagging_the_recap("12.5"), alongside),
        ];
        for i in order {
            repair(&state, &repairs[i].0, repairs[i].1);
        }
        assert_eq!(
            display_stamp(&state, SHOW),
            Some((13, "12.5".into())),
            "order {order:?}"
        );
        assert_eq!(former(&state), Some((13, "24.5".into())), "order {order:?}");
    }
}
