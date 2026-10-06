use super::*;

proptest::proptest! {
    /// The id is exactly the trimmed value when that is non-empty
    /// digits, and nothing otherwise.
    #[test]
    fn an_id_is_trimmed_digits_and_nothing_else(raw in "\\PC{0,12}") {
        let t = raw.trim();
        let want = (!t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())).then_some(t);
        proptest::prop_assert_eq!(kitsu_id_in(&raw), want);
    }

    /// Whatever passes is digits only — never a separator, a dot, a
    /// colon or a space that could reshape a key or a URL path.
    #[test]
    fn what_passes_is_digits_only(raw in "[ 0-9a-z./:%\\t-]{0,12}") {
        if let Some(id) = kitsu_id_in(&raw) {
            proptest::prop_assert!(!id.is_empty());
            proptest::prop_assert!(id.bytes().all(|b| b.is_ascii_digit()), "{id:?}");
        }
    }
}

#[test]
fn values_shaped_like_paths_or_keys_are_not_ids() {
    for raw in [
        "../49877", "49877/x", "12:21", "kid-1", "", "  ", "4 2", "+1", "١٢",
    ] {
        assert_eq!(kitsu_id_in(raw), None, "{raw:?}");
    }
}

#[test]
fn surrounding_whitespace_is_trimmed() {
    assert_eq!(kitsu_id_in(" 49877\n"), Some("49877"));
    assert_eq!(require("\t1555 ").ok(), Some("1555"));
}

#[test]
fn a_required_id_that_is_not_one_answers_the_stable_key() {
    let err = require("../49877").expect_err("not an id");
    assert_eq!(err.key(), "error.request.invalid_kitsu_id");
    assert_eq!(err.http_status_code(), 400);
    let wire = serde_json::to_value(&err).expect("serializes");
    assert_eq!(wire["kind"], "invalid_kitsu_id");
}

#[derive(serde::Deserialize)]
struct Optional {
    #[serde(default, deserialize_with = "deserialize_optional")]
    kitsu_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct Listed {
    #[serde(deserialize_with = "deserialize_list")]
    kitsu_ids: Vec<String>,
}

#[test]
fn an_optional_id_that_is_not_one_reads_as_none() {
    let read = |body: &str| {
        serde_json::from_str::<Optional>(body)
            .expect("parses")
            .kitsu_id
    };
    assert_eq!(read(r#"{"kitsu_id":"../49877"}"#), None);
    assert_eq!(read(r#"{"kitsu_id":""}"#), None);
    assert_eq!(read(r#"{"kitsu_id":null}"#), None);
    assert_eq!(read("{}"), None);
    assert_eq!(read(r#"{"kitsu_id":" 49877 "}"#), Some("49877".into()));
}

#[test]
fn an_optional_id_reads_the_same_from_a_query_string() {
    let read = |q: &str| {
        serde_urlencoded::from_str::<Optional>(q)
            .expect("parses")
            .kitsu_id
    };
    assert_eq!(read("kitsu_id=12%3A21"), None);
    assert_eq!(read("kitsu_id="), None);
    assert_eq!(read(""), None);
    assert_eq!(read("kitsu_id=%2049877"), Some("49877".into()));
}

#[test]
fn a_list_keeps_only_the_ids_trimmed_in_order() {
    let parsed: Listed =
        serde_json::from_str(r#"{"kitsu_ids":["1","../2"," 3 ","4/x","5"]}"#).expect("parses");
    assert_eq!(parsed.kitsu_ids, vec!["1", "3", "5"]);
}
