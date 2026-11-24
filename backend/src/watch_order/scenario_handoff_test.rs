//! The decisions `frontend/tests/acceptance/continue-watching-handoff.test.ts`
//! draws, asserted on the answers recorded for it
//! (`tests/fixtures/watch-order/scenarios/continue-watching-handoff.json`):
//! First Night (10) → Second Night (20) by their links, Other Night (25)
//! a second way on from First Night in the fork; Tide Bell Spring (30) →
//! Summer (40) the control series, whose hand-off always moves on.
//!
//! Two decisions make a Continue card move on: the hand-off route's
//! (`handoff <id>` cases: the entry after the one played through, held
//! back where the resume lookup names a row for it, or cannot be read),
//! and the rail's (`rail_view`: which rows ask, and which show the
//! hand-off they read, over the played set). The rail is pure and Home
//! reads it again as it learns things, so its decisions are asserted on
//! requests built here the way the scenarios' Home builds them, carrying
//! the hand-offs the backend recorded for those scenarios.
//! `continue-watching-handoff-share.test.ts` asserts no decision: its
//! walks never answer.

use serde_json::{json, Value};

use super::scenarios::answer;
use super::wire::{rail_view, RailRequest};
use super::WatchOrder;

const SUITE: &str = "continue-watching-handoff";
const AFTER: &str = "Continue Watching after a finale";

/// The hand-off route's answer for `id` in scenario `scenario`.
fn handoff(scenario: &str, id: &str) -> Value {
    answer(
        SUITE,
        &format!("{AFTER} > {scenario}"),
        &format!("handoff {id}"),
    )["handoff"]
        .clone()
}

/// `(finished, next)` of a hand-off answer.
fn moves(h: &Value) -> (&str, &str) {
    (
        h["finished"]["id"].as_str().expect("finished id"),
        h["next"]["id"].as_str().expect("next id"),
    )
}

const MOVES_ON: &str = "moves the finished show's card on to the next show's page";
const NO_RECORDED_ID: &str =
    "keeps the ordinary card when the next show was played from a row with no recorded id";
const KITSU_DELETED: &str = "keeps the ordinary card when the next show was played from a row whose recorded entry Kitsu deleted";

/// One rail row: `(row id, matched Kitsu id, episode, probe count, left part-way)`.
type Row<'a> = (&'a str, Option<&'a str>, &'a str, Option<u32>, bool);

/// The rail request Home sends: `rows` in order, each matched to a
/// finished 13-episode show where it is matched; `history` every row
/// with its recorded id; `matches` each row's match (a row still being
/// matched left out); `series` the hand-offs read so far.
fn rail(
    rows: &[Row<'_>],
    history: &[(&str, Option<&str>)],
    matches: &[(&str, Option<&str>)],
    series: &[(&str, Value)],
) -> Value {
    let rows: Vec<Value> = rows
        .iter()
        .map(|(row_id, matched, ep_no, playable, part_way)| {
            json!({
                "row_id": row_id,
                "matched": matched.map(|id| json!({
                    "id": id, "status": "finished", "episode_count": 13
                })),
                "ep_no": ep_no,
                "playable_count": playable,
                "left_part_way": part_way,
            })
        })
        .collect();
    let history: Vec<Value> = history
        .iter()
        .map(|(id, k)| json!({ "id": id, "kitsu_id": k }))
        .collect();
    let matches: serde_json::Map<String, Value> = matches
        .iter()
        .map(|(id, m)| ((*id).to_owned(), json!(m)))
        .collect();
    let series: serde_json::Map<String, Value> = series
        .iter()
        .map(|(k, h)| ((*k).to_owned(), h.clone()))
        .collect();
    let req = json!({ "rows": rows, "history": history, "matches": matches, "series": series });
    let req: RailRequest = serde_json::from_value(req).expect("a rail request");
    serde_json::to_value(rail_view(&req)).expect("a rail view")
}

/// The row ids a rail answer shows a hand-off for.
fn shown(view: &Value) -> Vec<&str> {
    view["handoffs"]
        .as_object()
        .expect("handoffs")
        .keys()
        .map(String::as_str)
        .collect()
}

// ---- the hand-off route ------------------------------------------------

#[test]
fn first_night_played_through_moves_on_to_second_night() {
    let h = handoff(MOVES_ON, "10");
    assert_eq!(moves(&h), ("10", "20"));
    // The card names both shows by their whole titles.
    assert_eq!(h["finished"]["title"], "Arc of the Moon: First Night");
    assert_eq!(h["next"]["title"], "Arc of the Moon: Second Night");
}

#[test]
fn a_show_outside_the_main_story_moves_on_along_its_own_chain() {
    let h = handoff(
        "moves a show outside the main story on along its own chain",
        "10",
    );
    assert_eq!(moves(&h), ("10", "20"));
}

#[test]
fn no_hand_off_once_the_next_show_has_a_row_of_its_own() {
    let s = "keeps both ordinary cards once the next show has a row of its own";
    assert!(handoff(s, "10").is_null());
    // The control series moves on all the same.
    assert_eq!(moves(&handoff(s, "30")), ("30", "40"));
}

#[test]
fn the_control_series_moves_on_while_the_first_series_cannot_be_read() {
    // The first series' read fails (503): it records no case, and the
    // row keeps its ordinary card; the control series still moves on.
    let h = handoff(
        "keeps the ordinary card, with no error, when the series cannot be read",
        "30",
    );
    assert_eq!(moves(&h), ("30", "40"));
}

#[test]
fn no_hand_off_where_the_show_leads_two_ways_as_linked() {
    let h = handoff(
        "keeps the ordinary card where the show leads two ways as linked",
        "10",
    );
    assert!(h.is_null());
}

#[test]
fn as_released_the_fork_moves_on_to_the_earlier_second_night() {
    let h = handoff(
        "moves on to the next show by release date when the setting says so",
        "10",
    );
    assert_eq!(moves(&h), ("10", "20"));
}

#[test]
fn the_route_moves_on_past_a_row_with_no_recorded_id_which_only_the_rail_knows() {
    // The resume lookup names no row for 20 (the row recorded no id),
    // so the route hands off; the rail holds it back
    // (`a_row_matched_by_its_title_to_the_next_show_holds_the_hand_off_back`).
    assert_eq!(moves(&handoff(NO_RECORDED_ID, "10")), ("10", "20"));
}

#[test]
fn the_route_moves_on_past_a_row_whose_recorded_entry_kitsu_deleted() {
    assert_eq!(moves(&handoff(KITSU_DELETED, "10")), ("10", "20"));
    assert_eq!(moves(&handoff(KITSU_DELETED, "30")), ("30", "40"));
}

#[test]
fn no_hand_off_where_the_resume_lookup_names_a_row_for_the_next_show() {
    let s = "keeps the ordinary card when the next show's page would resume from a row";
    assert!(handoff(s, "10").is_null());
    assert_eq!(moves(&handoff(s, "30")), ("30", "40"));
}

#[test]
fn removing_the_row_the_lookup_named_lets_the_hand_off_move_on() {
    let s = "moves on once the row the next show's page would resume from is removed";
    assert!(handoff(s, "10").is_null());
    // Once the row is gone the lookup names First Night and the control
    // show only: the same inputs as the no-recorded-id scenario's read,
    // which records them, and moves on.
    assert_eq!(moves(&handoff(NO_RECORDED_ID, "10")), ("10", "20"));
}

#[test]
fn both_series_move_on_before_another_row_is_removed() {
    let s = "keeps a moved-on card in place while another row is removed";
    assert_eq!(moves(&handoff(s, "10")), ("10", "20"));
    assert_eq!(moves(&handoff(s, "30")), ("30", "40"));
    // After the removal the reads are the no-recorded-id scenario's,
    // which still move on.
    assert_eq!(moves(&handoff(NO_RECORDED_ID, "10")), ("10", "20"));
}

#[test]
fn no_hand_off_for_either_series_when_the_resume_lookup_cannot_be_read() {
    let s = "keeps the ordinary card when the resume lookup cannot be read";
    assert!(handoff(s, "10").is_null());
    assert!(handoff(s, "30").is_null());
}

#[test]
fn no_hand_off_from_a_series_read_only_in_part() {
    let h = handoff(
        "keeps the ordinary card when the series was read only in part",
        "10",
    );
    assert!(h.is_null());
}

#[test]
fn settings_that_cannot_be_read_read_as_linked() {
    // "moves on in the default order when the settings cannot be read":
    // the route reads the order itself, falling back to the default
    // config where the settings cannot be read; the scenario's read then
    // has the first scenario's inputs (linked), which move on.
    let fallback = crate::config::Config::default();
    assert_eq!(WatchOrder::of(&fallback.watch_order), WatchOrder::Linked);
    assert_eq!(moves(&handoff(MOVES_ON, "10")), ("10", "20"));
}

// ---- the rail ----------------------------------------------------------

#[test]
fn a_row_at_its_finale_asks_and_shows_the_hand_off_in_its_place() {
    let h = handoff(MOVES_ON, "10");
    let v = rail(
        &[("p-first", Some("10"), "13", Some(13), false)],
        &[("p-first", Some("10"))],
        &[("p-first", Some("10"))],
        &[("10", h.clone())],
    );
    assert_eq!(v["asks"], json!({ "p-first": "10" }));
    assert_eq!(v["handoffs"], json!({ "p-first": h }));
}

#[test]
fn a_row_not_matched_yet_asks_for_nothing() {
    let v = rail(
        &[("p-first", None, "13", None, false)],
        &[("p-first", Some("10"))],
        &[],
        &[],
    );
    assert_eq!(v["asks"], json!({ "p-first": null }));
}

#[test]
fn a_row_short_of_its_finale_asks_for_no_series() {
    let v = rail(
        &[("p-first", Some("10"), "12", Some(13), false)],
        &[("p-first", Some("10"))],
        &[("p-first", Some("10"))],
        &[],
    );
    assert_eq!(v["asks"], json!({ "p-first": null }));
}

#[test]
fn a_finale_left_part_way_asks_for_no_series() {
    let v = rail(
        &[("p-first", Some("10"), "13", Some(13), true)],
        &[("p-first", Some("10"))],
        &[("p-first", Some("10"))],
        &[],
    );
    assert_eq!(v["asks"], json!({ "p-first": null }));
}

#[test]
fn an_episode_the_source_has_past_kitsus_count_takes_the_hand_off_back() {
    // The probe answers after the hand-off was read: the row asks for no
    // series any more, and the card offers episode 14 again.
    let v = rail(
        &[("p-first", Some("10"), "13", Some(14), false)],
        &[("p-first", Some("10"))],
        &[("p-first", Some("10"))],
        &[("10", handoff(MOVES_ON, "10"))],
    );
    assert_eq!(v["asks"], json!({ "p-first": null }));
    assert_eq!(v["handoffs"], json!({}));
}

#[test]
fn a_row_matched_by_its_title_to_the_next_show_holds_the_hand_off_back() {
    let rows: [Row<'_>; 3] = [
        ("p-second", Some("20"), "2", Some(13), false),
        ("p-first", Some("10"), "13", Some(13), false),
        ("p-control", Some("30"), "13", Some(13), false),
    ];
    let history = [
        ("p-second", None),
        ("p-first", Some("10")),
        ("p-control", Some("30")),
    ];
    let series = [
        ("10", handoff(NO_RECORDED_ID, "10")),
        ("30", handoff(KITSU_DELETED, "30")),
    ];
    // While the row is still being matched the played set is not known,
    // so no card moves on, not even the control's.
    let pending = rail(
        &rows,
        &history,
        &[("p-first", Some("10")), ("p-control", Some("30"))],
        &series,
    );
    assert_eq!(pending["handoffs"], json!({}));
    // Matched to Second Night, it counts as played.
    let matched = rail(
        &rows,
        &history,
        &[
            ("p-second", Some("20")),
            ("p-first", Some("10")),
            ("p-control", Some("30")),
        ],
        &series,
    );
    assert_eq!(matched["asks"]["p-second"], Value::Null);
    assert_eq!(shown(&matched), ["p-control"]);
}

#[test]
fn a_row_whose_recorded_entry_kitsu_deleted_counts_as_the_show_it_is_matched_to() {
    let v = rail(
        &[
            ("p-second", Some("20"), "2", Some(13), false),
            ("p-first", Some("10"), "13", Some(13), false),
            ("p-control", Some("30"), "13", Some(13), false),
        ],
        &[
            ("p-second", Some("99")),
            ("p-first", Some("10")),
            ("p-control", Some("30")),
        ],
        &[
            ("p-second", Some("20")),
            ("p-first", Some("10")),
            ("p-control", Some("30")),
        ],
        &[
            ("10", handoff(KITSU_DELETED, "10")),
            ("30", handoff(KITSU_DELETED, "30")),
        ],
    );
    assert_eq!(shown(&v), ["p-control"]);
}

#[test]
fn a_row_recording_the_next_show_holds_the_hand_off_back_in_the_rail_too() {
    // The route already holds it back (the lookup names that row); the
    // rail's played set would too, were a hand-off read.
    let v = rail(
        &[
            ("p-second", Some("20"), "3", Some(13), false),
            ("p-first", Some("10"), "13", Some(13), false),
        ],
        &[("p-second", Some("20")), ("p-first", Some("10"))],
        &[("p-second", Some("20")), ("p-first", Some("10"))],
        &[("10", handoff(MOVES_ON, "10"))],
    );
    assert_eq!(v["handoffs"], json!({}));
}

#[test]
fn only_the_first_row_in_the_rail_moves_on_to_an_entry() {
    let h = handoff(MOVES_ON, "10");
    let v = rail(
        &[
            ("p-first", Some("10"), "13", Some(13), false),
            ("p-again", Some("10"), "13", Some(13), false),
        ],
        &[("p-first", Some("10")), ("p-again", Some("10"))],
        &[("p-first", Some("10")), ("p-again", Some("10"))],
        &[("10", h.clone())],
    );
    assert_eq!(v["asks"], json!({ "p-first": "10", "p-again": "10" }));
    assert_eq!(v["handoffs"], json!({ "p-first": h }));
}

#[test]
fn a_series_that_could_not_be_read_shows_no_hand_off() {
    let v = rail(
        &[
            ("p-first", Some("10"), "13", Some(13), false),
            ("p-control", Some("30"), "13", Some(13), false),
        ],
        &[("p-first", Some("10")), ("p-control", Some("30"))],
        &[("p-first", Some("10")), ("p-control", Some("30"))],
        &[("10", Value::Null), ("30", Value::Null)],
    );
    assert_eq!(v["asks"], json!({ "p-first": "10", "p-control": "30" }));
    assert_eq!(v["handoffs"], json!({}));
}
