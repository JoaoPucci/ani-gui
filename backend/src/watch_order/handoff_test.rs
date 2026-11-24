//! Continue Watching's hand-off (ported from the renderer's
//! `handoff.test.ts`).

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

use super::fixtures::{payload, run};
use super::handoff::{
    handoff_ask, handoff_from, played_ids, rail_handoffs, unclaimed_handoff, AskRow, Handoff,
    HandoffMatch, PlayedRow,
};
use super::progress::history_episode;
use super::run::run_of;
use super::strategies::franchise_arb;
use super::up_next::{is_finale, up_next_for, FinaleSelf, UpNext};
use super::{Chain, Franchise, WatchEntry, WatchOrder};

use WatchOrder::{Linked, Released};

const TODAY: &str = "2026-11-02";

fn finished(episodes: Option<u32>) -> HandoffMatch {
    HandoffMatch {
        id: "43078".into(),
        status: Some("finished".into()),
        episode_count: episodes,
    }
}

fn ask(
    matched: Option<HandoffMatch>,
    ep_no: &str,
    playable: Option<u32>,
    left_part_way: bool,
) -> Option<String> {
    handoff_ask(&AskRow {
        row_id: "r".into(),
        matched,
        ep_no: ep_no.into(),
        playable_count: playable,
        left_part_way,
    })
}

#[test]
fn asks_for_the_series_of_a_finale_played_through_by_the_rows_match() {
    assert_eq!(
        ask(Some(finished(Some(13))), "13", None, false).as_deref(),
        Some("43078")
    );
    // A row past Kitsu's count is at the end too.
    assert_eq!(
        ask(Some(finished(Some(13))), "14", None, false).as_deref(),
        Some("43078")
    );
}

#[test]
fn asks_nothing_for_a_row_short_of_the_finale() {
    for ep in ["12", "12.5", "", "abc"] {
        assert_eq!(ask(Some(finished(Some(13))), ep, None, false), None, "{ep}");
    }
}

#[test]
fn asks_nothing_for_an_entry_still_airing_of_unknown_length_or_not_matched() {
    let airing = HandoffMatch {
        status: Some("current".into()),
        ..finished(Some(13))
    };
    assert_eq!(ask(Some(airing), "13", None, false), None);
    let unknown = HandoffMatch {
        status: None,
        ..finished(Some(13))
    };
    assert_eq!(ask(Some(unknown), "13", None, false), None);
    assert_eq!(ask(Some(finished(None)), "13", None, false), None);
    assert_eq!(ask(None, "13", None, false), None);
}

#[test]
fn waits_for_the_players_own_last_step_where_the_source_lists_more_than_kitsu() {
    assert_eq!(ask(Some(finished(Some(13))), "13", Some(14), false), None);
    assert_eq!(
        ask(Some(finished(Some(13))), "14", Some(14), false).as_deref(),
        Some("43078")
    );
    // A source that lists fewer does not make an earlier episode the finale.
    assert_eq!(ask(Some(finished(Some(13))), "12", Some(12), false), None);
    assert_eq!(
        ask(Some(finished(Some(13))), "13", Some(12), false).as_deref(),
        Some("43078")
    );
}

#[test]
fn asks_nothing_when_the_finale_was_left_part_way() {
    assert_eq!(ask(Some(finished(Some(13))), "13", None, true), None);
}

fn none() -> BTreeSet<String> {
    BTreeSet::new()
}

#[test]
fn moves_on_to_the_one_entry_that_comes_next() {
    let got = handoff_from(&payload("bleach", "43078"), "43078", Linked, TODAY, &none())
        .expect("a hand-off");
    assert_eq!(
        (got.finished.id.as_str(), got.next.id.as_str()),
        ("43078", "46903")
    );
}

#[test]
fn keeps_the_ordinary_card_for_a_franchise_read_only_in_part() {
    let mut part = payload("bleach", "43078");
    part.complete = false;
    for order in [Linked, Released] {
        assert_eq!(handoff_from(&part, "43078", order, TODAY, &none()), None);
    }
}

#[test]
fn keeps_the_ordinary_card_once_the_next_entry_has_a_history_row_of_its_own() {
    let played = ["46903".to_string()].into_iter().collect();
    assert_eq!(
        handoff_from(&payload("bleach", "43078"), "43078", Linked, TODAY, &played),
        None
    );
}

#[test]
fn follows_the_order_setting_a_fork_as_linked_one_entry_as_released() {
    assert_eq!(
        handoff_from(&payload("fate", "7658"), "7658", Linked, TODAY, &none()),
        None
    );
    let got = handoff_from(&payload("fate", "7658"), "7658", Released, TODAY, &none())
        .expect("a hand-off");
    assert_eq!(got.next.id, "9718");
}

#[test]
fn keeps_the_ordinary_card_before_an_entry_not_aired_yet_and_at_the_end_and_for_another_series() {
    assert_eq!(
        handoff_from(
            &payload("monogatari", "48434"),
            "48434",
            Linked,
            TODAY,
            &none()
        ),
        None
    );
    assert_eq!(
        handoff_from(&payload("bleach", "49444"), "49444", Linked, TODAY, &none()),
        None
    );
    assert_eq!(
        handoff_from(&payload("bleach", "244"), "43078", Linked, TODAY, &none()),
        None
    );
    let lone = payload("bleach", "999");
    assert_eq!(handoff_from(&lone, "999", Linked, TODAY, &none()), None);
}

fn chained(seam: &str, chain: (Vec<WatchEntry>, Vec<(String, String)>)) -> Franchise {
    let r = run("bleach");
    let main = r
        .entries
        .iter()
        .find(|e| e.id == seam)
        .cloned()
        .expect("a member");
    Franchise {
        entries: vec![main],
        links: Vec::new(),
        chain: Some(Chain {
            entries: chain.0,
            links: chain.1,
            seams: vec![seam.to_string()],
        }),
        ..payload("bleach", &r.entries[1].id)
    }
}

#[test]
fn moves_an_entry_outside_the_main_story_on_along_its_own_chain() {
    let r = run("bleach");
    let (a, b, c) = (
        r.entries[0].clone(),
        r.entries[1].clone(),
        r.entries[2].clone(),
    );
    let links = vec![(a.id.clone(), b.id.clone()), (b.id.clone(), c.id.clone())];
    let f = chained(&a.id.clone(), (vec![a, b.clone(), c.clone()], links));
    assert_eq!(
        handoff_from(&f, &b.id, Linked, TODAY, &none()).map(|h| h.next.id),
        Some(c.id)
    );
}

#[test]
fn moves_on_into_the_main_story_title_a_chain_leads_into_and_nowhere_at_its_dead_end() {
    let r = run("bleach");
    let (a, b, c) = (
        r.entries[0].clone(),
        r.entries[1].clone(),
        r.entries[2].clone(),
    );
    let links = vec![(a.id.clone(), b.id.clone()), (b.id.clone(), c.id.clone())];
    let into = chained(&c.id, (vec![a.clone(), b.clone(), c.clone()], links));
    assert_eq!(
        handoff_from(&into, &b.id, Linked, TODAY, &none()).map(|h| h.next.id),
        Some(c.id.clone())
    );
    // A seam the viewer already played keeps the ordinary card.
    let played = [c.id.clone()].into_iter().collect();
    assert_eq!(handoff_from(&into, &b.id, Linked, TODAY, &played), None);
    let dead_end = Franchise {
        chain: Some(Chain {
            entries: vec![a.clone(), b.clone()],
            links: vec![(a.id.clone(), b.id.clone())],
            seams: Vec::new(),
        }),
        ..into
    };
    assert_eq!(handoff_from(&dead_end, &b.id, Linked, TODAY, &none()), None);
}

fn played_row(id: &str, kitsu_id: Option<&str>) -> PlayedRow {
    PlayedRow {
        id: id.into(),
        kitsu_id: kitsu_id.map(Into::into),
    }
}

#[test]
fn played_ids_hold_each_rows_recorded_kitsu_id_and_its_matchs() {
    let rows = [
        played_row("a", Some("1")),
        played_row("b", None),
        played_row("c", None),
        played_row("d", None),
        played_row("e", Some("5")),
    ];
    let matches: BTreeMap<String, Option<String>> = [
        ("a", Some("9")),
        ("b", Some("2")),
        ("c", None),
        ("d", None),
        ("e", Some("5")),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.map(Into::into)))
    .collect();
    // Row a recorded 1, which Kitsu no longer has: its card shows 9.
    let got: Vec<String> = played_ids(&rows, &matches)
        .expect("known")
        .into_iter()
        .collect();
    assert_eq!(got, ["1", "2", "5", "9"]);
}

#[test]
fn played_ids_are_not_known_while_any_row_is_still_being_matched() {
    let empty = BTreeMap::new();
    assert_eq!(
        played_ids(&[played_row("a", Some("1")), played_row("b", None)], &empty),
        None
    );
    // A recorded entry may be gone and matched again as another.
    assert_eq!(played_ids(&[played_row("a", Some("1"))], &empty), None);
    let unmatched = [("a".to_string(), None)].into_iter().collect();
    let got: Vec<String> = played_ids(&[played_row("a", Some("1"))], &unmatched)
        .expect("known")
        .into_iter()
        .collect();
    assert_eq!(got, ["1"]);
}

fn plain(id: &str) -> WatchEntry {
    WatchEntry {
        id: id.into(),
        title: format!("T{id}"),
        subtype: Some("TV".into()),
        start: Some("2020-01-01".into()),
        episodes: Some(12),
        status: Some("finished".into()),
        poster: None,
        cover: None,
    }
}

fn to(next: &str, from: &str) -> Handoff {
    Handoff {
        finished: plain(from),
        next: plain(next),
    }
}

fn asks(list: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
    list.iter()
        .map(|(r, k)| ((*r).to_string(), k.map(Into::into)))
        .collect()
}

#[test]
fn rail_gives_each_asking_row_its_hand_off_in_rail_order() {
    let got = rail_handoffs(
        &asks(&[("r1", Some("10")), ("r2", None), ("r3", Some("30"))]),
        |k| match k {
            "10" => Some(to("11", k)),
            "30" => Some(to("31", k)),
            _ => None,
        },
    );
    assert_eq!(got.keys().collect::<Vec<_>>(), ["r1", "r3"]);
    assert_eq!(got["r3"].next.id, "31");
}

#[test]
fn rail_moves_only_the_first_of_two_rows_that_lead_to_the_same_entry() {
    let got = rail_handoffs(&asks(&[("r2", Some("10")), ("r1", Some("20"))]), |k| {
        Some(to("50", k))
    });
    // First in the rail, not first by id.
    assert_eq!(got.keys().collect::<Vec<_>>(), ["r2"]);
}

#[test]
fn unclaimed_moves_on_once_the_lookup_answers_that_no_row_resumes_the_next_entry() {
    let h = to("20", "10");
    assert_eq!(
        unclaimed_handoff(Some(h.clone()), Some(false)),
        Some(h.clone())
    );
    assert_eq!(unclaimed_handoff(Some(h.clone()), None), None);
    assert_eq!(unclaimed_handoff(Some(h), Some(true)), None);
    assert_eq!(unclaimed_handoff(None, Some(false)), None);
}

proptest! {
    #[test]
    fn asks_only_where_the_entry_is_at_its_finale_and_was_not_left_part_way(
        status in prop::option::of(prop::sample::select(&["finished", "current", "tba"][..])),
        episodes in prop::option::of(1u32..=30),
        ep in prop_oneof![(0u32..=32).prop_map(|n| n.to_string()), prop::sample::select(&["", "3.5", "12.5", "x", "-1", "1-12"][..]).prop_map(String::from)],
        part_way in any::<bool>(),
        playable in prop::option::of(1u32..=32),
    ) {
        let matched = HandoffMatch { id: "7".into(), status: status.map(Into::into), episode_count: episodes };
        let got = ask(Some(matched), &ep, playable, part_way);
        let last = episodes.map(|n| n.max(playable.unwrap_or(0)));
        let own = FinaleSelf { status: status.map(Into::into), episodes: last };
        let at_finale = history_episode(&ep).is_some_and(|n| is_finale(&own, n));
        prop_assert_eq!(got, (at_finale && !part_way).then(|| "7".to_string()));
    }

    #[test]
    fn names_only_an_aired_unplayed_sole_next_entry_of_its_own_series_read_whole(
        franchise in franchise_arb(),
        order in prop::sample::select(&[Linked, Released][..]),
        played in prop::collection::btree_set((0u32..=60).prop_map(|n| n.to_string()), 0..=4),
    ) {
        let id = franchise.anchor.clone();
        let got = handoff_from(&franchise, &id, order, TODAY, &played);
        let up = run_of(&franchise)
            .filter(|r| r.entries.iter().any(|e| e.id == id))
            .map(|r| up_next_for(&r, &id, order, TODAY));
        match up {
            Some(UpNext::One { entry }) if franchise.complete && !played.contains(&entry.id) => {
                let got = got.expect("a hand-off");
                prop_assert_eq!(got.finished.id, id);
                prop_assert_eq!(got.next, entry);
            }
            _ => prop_assert_eq!(got, None),
        }
    }

    #[test]
    fn played_ids_hold_every_recorded_id_and_every_match_once_all_rows_are_matched(
        rows in prop::collection::vec(
            (prop::sample::select(&["a", "b", "c", "d"][..]), prop::option::of(prop::sample::select(&["1", "2", "3"][..]))),
            0..=6,
        ),
        matches in prop::collection::btree_map(
            prop::sample::select(&["a", "b", "c", "d"][..]),
            prop::option::of(prop::sample::select(&["1", "2", "3", "9"][..])),
            0..=4,
        ),
    ) {
        let rows: Vec<PlayedRow> = rows.into_iter().map(|(id, k)| played_row(id, k)).collect();
        let matches: BTreeMap<String, Option<String>> =
            matches.into_iter().map(|(k, v)| (k.to_string(), v.map(Into::into))).collect();
        let got = played_ids(&rows, &matches);
        if rows.iter().any(|r| !matches.contains_key(&r.id)) {
            prop_assert_eq!(got, None);
            return Ok(());
        }
        let mut want = BTreeSet::new();
        for r in &rows {
            want.extend(r.kitsu_id.clone());
            want.extend(matches[&r.id].clone());
        }
        prop_assert_eq!(got, Some(want));
    }

    #[test]
    fn the_rail_shows_any_entry_at_most_once_and_only_for_rows_that_asked(
        list in prop::collection::vec(prop::option::of(1u32..=9), 0..=8),
        next_of in prop::collection::btree_map(1u32..=9, prop::option::of(1u32..=5), 0..=9),
    ) {
        // Row ids are unique, as history ids are.
        let asks: Vec<(String, Option<String>)> = list.iter().enumerate().map(|(r, k)| (format!("r{r}"), k.map(|k| k.to_string()))).collect();
        let got = rail_handoffs(&asks, |k| {
            let n = next_of.get(&k.parse::<u32>().ok()?).copied().flatten()?;
            Some(to(&n.to_string(), k))
        });
        let nexts: BTreeSet<&str> = got.values().map(|h| h.next.id.as_str()).collect();
        prop_assert_eq!(nexts.len(), got.len());
        for (row, h) in &got {
            let first = asks.iter().find(|(r, _)| r == row).expect("an asking row");
            prop_assert_eq!(first.1.as_deref(), Some(h.finished.id.as_str()));
        }
    }
}

/// A rail of one row at Bleach's finale, its series' hand-off read, and
/// every history row but `unmatched` matched.
fn rail_request(unmatched: Option<&str>) -> super::wire::RailRequest {
    let history = ["r", "other"]
        .iter()
        .map(|id| PlayedRow {
            id: (*id).into(),
            kitsu_id: None,
        })
        .collect();
    let matches = [("r", Some("43078")), ("other", None)]
        .iter()
        .filter(|(id, _)| Some(*id) != unmatched)
        .map(|(id, m)| ((*id).to_string(), m.map(Into::into)))
        .collect();
    super::wire::RailRequest {
        rows: vec![AskRow {
            row_id: "r".into(),
            matched: Some(finished(Some(13))),
            ep_no: "13".into(),
            playable_count: None,
            left_part_way: false,
        }],
        history,
        matches,
        series: [("43078".to_string(), Some(to("20", "43078")))]
            .into_iter()
            .collect(),
    }
}

#[test]
fn the_rail_waits_for_the_played_set_while_a_row_is_still_being_matched() {
    let waiting = super::wire::rail_view(&rail_request(Some("other")));
    assert_eq!(waiting.asks.get("r"), Some(&Some("43078".to_string())));
    assert!(waiting.handoffs.is_empty());
    let known = super::wire::rail_view(&rail_request(None));
    assert_eq!(known.handoffs.get("r"), Some(&to("20", "43078")));
}

proptest! {
    #[test]
    fn unclaimed_moves_on_only_with_a_hand_off_the_lookup_says_no_row_resumes(
        has in any::<bool>(),
        named in prop::option::of(any::<bool>()),
    ) {
        let h = to("20", "10");
        prop_assert_eq!(
            unclaimed_handoff(has.then(|| h.clone()), named),
            (has && named == Some(false)).then_some(h)
        );
    }
}
