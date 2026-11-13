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
