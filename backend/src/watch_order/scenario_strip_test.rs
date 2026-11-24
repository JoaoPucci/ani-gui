//! The decisions `frontend/tests/acceptance/detail-watch-order-strip.test.ts`
//! draws, asserted on the answers recorded for it
//! (`tests/fixtures/watch-order/scenarios/detail-watch-order-strip.json`):
//! the Saga run, Origins → Saga → Second Arc → Final Arc by its links,
//! Origins released between Second Arc and Final Arc, read for Second
//! Arc (42).

use serde_json::{json, Value};

use super::scenarios::{answer, ids};

const SUITE: &str = "detail-watch-order-strip";
const STRIP: &str = "detail route — the watch-order strip";
const LINK: &str = "detail route — the franchise link without a strip";

fn strip(scenario: &str) -> Value {
    answer(SUITE, &format!("{STRIP} > {scenario}"), "show 42")
}

fn alone(scenario: &str) -> Value {
    answer(SUITE, &format!("{LINK} > {scenario}"), "show 42")
}

#[test]
fn second_arc_comes_after_saga_and_before_final_arc_third_of_four() {
    let a = strip("shows what comes before and after, and the place in the run");
    let s = &a["strip"];
    assert_eq!(a["order"], "linked");
    assert_eq!(s["kind"], "run");
    assert_eq!(ids(&s["before"]), ["10"]);
    assert_eq!(ids(&s["after"]), ["50"]);
    assert_eq!(s["after"][0]["title"], "Saga: Final Arc");
    assert_eq!(
        s["caption"],
        json!({ "kind": "position", "position": 3, "total": 4 })
    );
    assert_eq!(s["eyebrow"], json!({ "position": 3, "total": 4 }));
    assert!(a["link"].is_null());
}

#[test]
fn the_strips_franchise_link_counts_the_four_titles_of_the_main_story() {
    let a = strip("shows what comes before and after, and the place in the run");
    assert_eq!(a["strip"]["stack"]["summary"]["main"], 4);
    assert_eq!(a["strip"]["stack"]["front"], "10");
    assert_eq!(a["strip"]["complete"], true);
}

#[test]
fn names_origins_being_watched_elsewhere_in_the_run_and_leaves_it_out_of_similar() {
    let a = strip("keeps the franchise link beside the card for the entry being watched");
    let s = &a["strip"];
    assert_eq!(s["watching"]["entry"]["id"], "5");
    assert_eq!(s["watching"]["entry"]["title"], "Saga: Origins");
    assert_eq!(s["watching"]["episode"], 3.0);
    assert!(ids(&s["before"])
        .iter()
        .chain(&ids(&s["after"]))
        .all(|id| *id != "5"));
    assert!(s["similar_hidden"]
        .as_array()
        .expect("ids")
        .contains(&json!("5")));
}

#[test]
fn release_order_puts_origins_after_second_arc_second_of_four() {
    let a = strip("takes its neighbours and its place from the order setting");
    let s = &a["strip"];
    assert_eq!(a["order"], "released");
    assert_eq!(ids(&s["before"]), ["10"]);
    assert_eq!(ids(&s["after"]), ["5"]);
    assert_eq!(s["eyebrow"], json!({ "position": 2, "total": 4 }));
}

#[test]
fn a_row_named_for_origins_after_kitsu_deleted_its_entry_counts_toward_origins() {
    let a = strip(
        "gives a row's progress to the entry the backend names it for, when Kitsu deleted the recorded one",
    );
    assert_eq!(a["strip"]["watching"]["entry"]["id"], "5");
    assert_eq!(a["strip"]["watching"]["episode"], 3.0);
}

#[test]
fn similar_titles_leaves_out_the_whole_franchise_and_nothing_else() {
    let a = strip("leaves the whole franchise out of Similar titles, and only the franchise");
    let hidden: Vec<&str> = a["strip"]["similar_hidden"]
        .as_array()
        .expect("ids")
        .iter()
        .map(|v| v.as_str().expect("id"))
        .collect();
    let mut hidden = hidden;
    hidden.sort_unstable();
    assert_eq!(hidden, ["10", "5", "50", "60"]);
}

#[test]
fn history_that_cannot_be_read_leaves_the_strip_without_progress() {
    let a = strip("shows the strip without progress when the history cannot be read");
    let s = &a["strip"];
    assert_eq!(ids(&s["before"]), ["10"]);
    assert!(s["watching"].is_null());
    for e in s["before"]
        .as_array()
        .into_iter()
        .chain(s["after"].as_array())
        .flatten()
    {
        assert_eq!(e["progress"], json!({ "kind": "none" }));
    }
}

#[test]
fn a_franchise_read_in_part_claims_no_place_total_or_end() {
    let a = strip("claims no place, total or end for a franchise the backend read only in part");
    let s = &a["strip"];
    assert_eq!(ids(&s["before"]), ["10"]);
    assert_eq!(ids(&s["after"]), Vec::<&str>::new());
    assert_eq!(s["caption"], json!({ "kind": "partial" }));
    assert!(s["eyebrow"].is_null());
    assert_eq!(s["complete"], false);
}

#[test]
fn a_side_story_shows_its_parent_stories_and_nothing_after_it() {
    let a = strip("names a side story's parent stories, and nothing after it");
    let s = &a["strip"];
    assert_eq!(s["kind"], "side");
    assert_eq!(ids(&s["before"]), ["10", "50"]);
    assert_eq!(ids(&s["after"]), Vec::<&str>::new());
    assert_eq!(s["caption"], json!({ "kind": "side" }));
    assert!(s["eyebrow"].is_null());
}

#[test]
fn a_show_with_no_run_has_no_strip() {
    let a = answer(
        SUITE,
        &format!(
            "{STRIP} > keeps no strip or place from the show before, moving to one without a run"
        ),
        "show 99",
    );
    assert!(a["strip"].is_null());
    assert!(a["link"].is_null());
}

#[test]
fn a_show_that_is_its_main_story_alone_links_to_the_franchise_page_with_its_film_and_side_story() {
    let a = alone("links to the franchise page from a show that is its main story alone");
    assert!(a["strip"].is_null());
    let l = &a["link"];
    assert_eq!(l["complete"], true);
    assert_eq!(l["stack"]["summary"]["main"], 1);
    assert_eq!(l["stack"]["summary"]["films"], 1);
    assert_eq!(l["stack"]["summary"]["sides"], 1);
    assert_eq!(l["similar_hidden"], json!(["60", "61"]));
}

#[test]
fn the_lone_link_carries_a_franchise_read_in_part() {
    let a = alone("links there without counts from a franchise read in part");
    assert!(a["strip"].is_null());
    assert_eq!(a["link"]["complete"], false);
}

#[test]
fn a_show_that_is_the_franchise_alone_gets_no_link_read_whole_or_in_part() {
    for read in ["whole", "in part"] {
        let a = alone(&format!(
            "gives no link for a show that is the franchise alone (read {read})"
        ));
        assert!(a["strip"].is_null());
        assert!(a["link"].is_null());
    }
}
