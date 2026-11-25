//! The registry's rules, and the registry this build ships.

use super::*;
use crate::flags::{FlagDecl, FlagKind};

const FLAG: FlagDecl = FlagDecl {
    name: "a_feature",
    kind: FlagKind::Rollout,
    safe_because: "a test flag",
};

fn tip(json: serde_json::Value) -> TipDecl {
    let mut base = serde_json::json!({
        "id": "a-tip",
        "kind": "single",
        "since": "0.19.0",
        "surface": "show",
        "moment": "arrive",
        "places": ["a-place"],
    });
    base.as_object_mut()
        .expect("an object")
        .extend(json.as_object().expect("an object").clone());
    serde_json::from_value(base).expect("a declaration")
}

fn refused(tips: &[TipDecl], why: &str) {
    let err = validate(tips, &[FLAG]).expect_err("refused");
    assert!(err.contains(why), "{err:?} does not say {why:?}");
}

#[test]
fn the_shipped_registry_holds_to_the_rules() {
    validate(&TIPS, crate::flags::DECLARED).expect("tips.json holds to the rules");
}

#[test]
fn the_tips_offered_in_0_19_0_are_the_watch_order_strip_alone() {
    let offered: Vec<(&str, &[String])> = TIPS
        .iter()
        .filter(|t| !t.on_demand && t.since == Release::new(0, 19, 0))
        .map(|t| (t.id.as_str(), t.places.as_slice()))
        .collect();
    assert_eq!(
        offered,
        vec![("watch-order", &["watch-order-strip".to_owned()][..])]
    );
    let strip = find(&TIPS, "watch-order").expect("declared");
    assert_eq!(strip.kind, Kind::Single);
    assert_eq!(strip.surface, Surface::Show);
    assert_eq!(strip.moment, Moment::Arrive);
    assert_eq!(strip.until, None, "it takes the default window");
    assert_eq!(strip.flag, None);
}

#[test]
fn a_well_formed_tip_of_each_tool_is_accepted() {
    let tips = [
        tip(serde_json::json!({})),
        tip(serde_json::json!({
            "id": "a-guide", "kind": "sequence", "surface": "franchise", "on_demand": true,
            "places": ["one", "two", "three"],
        })),
        tip(serde_json::json!({ "id": "an-entry", "kind": "entry", "how_to": "a-guide" })),
        tip(serde_json::json!({
            "id": "in-the-player", "surface": "play", "moment": "stream-load",
        })),
        tip(serde_json::json!({ "id": "flagged", "flag": "a_feature", "until": "0.25.0" })),
    ];
    validate(&tips, &[FLAG]).expect("accepted");
}

#[test]
fn ids_and_places_are_kebab_case() {
    for id in ["", "A-tip", "a_tip", "-a", "a-", "a--b", "a tip"] {
        refused(
            &[tip(serde_json::json!({ "id": id }))],
            "not lower-case words",
        );
    }
    refused(
        &[tip(serde_json::json!({ "places": ["Strip"] }))],
        "not kebab-case",
    );
}

#[test]
fn an_id_is_declared_once_and_a_place_named_once_per_tip() {
    refused(
        &[tip(serde_json::json!({})), tip(serde_json::json!({}))],
        "declared twice",
    );
    refused(
        &[tip(
            serde_json::json!({ "kind": "sequence", "places": ["a", "a"] }),
        )],
        "named twice",
    );
}

#[test]
fn a_single_or_entry_tip_has_one_step_and_a_sequence_up_to_three() {
    refused(
        &[tip(serde_json::json!({ "places": ["a", "b"] }))],
        "cannot have 2 steps",
    );
    refused(
        &[tip(serde_json::json!({ "places": [] }))],
        "cannot have 0 steps",
    );
    refused(
        &[tip(
            serde_json::json!({ "kind": "sequence", "places": ["a", "b", "c", "d"] }),
        )],
        "cannot have 4 steps",
    );
    refused(
        &[tip(serde_json::json!({ "kind": "sequence", "places": [] }))],
        "cannot have 0 steps",
    );
}

#[test]
fn only_an_entry_tip_names_a_walkthrough_and_it_names_an_on_demand_sequence_elsewhere() {
    let guide = || {
        tip(serde_json::json!({
            "id": "a-guide", "kind": "sequence", "surface": "franchise", "on_demand": true,
        }))
    };
    refused(
        &[tip(serde_json::json!({ "kind": "entry" }))],
        "names a walkthrough",
    );
    refused(
        &[guide(), tip(serde_json::json!({ "how_to": "a-guide" }))],
        "names a walkthrough",
    );
    refused(
        &[tip(
            serde_json::json!({ "kind": "entry", "how_to": "missing" }),
        )],
        "is not declared",
    );
    let shown =
        tip(serde_json::json!({ "id": "a-guide", "kind": "sequence", "surface": "franchise" }));
    refused(
        &[
            shown,
            tip(serde_json::json!({ "kind": "entry", "how_to": "a-guide" })),
        ],
        "not an on-demand sequence",
    );
    let single =
        tip(serde_json::json!({ "id": "a-guide", "on_demand": true, "surface": "franchise" }));
    refused(
        &[
            single,
            tip(serde_json::json!({ "kind": "entry", "how_to": "a-guide" })),
        ],
        "not an on-demand sequence",
    );
    let here = tip(serde_json::json!({ "id": "a-guide", "kind": "sequence", "on_demand": true }));
    refused(
        &[
            here,
            tip(serde_json::json!({ "kind": "entry", "how_to": "a-guide" })),
        ],
        "where the feature lives",
    );
}

#[test]
fn a_stream_load_tip_is_a_single_tip_in_the_player() {
    refused(
        &[tip(serde_json::json!({ "moment": "stream-load" }))],
        "single tip in the player",
    );
    refused(
        &[tip(serde_json::json!({
            "moment": "stream-load", "surface": "play", "kind": "sequence",
        }))],
        "single tip in the player",
    );
}

#[test]
fn a_window_opens_before_it_closes() {
    refused(
        &[tip(serde_json::json!({ "until": "0.19.0" }))],
        "before it opens",
    );
    refused(
        &[tip(serde_json::json!({ "until": "0.18.5" }))],
        "before it opens",
    );
}

#[test]
fn a_flagged_tip_names_a_declared_flag_and_its_hard_end() {
    refused(
        &[tip(serde_json::json!({ "flag": "a_feature" }))],
        "declares its hard end",
    );
    refused(
        &[tip(
            serde_json::json!({ "flag": "unknown", "until": "0.30.0" }),
        )],
        "does not declare",
    );
}

#[test]
fn a_field_the_registry_does_not_know_is_refused() {
    let raw = serde_json::json!([{
        "id": "a", "kind": "single", "since": "0.19.0", "surface": "show",
        "moment": "arrive", "places": ["a"], "colour": "red",
    }]);
    assert!(serde_json::from_value::<Vec<TipDecl>>(raw).is_err());
}

#[test]
fn the_tools_ship_as_on_demand_demonstrations_of_an_entry_and_its_walkthrough() {
    let entry = find(&TIPS, "franchise-entry").expect("declared");
    assert!(entry.on_demand, "a demonstration is never offered");
    assert_eq!(entry.kind, Kind::Entry);
    assert_eq!(entry.places, ["watch-order-franchise"]);
    assert_eq!(entry.how_to.as_deref(), Some("franchise-tour"));
    let tour = find(&TIPS, "franchise-tour").expect("declared");
    assert!(tour.on_demand);
    assert_eq!(tour.kind, Kind::Sequence);
    assert_eq!(tour.surface, Surface::Franchise);
    assert_eq!(
        tour.places,
        ["franchise-masthead", "franchise-order", "franchise-rows"]
    );
}

#[test]
fn the_player_load_tip_ships_as_an_on_demand_demonstration() {
    let tip = find(&TIPS, "player-up-next").expect("declared");
    assert!(tip.on_demand, "a demonstration is never offered");
    assert_eq!(tip.kind, Kind::Single);
    assert_eq!(tip.surface, Surface::Play);
    assert_eq!(tip.moment, Moment::StreamLoad);
    assert_eq!(tip.places, ["player-frame"]);
}
