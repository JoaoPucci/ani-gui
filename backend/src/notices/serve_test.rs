//! What the window is handed of the published notices: nothing while
//! the notification center is off, the parsed block while it is on.

use serde_json::json;

use super::{served, NOTIFICATION_CENTER};
use crate::flags::decl::FlagKind;
use crate::flags::resolve::{parse_document, resolve, Resolved};
use crate::flags::DECLARED;

const APP: &str = "0.20.0";

fn flags(on: bool) -> Resolved {
    let doc = parse_document(format!(r#"{{"flags":{{"notification_center":{on}}}}}"#).as_bytes())
        .expect("doc");
    resolve(DECLARED, Some(&doc))
}

fn block() -> serde_json::Value {
    json!({
        "format": 1,
        "donation_ask": 3,
        "items": [
            { "id": "a", "title": "A notice", "link": "https://example.org/a" },
            { "id": "b", "title": "Broken", "link": "javascript:alert(1)" }
        ]
    })
}

#[test]
fn the_center_is_a_rollout_this_build_declares() {
    assert_eq!(NOTIFICATION_CENTER.name, "notification_center");
    assert_eq!(NOTIFICATION_CENTER.kind, FlagKind::Rollout);
    assert!(DECLARED.contains(&NOTIFICATION_CENTER));
}

#[test]
fn with_the_center_off_nothing_is_served() {
    let got = served(&flags(false), Some(&block()), APP);
    assert_eq!(got.donation_ask, None);
    assert!(got.items.is_empty());
}

#[test]
fn with_the_center_on_the_parsed_block_is_served_and_nothing_malformed() {
    let got = served(&flags(true), Some(&block()), APP);
    assert_eq!(got.donation_ask, Some(3));
    let ids: Vec<&str> = got.items.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["a"]);
}

#[test]
fn with_no_block_nothing_is_served() {
    let got = served(&flags(true), None, APP);
    assert_eq!(got, super::RemoteNotices::default());
}

#[test]
fn a_notice_aimed_at_other_releases_is_not_served() {
    let aimed = json!({ "format": 1, "items": [
        { "id": "old", "title": "Old", "max_app": "0.19.9" },
        { "id": "now", "title": "Now", "min_app": "0.20.0", "max_app": "0.20.9" }
    ]});
    let ids: Vec<String> = served(&flags(true), Some(&aimed), APP)
        .items
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert_eq!(ids, ["now"]);
}

/// The flag matrix's checks for the center: what the window is served
/// at each value.
pub(crate) mod checks {
    use super::{block, served, APP};
    use crate::flags::resolve::Resolved;

    pub(crate) fn off(r: &Resolved) {
        assert!(served(r, Some(&block()), APP).items.is_empty());
    }

    pub(crate) fn on(r: &Resolved) {
        assert_eq!(served(r, Some(&block()), APP).items.len(), 1);
    }
}
