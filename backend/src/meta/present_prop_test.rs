//! [`Present`] tells a field missing from a reply from one given as
//! null, whatever the field carries and whatever surrounds it: missing
//! fails, null is `None`, and a value reads as the field's own type
//! would read it.

use super::Present;
use proptest::prelude::*;
use serde::{Deserialize, Serialize};

/// A reply with the field under test, `f`, beside whatever else.
#[derive(Deserialize)]
#[serde(bound = "T: serde::de::DeserializeOwned")]
struct Reply<T> {
    f: Present<T>,
}

/// A nested payload, as AniList's `Media` and its fields are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Nested {
    id: u32,
    title: Option<String>,
    episodes: Vec<u16>,
}

fn nested() -> impl Strategy<Value = Nested> {
    (
        any::<u32>(),
        proptest::option::of("\\PC{0,12}"),
        proptest::collection::vec(any::<u16>(), 0..4),
    )
        .prop_map(|(id, title, episodes)| Nested {
            id,
            title,
            episodes,
        })
}

/// Fields beside `f`: other names, with values of any shape.
fn surroundings() -> impl Strategy<Value = serde_json::Map<String, serde_json::Value>> {
    let value = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::from),
        any::<i64>().prop_map(serde_json::Value::from),
        "\\PC{0,8}".prop_map(serde_json::Value::from),
        Just(serde_json::json!({ "f": 1 })),
        Just(serde_json::json!([null, "f"])),
    ];
    proptest::collection::btree_map("[a-e]{1,3}", value, 0..4).prop_map(|m| m.into_iter().collect())
}

/// `reply` with `f` set to `field`, or without it.
fn with_field(
    mut reply: serde_json::Map<String, serde_json::Value>,
    field: Option<serde_json::Value>,
) -> String {
    reply.remove("f");
    if let Some(field) = field {
        reply.insert("f".into(), field);
    }
    serde_json::Value::Object(reply).to_string()
}

/// The three readings, for a payload of type `T` given as `value`.
fn reads_as_its_kind<T>(value: &T, around: serde_json::Map<String, serde_json::Value>)
where
    T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let missing = with_field(around.clone(), None);
    assert!(
        serde_json::from_str::<Reply<T>>(&missing).is_err(),
        "a reply without the field is no answer: {missing}"
    );
    let null = with_field(around.clone(), Some(serde_json::Value::Null));
    let read = serde_json::from_str::<Reply<T>>(&null).expect("null is an answer");
    assert_eq!(read.f.into_inner(), None, "{null}");
    let given = with_field(around, Some(serde_json::to_value(value).expect("json")));
    let read = serde_json::from_str::<Reply<T>>(&given).expect("a value is an answer");
    assert_eq!(read.f.into_inner().as_ref(), Some(value), "{given}");
}

proptest! {
    #[test]
    fn a_number_field_is_missing_null_or_itself(n in any::<i64>(), around in surroundings()) {
        reads_as_its_kind(&n, around);
    }

    #[test]
    fn a_text_field_is_missing_null_or_itself(s in "\\PC{0,16}", around in surroundings()) {
        reads_as_its_kind(&s, around);
    }

    #[test]
    fn a_list_field_is_missing_null_or_itself(
        v in proptest::collection::vec(any::<u8>(), 0..6),
        around in surroundings(),
    ) {
        reads_as_its_kind(&v, around);
    }

    #[test]
    fn a_nested_field_is_missing_null_or_itself(n in nested(), around in surroundings()) {
        reads_as_its_kind(&n, around);
    }

    /// A field of the wrong kind is no answer either: it fails as the
    /// field's own type would, rather than reading as null.
    #[test]
    fn a_field_of_another_kind_fails(s in "\\PC{0,16}", around in surroundings()) {
        let text = with_field(around.clone(), Some(serde_json::Value::from(s)));
        prop_assert!(serde_json::from_str::<Reply<i64>>(&text).is_err(), "{}", text);
        let number = with_field(around, Some(serde_json::Value::from(7)));
        prop_assert!(serde_json::from_str::<Reply<Nested>>(&number).is_err(), "{}", number);
    }

    /// Present nested in Present, as AniList's `Media` and the fields
    /// inside it are: the inner field missing fails the reply though
    /// the outer one is there, the inner null reads as None inside
    /// Some, and the outer null reads as None without asking for the
    /// inner field at all.
    #[test]
    fn a_nested_present_field_is_missing_null_or_itself(
        n in any::<u32>(),
        around in surroundings(),
    ) {
        #[derive(Deserialize)]
        struct Outer {
            inner: Present<u32>,
        }
        let read = |field: Option<serde_json::Value>| {
            serde_json::from_str::<Reply<Outer>>(&with_field(around.clone(), field))
        };
        let empty = read(Some(serde_json::json!({})));
        prop_assert!(empty.is_err(), "an outer with no inner field");
        let other = read(Some(serde_json::json!({ "other": n })));
        prop_assert!(other.is_err(), "an outer with only other fields");
        let null_inner = read(Some(serde_json::json!({ "inner": null }))).expect("an answer");
        prop_assert_eq!(null_inner.f.into_inner().map(|o| o.inner.into_inner()), Some(None));
        let given = read(Some(serde_json::json!({ "inner": n }))).expect("an answer");
        prop_assert_eq!(given.f.into_inner().map(|o| o.inner.into_inner()), Some(Some(n)));
        let null_outer = read(Some(serde_json::Value::Null)).expect("an answer");
        prop_assert!(null_outer.f.into_inner().is_none());
    }
}

/// The same reply read through the `present` field function, as the
/// readers that keep a plain `Option` field do.
#[derive(Deserialize)]
#[serde(bound = "T: serde::de::DeserializeOwned")]
struct PlainReply<T> {
    #[serde(deserialize_with = "super::present")]
    f: Option<T>,
}

/// What a reply reads as through either form: `None` when it is no
/// answer, else the field.
fn through_both<T>(reply: &str) -> (Option<Option<T>>, Option<Option<T>>)
where
    T: serde::de::DeserializeOwned,
{
    let typed = serde_json::from_str::<Reply<T>>(reply)
        .ok()
        .map(|r| r.f.into_inner());
    let plain = serde_json::from_str::<PlainReply<T>>(reply)
        .ok()
        .map(|r| r.f);
    (typed, plain)
}

proptest! {
    /// `present` reads every reply exactly as `Present` does: missing is
    /// no answer, null is None, a value is itself, a value of another
    /// kind is no answer.
    #[test]
    fn the_present_field_function_reads_as_present_does(
        field in proptest::option::of(prop_oneof![
            Just(serde_json::Value::Null),
            any::<u32>().prop_map(serde_json::Value::from),
            "\\PC{0,8}".prop_map(serde_json::Value::from),
            nested().prop_map(|n| serde_json::to_value(n).expect("json")),
        ]),
        around in surroundings(),
    ) {
        let reply = with_field(around, field);
        let (typed, plain) = through_both::<u32>(&reply);
        prop_assert_eq!(typed, plain, "{}", reply);
        let (typed, plain) = through_both::<Nested>(&reply);
        prop_assert_eq!(typed, plain, "{}", reply);
    }
}

/// The reading the four private copies of `present` gave before they
/// were merged into the shared one: a plain `Option` deserialize.
fn former_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Deserialize)]
#[serde(bound = "T: serde::de::DeserializeOwned")]
struct FormerReply<T> {
    #[serde(deserialize_with = "former_present")]
    f: Option<T>,
}

proptest! {
    /// The shared `present` reads every reply as the copies it replaced
    /// did: missing, null, a value, or a value of another kind.
    #[test]
    fn the_shared_present_reads_as_the_copies_it_replaced(
        field in proptest::option::of(prop_oneof![
            Just(serde_json::Value::Null),
            any::<u32>().prop_map(serde_json::Value::from),
            any::<i64>().prop_map(serde_json::Value::from),
            "\\PC{0,8}".prop_map(serde_json::Value::from),
            nested().prop_map(|n| serde_json::to_value(n).expect("json")),
        ]),
        around in surroundings(),
    ) {
        let reply = with_field(around, field);
        let now = serde_json::from_str::<PlainReply<u32>>(&reply).ok().map(|r| r.f);
        let before = serde_json::from_str::<FormerReply<u32>>(&reply).ok().map(|r| r.f);
        prop_assert_eq!(now, before, "{}", reply);
        let now = serde_json::from_str::<PlainReply<Nested>>(&reply).ok().map(|r| r.f);
        let before = serde_json::from_str::<FormerReply<Nested>>(&reply).ok().map(|r| r.f);
        prop_assert_eq!(now, before, "{}", reply);
    }
}
