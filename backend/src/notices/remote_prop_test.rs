//! Whatever the file holds, reading it never panics, and nothing it
//! passes on breaks the block's rules: a link is always plain `http`
//! or `https` with no credentials, text is bounded and free of control
//! and direction characters, ids are unique and well formed.

use proptest::prelude::*;
use serde_json::{json, Map, Value};

use super::{
    parse, RemoteNotice, FORMAT, MAX_BODY_CHARS, MAX_ID_CHARS, MAX_ITEMS, MAX_LINK_CHARS,
    MAX_TITLE_CHARS,
};

/// Arbitrary JSON, a few levels deep.
fn any_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        any::<f64>().prop_map(|n| json!(n)),
        ".{0,40}".prop_map(Value::String),
    ];
    leaf.prop_recursive(3, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::btree_map(".{0,8}", inner, 0..6)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// Strings shaped like the fields a notice holds, hostile ones
/// included: schemes, credentials, markup, controls, overrides.
fn field_text() -> impl Strategy<Value = String> {
    prop_oneof![
        ".{0,200}",
        Just(String::new()),
        Just("   ".to_string()),
        Just("\r\n".to_string()),
        Just("javascript:alert(1)".to_string()),
        Just("JaVaScRiPt:alert(1)".to_string()),
        Just(" javascript:alert(1)".to_string()),
        Just("data:text/html,<script>x</script>".to_string()),
        Just("https://u:p@example.org/".to_string()),
        Just("https://example.org/<script>".to_string()),
        Just("<img src=x onerror=alert(1)>".to_string()),
        Just("a\u{202E}b".to_string()),
        Just("a\u{0}b".to_string()),
        Just("https://github.com/JoaoPucci/ani-gui".to_string()),
        Just("0.20.0".to_string()),
        Just("source".to_string()),
        Just("warning".to_string()),
    ]
}

/// A notice-shaped object: the known keys, each with a hostile or
/// arbitrary value, plus an arbitrary extra key.
fn notice_like() -> impl Strategy<Value = Value> {
    let key = prop_oneof![
        Just("id"),
        Just("title"),
        Just("body"),
        Just("link"),
        Just("priority"),
        Just("about"),
        Just("min_app"),
        Just("max_app"),
        Just("extra"),
    ];
    let value = prop_oneof![field_text().prop_map(Value::String), any_json()];
    prop::collection::vec((key, value), 0..9).prop_map(|pairs| {
        let mut m = Map::new();
        for (k, v) in pairs {
            m.insert(k.to_string(), v);
        }
        Value::Object(m)
    })
}

fn block() -> impl Strategy<Value = Value> {
    let items = prop::collection::vec(notice_like(), 0..12);
    (any_json(), items, prop::option::of(any_json())).prop_map(|(ask, items, format)| {
        json!({
            "format": format.unwrap_or(json!(FORMAT)),
            "donation_ask": ask,
            "items": items
        })
    })
}

fn safe(n: &RemoteNotice) -> Result<(), String> {
    let clean = |s: &str, multiline: bool| {
        !s.is_empty()
            && s.trim() == s
            && s.chars().all(|c| {
                (c == '\n' && multiline)
                    || (!c.is_control()
                        && !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
            })
    };
    if !(clean(&n.title, false) && n.title.chars().count() <= MAX_TITLE_CHARS) {
        return Err(format!("title {:?}", n.title));
    }
    if let Some(b) = &n.body {
        if !(clean(b, true) && b.chars().count() <= MAX_BODY_CHARS) {
            return Err(format!("body {b:?}"));
        }
    }
    if n.id.is_empty() || n.id.chars().count() > MAX_ID_CHARS {
        return Err(format!("id {:?}", n.id));
    }
    if let Some(l) = &n.link {
        let u = url::Url::parse(l).map_err(|e| format!("link {l:?}: {e}"))?;
        let plain = matches!(u.scheme(), "http" | "https")
            && u.username().is_empty()
            && u.password().is_none()
            && l.len() <= MAX_LINK_CHARS;
        if !plain {
            return Err(format!("link {l:?}"));
        }
    }
    Ok(())
}

proptest! {
    #[test]
    fn arbitrary_json_never_panics(v in any_json()) {
        let _ = parse(Some(&v), "0.20.0");
    }

    #[test]
    fn everything_passed_on_is_safe_and_bounded(v in block(), app in "[0-9]\\.[0-9]{1,2}\\.[0-9](-dev)?") {
        let parsed = parse(Some(&v), &app);
        prop_assert!(parsed.notices.items.len() <= MAX_ITEMS);
        let mut ids = std::collections::BTreeSet::new();
        for n in &parsed.notices.items {
            if let Err(why) = safe(n) {
                prop_assert!(false, "unsafe notice passed on: {}", why);
            }
            prop_assert!(ids.insert(n.id.clone()), "repeated id {}", n.id);
        }
    }

    #[test]
    fn a_notice_kept_or_dropped_is_accounted_for(v in block()) {
        let parsed = parse(Some(&v), "0.20.0");
        let offered = v["items"].as_array().map_or(0, Vec::len);
        let well_formed_block = v["format"] == json!(FORMAT);
        if well_formed_block {
            // Every notice is kept, dropped with a reason, or aimed at
            // other builds; none is counted twice.
            let item_drops = parsed
                .dropped
                .iter()
                .filter(|d| matches!(d, super::Dropped::Item { .. }))
                .count();
            prop_assert!(parsed.notices.items.len() + item_drops <= offered);
        } else {
            prop_assert!(parsed.notices.items.is_empty());
            prop_assert_eq!(parsed.notices.donation_ask, None);
        }
    }
}

/// Strings shaped like addresses: assorted schemes and cases,
/// credentials, ports, percent-escapes and markup in the path.
fn address_like() -> impl Strategy<Value = String> {
    prop_oneof![
        "(https?|HTTPS?|Https|ftp|file|javascript|data|mailto|ws)(:|://|:///)[a-zA-Z0-9:@.%\\[\\]-]{0,24}(/[a-zA-Z0-9%<>\"' .#?=-]{0,16})?",
        field_text(),
    ]
}

/// The served link the window shows and opens: an `http` or `https`
/// address with a host and no credentials, serialised as the backend's
/// WHATWG parser (the `url` crate) parses it back. The window's own
/// parser is Chromium's; that the two agree on every address is not
/// something this test can show.
fn plain_link(l: &str) -> Result<(), String> {
    let u = url::Url::parse(l).map_err(|e| format!("link {l:?}: {e}"))?;
    let plain = matches!(u.scheme(), "http" | "https")
        && u.username().is_empty()
        && u.password().is_none()
        && u.host_str().is_some_and(|h| !h.is_empty());
    if !plain || u.as_str() != l {
        return Err(format!("link {l:?}"));
    }
    Ok(())
}

/// The served answer, read by the rules the window applied before the
/// backend became the one place a notice is validated: an object with
/// a list; each notice an object whose id and title are non-empty
/// text, body none or non-empty text, priority one of the three, link
/// none or plain; one id once; the donation number a non-negative
/// integer or none.
fn window_would_keep(answer: &Value) -> Result<(), String> {
    let o = answer.as_object().ok_or("not an object")?;
    let ask = &o["donation_ask"];
    if !(ask.is_null() || ask.as_u64().is_some()) {
        return Err(format!("donation_ask {ask}"));
    }
    let items = o["items"].as_array().ok_or("items not a list")?;
    let mut ids = std::collections::BTreeSet::new();
    for n in items {
        let text = |k: &str| n[k].as_str().is_some_and(|s| !s.is_empty());
        if !(text("id") && text("title") && (n["body"].is_null() || text("body"))) {
            return Err(format!("text fields {n}"));
        }
        if !matches!(
            n["priority"].as_str(),
            Some("info" | "warning" | "critical")
        ) {
            return Err(format!("priority {n}"));
        }
        if !n["about_source"].is_boolean() {
            return Err(format!("about_source {n}"));
        }
        match &n["link"] {
            Value::Null => {}
            Value::String(l) => plain_link(l)?,
            other => return Err(format!("link {other}")),
        }
        if !ids.insert(n["id"].as_str().unwrap_or_default().to_string()) {
            return Err(format!("repeated id {n}"));
        }
    }
    Ok(())
}

/// A notice close enough to well formed that many are kept: a valid
/// id, and every other field drawn from hostile and arbitrary values.
fn nearly_well_formed() -> impl Strategy<Value = Value> {
    let priority = prop_oneof![
        Just(json!("info")),
        Just(json!("warning")),
        Just(json!("critical")),
        field_text().prop_map(Value::String),
        Just(Value::Null),
    ];
    (
        prop_oneof![9 => "[a-c0-9._-]{1,3}", 1 => Just(String::new())],
        field_text(),
        prop::option::of(field_text()),
        prop::option::of(address_like()),
        prop::option::of(priority),
        prop::option::of(prop_oneof![Just(json!("source")), any_json()]),
    )
        .prop_map(|(id, title, body, link, priority, about)| {
            let mut m = Map::new();
            m.insert("id".into(), json!(id));
            m.insert("title".into(), json!(title));
            for (k, v) in [
                ("body", body.map(Value::String)),
                ("link", link.map(Value::String)),
                ("priority", priority),
                ("about", about),
            ] {
                if let Some(v) = v {
                    m.insert(k.into(), v);
                }
            }
            Value::Object(m)
        })
}

fn served_block() -> impl Strategy<Value = Value> {
    let item = prop_oneof![nearly_well_formed(), notice_like()];
    (any_json(), prop::collection::vec(item, 0..12))
        .prop_map(|(ask, items)| json!({ "format": FORMAT, "donation_ask": ask, "items": items }))
}

proptest! {
    #[test]
    fn no_link_but_a_plain_http_or_https_one_is_ever_served(link in address_like()) {
        let v = json!({ "format": FORMAT, "items": [{ "id": "a", "title": "A", "link": link }] });
        for n in parse(Some(&v), "0.20.0").notices.items {
            if let Some(l) = &n.link {
                if let Err(why) = plain_link(l) {
                    prop_assert!(false, "served {}", why);
                }
            }
        }
    }

    #[test]
    fn one_id_is_served_once_and_the_first_notice_with_it_wins(
        offered in prop::collection::vec(("[abc]", "[A-Z][a-z]{0,5}"), 0..12)
    ) {
        let items: Vec<Value> = offered
            .iter()
            .map(|(id, title)| json!({ "id": id, "title": title }))
            .collect();
        let v = json!({ "format": FORMAT, "items": items });
        let mut first: Vec<(String, String)> = Vec::new();
        for (id, title) in &offered {
            if !first.iter().any(|(i, _)| i == id) {
                first.push((id.clone(), title.clone()));
            }
        }
        let served: Vec<(String, String)> = parse(Some(&v), "0.20.0")
            .notices
            .items
            .into_iter()
            .map(|n| (n.id, n.title))
            .collect();
        prop_assert_eq!(served, first);
    }

    #[test]
    fn the_served_answer_passes_every_rule_the_window_once_checked(
        v in prop_oneof![block(), served_block()],
        app in "[0-9]\\.[0-9]{1,2}\\.[0-9](-dev)?"
    ) {
        let answer = serde_json::to_value(parse(Some(&v), &app).notices).expect("serialise");
        if let Err(why) = window_would_keep(&answer) {
            prop_assert!(false, "the window would have refused: {}", why);
        }
    }
}
