//! The notices block: what is passed on, and what is dropped.

use serde_json::{json, Value};

use super::{parse, Dropped, Priority, RemoteNotice, FORMAT, MAX_BODY_CHARS, MAX_ITEMS};

const APP: &str = "0.20.0";

fn item(id: &str) -> Value {
    json!({ "id": id, "title": "Planned maintenance", "priority": "info" })
}

fn block(items: Vec<Value>) -> Value {
    json!({ "format": FORMAT, "items": items })
}

fn ids(v: &Value) -> Vec<String> {
    parse(Some(v), APP)
        .notices
        .items
        .into_iter()
        .map(|n| n.id)
        .collect()
}

#[test]
fn no_block_is_no_notices_and_nothing_dropped() {
    let parsed = parse(None, APP);
    assert!(parsed.notices.items.is_empty());
    assert_eq!(parsed.notices.donation_ask, None);
    assert!(parsed.dropped.is_empty());
}

#[test]
fn a_well_formed_notice_is_passed_on_with_every_field() {
    let v = block(vec![json!({
        "id": "source-2026-11",
        "title": "The source is down for maintenance",
        "body": "It should be back tonight.\nNothing on your side needs to change.",
        "link": "https://github.com/JoaoPucci/ani-gui/discussions/1",
        "priority": "warning",
        "about": "source"
    })]);
    let parsed = parse(Some(&v), APP);
    assert!(parsed.dropped.is_empty(), "{:?}", parsed.dropped);
    assert_eq!(
        parsed.notices.items,
        vec![RemoteNotice {
            id: "source-2026-11".into(),
            title: "The source is down for maintenance".into(),
            body: Some("It should be back tonight.\nNothing on your side needs to change.".into()),
            link: Some("https://github.com/JoaoPucci/ani-gui/discussions/1".into()),
            priority: Priority::Warning,
            about_source: true,
        }]
    );
}

#[test]
fn priority_defaults_to_info_and_an_unknown_one_drops_the_notice() {
    let v = block(vec![
        json!({ "id": "a", "title": "A" }),
        json!({ "id": "b", "title": "B", "priority": "urgent" }),
        json!({ "id": "c", "title": "C", "priority": "critical" }),
    ]);
    let parsed = parse(Some(&v), APP);
    let got: Vec<_> = parsed
        .notices
        .items
        .iter()
        .map(|n| (n.id.as_str(), n.priority))
        .collect();
    assert_eq!(got, vec![("a", Priority::Info), ("c", Priority::Critical)]);
    assert_eq!(parsed.dropped.len(), 1);
}

#[test]
fn unknown_fields_are_ignored_so_later_files_still_read() {
    let v = json!({
        "format": FORMAT,
        "a_later_field": { "anything": [1, 2] },
        "items": [{
            "id": "a",
            "title": "A",
            "button": { "action": "open_settings" },
            "image": "notices/a.png"
        }]
    });
    assert_eq!(ids(&v), vec!["a"]);
}

#[test]
fn an_unknown_about_is_ignored_rather_than_dropping_the_notice() {
    let v = block(vec![json!({ "id": "a", "title": "A", "about": "network" })]);
    let parsed = parse(Some(&v), APP);
    assert_eq!(parsed.notices.items.len(), 1);
    assert!(!parsed.notices.items[0].about_source);
}

#[test]
fn a_format_this_build_does_not_know_drops_the_whole_block() {
    for format in [json!(2), json!(0), json!("1"), Value::Null] {
        let v = json!({ "format": format, "donation_ask": 3, "items": [item("a")] });
        let parsed = parse(Some(&v), APP);
        assert_eq!(parsed.notices.items, vec![], "format {format}");
        assert_eq!(parsed.notices.donation_ask, None, "format {format}");
        assert!(matches!(parsed.dropped[..], [Dropped::Block(_)]));
    }
    let v = json!({ "donation_ask": 3, "items": [item("a")] });
    assert!(parse(Some(&v), APP).notices.items.is_empty());
}

#[test]
fn a_block_that_is_not_an_object_is_dropped() {
    for v in [json!([]), json!("x"), json!(1), json!(true)] {
        let parsed = parse(Some(&v), APP);
        assert!(parsed.notices.items.is_empty());
        assert_eq!(parsed.dropped.len(), 1);
    }
}

#[test]
fn the_donation_ask_is_a_non_negative_integer_or_nothing() {
    let ask = |raw: Value| {
        let v = json!({ "format": FORMAT, "donation_ask": raw });
        parse(Some(&v), APP)
    };
    assert_eq!(ask(json!(4)).notices.donation_ask, Some(4));
    assert_eq!(ask(json!(0)).notices.donation_ask, Some(0));
    for bad in [
        json!(-1),
        json!(1.5),
        json!("4"),
        json!(u64::MAX),
        Value::Null,
    ] {
        let parsed = ask(bad.clone());
        assert_eq!(parsed.notices.donation_ask, None, "{bad}");
        assert_eq!(parsed.dropped, vec![Dropped::DonationAsk], "{bad}");
    }
    let v = json!({ "format": FORMAT });
    assert_eq!(parse(Some(&v), APP).notices.donation_ask, None);
}

#[test]
fn a_link_must_be_plain_http_or_https() {
    let with_link = |link: &str| block(vec![json!({ "id": "a", "title": "A", "link": link })]);
    for good in [
        "https://github.com/JoaoPucci/ani-gui",
        "http://example.org/status?x=1#y",
    ] {
        assert_eq!(ids(&with_link(good)), vec!["a"], "{good}");
    }
    for bad in [
        "javascript:alert(1)",
        "data:text/html,<b>x</b>",
        "file:///etc/passwd",
        "ftp://example.org",
        "https://user:pass@example.org/",
        "https://user@example.org/",
        "//example.org",
        "/relative",
        "not a url",
        "",
    ] {
        assert!(ids(&with_link(bad)).is_empty(), "{bad}");
    }
    let long = format!("https://example.org/{}", "a".repeat(400));
    assert!(ids(&with_link(&long)).is_empty());
}

#[test]
fn text_is_bounded_and_free_of_control_and_direction_characters() {
    let with = |title: &str, body: Option<&str>| {
        let mut n = json!({ "id": "a", "title": title });
        if let Some(b) = body {
            n["body"] = json!(b);
        }
        block(vec![n])
    };
    assert_eq!(ids(&with("Fine", Some("Two\nlines"))), vec!["a"]);
    assert!(ids(&with("", None)).is_empty(), "an empty title");
    assert!(ids(&with("   ", None)).is_empty(), "a blank title");
    assert!(
        ids(&with("Bell\u{7}", None)).is_empty(),
        "a control character"
    );
    assert!(
        ids(&with("Two\nlines", None)).is_empty(),
        "a title is one line"
    );
    assert!(
        ids(&with("A\u{202E}gnp.exe", None)).is_empty(),
        "a direction override"
    );
    assert!(ids(&with("A", Some("\u{2066}x"))).is_empty(), "an isolate");
    assert!(
        ids(&with(&"t".repeat(121), None)).is_empty(),
        "a long title"
    );
    let body_at_cap = "b".repeat(MAX_BODY_CHARS);
    assert_eq!(ids(&with("A", Some(&body_at_cap))), vec!["a"]);
    let body_past_cap = "b".repeat(MAX_BODY_CHARS + 1);
    assert!(ids(&with("A", Some(&body_past_cap))).is_empty());
    assert!(ids(&with("A", Some(""))).is_empty(), "an empty body");
}

#[test]
fn text_is_trimmed_and_windows_line_ends_read_as_one() {
    let v = block(vec![
        json!({ "id": "a", "title": "  Spaced  ", "body": " one\r\ntwo " }),
    ]);
    let parsed = parse(Some(&v), APP);
    let n = &parsed.notices.items[0];
    assert_eq!(n.title, "Spaced");
    assert_eq!(n.body.as_deref(), Some("one\ntwo"));
}

#[test]
fn a_notice_needs_a_well_formed_id_and_ids_are_unique() {
    let v = block(vec![
        json!({ "title": "no id" }),
        json!({ "id": "Has Caps", "title": "A" }),
        json!({ "id": "x".repeat(65), "title": "A" }),
        json!({ "id": 7, "title": "A" }),
        item("ok-1.2_3"),
        item("ok-1.2_3"),
    ]);
    let parsed = parse(Some(&v), APP);
    assert_eq!(
        parsed
            .notices
            .items
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        vec!["ok-1.2_3"]
    );
    assert_eq!(parsed.dropped.len(), 5);
}

#[test]
fn at_most_the_first_few_notices_are_kept() {
    let many: Vec<Value> = (0..MAX_ITEMS + 3).map(|i| item(&format!("n{i}"))).collect();
    let parsed = parse(Some(&block(many)), APP);
    assert_eq!(parsed.notices.items.len(), MAX_ITEMS);
    assert_eq!(parsed.notices.items[0].id, "n0");
    assert_eq!(parsed.dropped.len(), 3);
}

#[test]
fn items_that_are_not_a_list_drop_only_the_items() {
    let v = json!({ "format": FORMAT, "donation_ask": 2, "items": { "id": "a" } });
    let parsed = parse(Some(&v), APP);
    assert!(parsed.notices.items.is_empty());
    assert_eq!(parsed.notices.donation_ask, Some(2));
    assert_eq!(parsed.dropped.len(), 1);
}

#[test]
fn a_version_range_targets_the_builds_it_names() {
    let ranged = |min: Option<&str>, max: Option<&str>| {
        let mut n = item("a");
        if let Some(m) = min {
            n["min_app"] = json!(m);
        }
        if let Some(m) = max {
            n["max_app"] = json!(m);
        }
        block(vec![n])
    };
    let shown = |v: &Value, app: &str| !parse(Some(v), app).notices.items.is_empty();
    let r = ranged(Some("0.20.0"), Some("0.20.3"));
    assert!(shown(&r, "0.20.0"));
    assert!(shown(&r, "0.20.3"));
    assert!(
        shown(&r, "0.20.1-dev"),
        "a pre-release reads as its release"
    );
    assert!(!shown(&r, "0.19.9"));
    assert!(!shown(&r, "0.21.0"));
    assert!(!shown(&r, "1.0.0"));
    assert!(shown(&ranged(Some("0.19.0"), None), "0.25.0"));
    assert!(shown(&ranged(None, Some("0.20.0")), "0.1.0"));
    // Out of range is targeting, not a fault: nothing is reported.
    assert!(parse(Some(&r), "0.21.0").dropped.is_empty());
    // A version that is not one is malformed, and drops the notice.
    for bad in ["0.20", "v0.20.0", "0.20.x", ""] {
        let parsed = parse(Some(&ranged(Some(bad), None)), APP);
        assert!(parsed.notices.items.is_empty(), "{bad}");
        assert_eq!(parsed.dropped.len(), 1, "{bad}");
    }
}

#[test]
fn the_response_shape_is_what_the_renderer_reads() {
    let v = block(vec![
        json!({ "id": "a", "title": "A", "priority": "critical" }),
    ]);
    let mut v = v;
    v["donation_ask"] = json!(1);
    let parsed = parse(Some(&v), APP);
    assert_eq!(
        serde_json::to_value(&parsed.notices).expect("serialise"),
        json!({
            "donation_ask": 1,
            "items": [{
                "id": "a",
                "title": "A",
                "body": null,
                "link": null,
                "priority": "critical",
                "about_source": false
            }]
        })
    );
}
