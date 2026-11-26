//! What the routes send, built from a registry and a record.

use super::*;

fn r(text: &str) -> Release {
    Release::parse(text).expect("a release")
}

fn registry() -> Vec<TipDecl> {
    serde_json::from_value(serde_json::json!([
        { "id": "strip", "kind": "single", "since": "0.19.0", "surface": "show",
          "moment": "arrive", "places": ["the-strip"] },
        { "id": "entry", "kind": "entry", "since": "0.19.0", "surface": "show",
          "moment": "arrive", "places": ["the-link"], "how_to": "guide" },
        { "id": "guide", "kind": "sequence", "since": "0.19.0", "surface": "franchise",
          "moment": "arrive", "places": ["one", "two"], "on_demand": true },
        { "id": "flagged", "kind": "single", "since": "0.19.0", "until": "0.30.0",
          "surface": "home", "moment": "arrive", "places": ["bell"], "flag": "a_feature" },
        { "id": "old", "kind": "single", "since": "0.16.0", "surface": "home",
          "moment": "arrive", "places": ["old"] },
    ]))
    .expect("a registry")
}

const OFF: fn(&str) -> bool = |_| false;

#[test]
fn the_view_lists_the_offered_tips_in_order_seen_or_not() {
    let tips = registry();
    let mut rec = Record::default();
    rec.seen.insert("entry".into());
    let view = offered_view(&tips, &rec, r("0.19.2"), OFF);
    assert_eq!(view.version, r("0.19.2"));
    let listed: Vec<(&str, bool)> = view
        .tips
        .iter()
        .map(|t| (t.id.as_str(), t.pending))
        .collect();
    assert_eq!(listed, vec![("strip", true), ("entry", false)]);
}

#[test]
fn an_entry_tip_carries_its_walkthrough() {
    let tips = registry();
    let view = offered_view(&tips, &Record::default(), r("0.19.0"), OFF);
    let entry = view.tips.iter().find(|t| t.id == "entry").expect("offered");
    assert_eq!(
        entry.how_to,
        Some(HowToView {
            id: "guide".into(),
            surface: Surface::Franchise,
            places: vec!["one".into(), "two".into()],
        })
    );
    assert_eq!(view.tips[0].how_to, None);
}

#[test]
fn a_flag_turned_on_brings_its_tip_into_the_view() {
    let tips = registry();
    let view = offered_view(&tips, &Record::default(), r("0.19.0"), |f: &str| {
        f == "a_feature"
    });
    assert!(view.tips.iter().any(|t| t.id == "flagged" && t.pending));
}

#[test]
fn the_debug_view_lists_every_tip_with_its_state() {
    let tips = registry();
    let mut rec = Record::new_install(true, r("0.19.0"));
    rec.seen.insert("strip".into());
    rec.flag_on_since.insert("a_feature".into(), r("0.19.0"));
    let view = debug_view(&tips, &rec, r("0.20.1"), |f: &str| f == "a_feature");
    assert_eq!(view.installed_in, Some(r("0.19.0")));
    assert_eq!(view.tips.len(), tips.len());
    let strip = &view.tips[0];
    assert!(strip.seen && strip.offered && !strip.pending);
    assert_eq!(
        (strip.opens, strip.ends),
        (Some(r("0.19.0")), Some(r("0.21.0")))
    );
    let guide = &view.tips[2];
    assert!(guide.on_demand && !guide.offered && guide.opens.is_none());
    let flagged = &view.tips[3];
    assert!(flagged.flag_on && flagged.pending);
    assert_eq!(flagged.flag_on_since, Some(r("0.19.0")));
    assert_eq!(flagged.until, Some(r("0.30.0")));
    assert_eq!(flagged.ends, Some(r("0.21.0")));
    let old = &view.tips[4];
    assert!(!old.offered && !old.pending);
}

#[test]
fn marking_sets_and_clears_seen() {
    let tips = registry();
    let seen = mark(&tips, &Record::default(), "strip", true).expect("marked");
    assert!(seen.seen.contains("strip"));
    let unseen = mark(&tips, &seen, "strip", false).expect("cleared");
    assert!(!unseen.seen.contains("strip"));
}

#[test]
fn an_unknown_or_on_demand_tip_is_not_marked() {
    let tips = registry();
    assert_eq!(
        mark(&tips, &Record::default(), "nope", true),
        Err(MarkRefused::Unknown)
    );
    assert_eq!(
        mark(&tips, &Record::default(), "guide", true),
        Err(MarkRefused::OnDemand)
    );
}
