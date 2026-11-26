//! The eligibility rules, case by case.

use super::super::decl::TipDecl;
use super::*;

fn r(text: &str) -> Release {
    Release::parse(text).expect("a release")
}

fn tip(extra: serde_json::Value) -> TipDecl {
    let mut base = serde_json::json!({
        "id": "a-tip", "kind": "single", "since": "0.19.0", "surface": "show",
        "moment": "arrive", "places": ["a-place"],
    });
    base.as_object_mut()
        .expect("an object")
        .extend(extra.as_object().expect("an object").clone());
    serde_json::from_value(base).expect("a declaration")
}

fn flagged(until: &str) -> TipDecl {
    tip(serde_json::json!({ "id": "flagged", "flag": "a_feature", "until": until }))
}

const ON: fn(&str) -> bool = |_| true;
const OFF: fn(&str) -> bool = |_| false;

#[test]
fn a_tip_shipped_in_0_19_0_is_offered_in_0_19_x_and_0_20_x_and_gone_from_0_21_0() {
    let t = tip(serde_json::json!({}));
    let rec = Record::default();
    for v in ["0.19.0", "0.19.3", "0.20.0", "0.20.9"] {
        assert!(offered(&t, &rec, r(v), OFF), "{v}");
    }
    for v in ["0.18.1", "0.21.0", "0.21.4", "1.0.0"] {
        assert!(!offered(&t, &rec, r(v), OFF), "{v}");
    }
}

#[test]
fn a_declared_end_replaces_the_default_one() {
    let t = tip(serde_json::json!({ "until": "0.20.0" }));
    assert!(offered(&t, &Record::default(), r("0.19.5"), OFF));
    assert!(!offered(&t, &Record::default(), r("0.20.0"), OFF));
}

#[test]
fn a_seen_tip_is_offered_but_not_pending() {
    let t = tip(serde_json::json!({}));
    let mut rec = Record::default();
    assert!(pending(&t, &rec, r("0.19.0"), OFF));
    rec.seen.insert("a-tip".into());
    assert!(offered(&t, &rec, r("0.19.0"), OFF));
    assert!(!pending(&t, &rec, r("0.19.0"), OFF));
}

#[test]
fn an_on_demand_tip_is_never_offered() {
    let t = tip(serde_json::json!({ "on_demand": true }));
    assert_eq!(window(&t, &Record::default(), r("0.19.0"), ON), None);
    assert!(!pending(&t, &Record::default(), r("0.19.0"), ON));
}

#[test]
fn a_flagged_tip_is_offered_only_while_its_flag_is_on() {
    let t = flagged("0.30.0");
    let rec = Record::default();
    assert!(!offered(&t, &rec, r("0.22.0"), OFF));
    assert!(offered(&t, &rec, r("0.22.0"), ON));
}

#[test]
fn a_flagged_tip_counts_two_minors_from_the_first_launch_with_its_flag_on() {
    let t = flagged("0.30.0");
    let mut rec = Record::default();
    rec.flag_on_since.insert("a_feature".into(), r("0.24.2"));
    assert_eq!(
        window(&t, &rec, r("0.25.0"), ON),
        Some((r("0.24.2"), r("0.26.0")))
    );
    assert!(offered(&t, &rec, r("0.25.9"), ON));
    assert!(!offered(&t, &rec, r("0.26.0"), ON));
}

#[test]
fn a_flagged_tip_stops_at_its_declared_end_however_late_the_flag_came_on() {
    let t = flagged("0.25.0");
    let mut rec = Record::default();
    rec.flag_on_since.insert("a_feature".into(), r("0.24.0"));
    assert_eq!(
        window(&t, &rec, r("0.24.0"), ON),
        Some((r("0.24.0"), r("0.25.0")))
    );
    assert!(!offered(&t, &rec, r("0.25.0"), ON));
    rec.flag_on_since.insert("a_feature".into(), r("0.26.0"));
    assert!(!offered(&t, &rec, r("0.26.0"), ON));
    // A window that would open at or after its end has none at all, so
    // the debug route never shows an inverted one.
    assert_eq!(window(&t, &rec, r("0.26.0"), ON), None);
}

#[test]
fn a_flag_on_before_the_feature_shipped_counts_from_the_shipping_release() {
    let t = tip(serde_json::json!({
        "id": "flagged", "since": "0.22.0", "flag": "a_feature", "until": "0.30.0",
    }));
    let mut rec = Record::default();
    rec.flag_on_since.insert("a_feature".into(), r("0.20.0"));
    assert_eq!(
        window(&t, &rec, r("0.22.0"), ON),
        Some((r("0.22.0"), r("0.24.0")))
    );
}

#[test]
fn a_flag_on_with_no_launch_recorded_counts_from_the_running_release() {
    let t = flagged("0.30.0");
    assert_eq!(
        window(&t, &Record::default(), r("0.23.1"), ON),
        Some((r("0.23.1"), r("0.25.0")))
    );
}

#[test]
fn sync_records_the_release_a_named_flag_was_first_on_in_and_keeps_it() {
    let tips = [flagged("0.30.0")];
    let once = sync(&tips, &Record::default(), r("0.22.0"), ON);
    assert_eq!(once.flag_on_since("a_feature"), Some(r("0.22.0")));
    let later = sync(&tips, &once, r("0.23.0"), ON);
    assert_eq!(later.flag_on_since("a_feature"), Some(r("0.22.0")));
    let off = sync(&tips, &Record::default(), r("0.22.0"), OFF);
    assert_eq!(off.flag_on_since("a_feature"), None);
}

#[test]
fn sync_records_only_flags_a_tip_names() {
    let tips = [tip(serde_json::json!({}))];
    let next = sync(&tips, &Record::default(), r("0.22.0"), ON);
    assert!(next.flag_on_since.is_empty());
}

#[test]
fn a_first_ever_install_marks_seen_what_is_offered_at_its_first_launch() {
    let tips = [
        tip(serde_json::json!({})),
        tip(serde_json::json!({ "id": "later", "since": "0.20.0" })),
        flagged("0.30.0"),
        tip(serde_json::json!({ "id": "guide", "on_demand": true })),
    ];
    let first = Record::new_install(true, r("0.19.0"));
    let next = sync(&tips, &first, r("0.19.0"), OFF);
    assert!(!next.baseline_pending);
    assert_eq!(next.seen.iter().collect::<Vec<_>>(), vec!["a-tip"]);
    assert_eq!(next.installed_in, Some(r("0.19.0")));
    // A later release introduces its own tip, and a flag turned on
    // later introduces its feature.
    assert!(pending(&tips[1], &next, r("0.20.0"), OFF));
    assert!(pending(&tips[2], &next, r("0.20.0"), ON));
}

#[test]
fn a_first_ever_install_with_a_flag_on_marks_that_feature_seen_too() {
    let tips = [flagged("0.30.0")];
    let next = sync(
        &tips,
        &Record::new_install(true, r("0.22.0")),
        r("0.22.0"),
        ON,
    );
    assert!(next.seen.contains("flagged"));
}

#[test]
fn an_update_marks_nothing_seen() {
    let tips = [tip(serde_json::json!({}))];
    let next = sync(
        &tips,
        &Record::new_install(false, r("0.19.0")),
        r("0.19.0"),
        OFF,
    );
    assert!(next.seen.is_empty());
    assert!(pending(&tips[0], &next, r("0.19.0"), OFF));
}
