//! The decisions frontend/tests/acceptance/play-up-next.test.ts draws, asserted on the answers recorded for it
//! (`tests/fixtures/watch-order/scenarios/`): the Saga run, Saga →
//! Second Arc → Final Arc by its links with Origins, a prequel released
//! after Second Arc, before Saga; read for Second Arc (42), whose own
//! Kitsu detail says it finished airing with 12 episodes.
//!
//! A franchise that cannot be read fails the route, so there is no
//! recorded answer for it: `api::watch_order` tests
//! `a_franchise_that_cannot_be_read_fails_every_walking_route`.

use serde_json::{json, Value};

use super::scenarios::{answer, ids};

const SUITE: &str = "play-up-next";
const FINALE: &str = "the Up next card at a finale";

fn read(scenario: &str, episode: &str) -> Value {
    answer(
        SUITE,
        &format!("{FINALE} > {scenario}"),
        &format!("up-next 42 ep {episode}"),
    )
}

fn at_finale(scenario: &str) -> Value {
    read(scenario, "12")
}

/// The one entry an entry card names, with its kind.
fn one(a: &Value) -> (&str, &Value) {
    let card = &a["card"];
    assert_eq!(card["kind"], "entry", "not an entry card: {a}");
    let next = &card["card"];
    (next["kind"].as_str().expect("a kind"), &next["entry"])
}

#[test]
fn the_finale_of_second_arc_names_final_arc_having_aired() {
    // Also what the player moved to Second Arc from another show reads
    // (its inputs are this case's), and what the link scenario reads
    // before its link answers: no progress.
    let a = at_finale("names the next entry once the video ends, in the Skip Ending pill's place");
    let (kind, e) = one(&a);
    assert_eq!(kind, "one");
    assert_eq!(e["id"], "50");
    assert_eq!(e["title"], "Saga: Final Arc");
    assert_eq!(e["unaired"], false);
    assert_eq!(e["progress"], json!({ "kind": "none" }));
}

#[test]
fn an_episode_before_the_last_has_no_card() {
    // Episode 11 is what "shows no card before the entry's last episode"
    // reads too; episode 10 is read on the way through another scenario.
    let a = read(
        "drops the last episode's end with its stream on a switch",
        "11",
    );
    assert!(a["card"].is_null());
    let a = read(
        "keeps an episode's skip times when an earlier request for them fails late",
        "10",
    );
    assert!(a["card"].is_null());
}

#[test]
fn release_order_names_origins_after_second_arc() {
    let a = at_finale("follows the order setting");
    let (kind, e) = one(&a);
    assert_eq!(kind, "one");
    assert_eq!(e["id"], "5");
    assert_eq!(e["title"], "Saga: Origins");
}

#[test]
fn a_fork_lists_both_ways_on_in_start_order() {
    let a = at_finale("lists the ways on at a fork");
    assert_eq!(a["card"]["kind"], "entry");
    let next = &a["card"]["card"];
    assert_eq!(next["kind"], "choices");
    assert_eq!(ids(&next["entries"]), ["50", "60"]);
}

#[test]
fn a_next_entry_that_has_not_aired_is_named_unaired_with_its_start() {
    let a = at_finale("shows a next entry that has not aired with its date, and links nowhere");
    let (kind, e) = one(&a);
    assert_eq!(kind, "unaired");
    assert_eq!(e["id"], "50");
    assert_eq!(e["unaired"], true);
    assert_eq!(e["start"], "2099-01-01");
}

#[test]
fn a_franchise_read_in_part_has_no_card() {
    let a = at_finale(
        "shows no card where the franchise was read only in part, and the pill shows as before",
    );
    assert!(a["card"].is_null());
}

#[test]
fn the_next_entry_carries_the_viewers_progress_in_it() {
    // Also what the link scenario reads once its link answers: the row
    // moved onto Final Arc.
    let a = at_finale("gives the next entry's meta line the viewer's progress in it");
    let (_, e) = one(&a);
    assert_eq!(e["id"], "50");
    assert_eq!(
        e["progress"],
        json!({ "kind": "partway", "episode": 3.0, "total": 12 })
    );
}

#[test]
fn a_row_named_for_final_arc_after_kitsu_deleted_its_entry_counts_toward_final_arc() {
    let a = at_finale(
        "gives the next entry a row's progress when Kitsu deleted the entry the row recorded",
    );
    let (_, e) = one(&a);
    assert_eq!(e["id"], "50");
    assert_eq!(
        e["progress"],
        json!({ "kind": "partway", "episode": 3.0, "total": 12 })
    );
}

#[test]
fn an_entry_outside_the_main_story_follows_its_own_chain() {
    let a = at_finale("names the next title of the entry's own chain, not the main story's");
    let (kind, e) = one(&a);
    assert_eq!(kind, "one");
    assert_eq!(e["id"], "60");
    assert_eq!(e["title"], "Saga: The Movie");
}

#[test]
fn a_chain_that_leads_nowhere_offers_the_franchise_page() {
    let a = at_finale("offers the whole franchise at the end of a chain that leads nowhere");
    assert_eq!(a["card"]["kind"], "franchise");
    let c = &a["card"]["card"];
    assert_eq!(c["name"], "Saga");
    assert_eq!(c["id"], "42");
    assert_eq!(c["front"], "10");
    assert_eq!(c["summary"]["total"], 2);
    assert_eq!(c["summary"]["posters"], json!([]));
}

#[test]
fn a_chain_that_leads_into_the_main_story_goes_on_into_it() {
    let a = at_finale("goes on into the main story where the chain leads into it");
    let (kind, e) = one(&a);
    assert_eq!(kind, "one");
    assert_eq!(e["id"], "50");
}

#[test]
fn the_end_of_the_run_has_no_card() {
    let a = at_finale("shows no card at the end of the run");
    assert!(a["card"].is_null());
}

#[test]
fn an_episode_past_kitsus_count_is_a_finale_too() {
    // Episode 12 of a source listing 13 has a card as well (the first
    // case): the player holds it back while the entry has a next
    // episode, which is the renderer's to decide.
    let a = read(
        "shows on the episode past the count once nothing in the entry follows",
        "13",
    );
    assert_eq!(one(&a).1["id"], "50");
}

#[test]
fn an_extra_after_the_last_episode_is_a_finale() {
    let a = read(
        "shows on an extra after the last episode, which has no skip times to ask for",
        "12.5",
    );
    assert_eq!(one(&a).1["id"], "50");
}
