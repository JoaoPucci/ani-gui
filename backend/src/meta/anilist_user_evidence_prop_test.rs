//! Property coverage for AniList's `media_evidence` read: its fuzzy
//! dates, the numbers its streaming-episode titles carry, and the
//! parser over any `Media` envelope.

use super::{fuzzy_date, listed_number_of, parse_media_evidence_response};
use crate::account::episode_map::TrackerShow;
use crate::account::episode_map_calendar::{days_in_month, parse_date, CalendarDate};
use crate::account::provider::{MediaEvidence, ProviderMediaId};
use proptest::prelude::*;
use serde_json::{json, Value};

/// A fuzzy-date part as AniList could send it: absent, plausible, or
/// any number at all.
fn date_part(plausible: std::ops::RangeInclusive<u32>) -> impl Strategy<Value = Option<u32>> {
    prop_oneof![
        1 => Just(None),
        4 => plausible.prop_map(Some),
        1 => any::<u32>().prop_map(Some),
    ]
}

fn fuzzy_parts() -> impl Strategy<Value = (Option<u32>, Option<u32>, Option<u32>)> {
    (date_part(1900..=2100), date_part(0..=13), date_part(0..=32))
}

proptest! {
    /// Any parts read as a date or as nothing, never a panic; a date
    /// that comes back is the year alone where no month was sent, and
    /// otherwise reads back through the calendar as the parts sent.
    #[test]
    fn a_fuzzy_date_reads_back_as_its_parts((year, month, day) in fuzzy_parts()) {
        let Some(out) = fuzzy_date(year, month, day) else {
            return Ok(());
        };
        let year = year.expect("a date has a year");
        match month {
            None => prop_assert_eq!(out, format!("{year:04}")),
            Some(month) => {
                let read = parse_date(&out).expect("a real date");
                prop_assert_eq!(u32::try_from(read.year).ok(), Some(year));
                prop_assert_eq!(read.month, month);
                prop_assert_eq!(read.day, day);
            }
        }
    }

    /// Without a year there is no date, whatever else was sent.
    #[test]
    fn a_fuzzy_date_without_a_year_is_none(month: Option<u32>, day: Option<u32>) {
        prop_assert_eq!(fuzzy_date(None, month, day), None);
    }

    /// A year past four digits names no date the map can compare,
    /// whatever month and day come with it.
    #[test]
    fn a_year_past_9999_is_no_date(year in 10_000u32.., month: Option<u32>, day: Option<u32>) {
        prop_assert_eq!(fuzzy_date(Some(year), month, day), None);
    }

    /// A real calendar date, month or year reads as its zero-padded
    /// text, and that text reads back through the calendar as the date.
    #[test]
    fn a_real_date_round_trips(
        (year, month, day) in (0u32..=9999, 1u32..=12)
            .prop_flat_map(|(y, m)| {
                let len = days_in_month(i32::try_from(y).unwrap_or(0), m);
                (Just(y), Just(m), 1..=len)
            })
    ) {
        let full = format!("{year:04}-{month:02}-{day:02}");
        prop_assert_eq!(fuzzy_date(Some(year), Some(month), Some(day)), Some(full.clone()));
        let want = CalendarDate { year: i32::try_from(year).unwrap_or(0), month, day: Some(day) };
        prop_assert_eq!(parse_date(&full), Some(want));
        prop_assert_eq!(
            fuzzy_date(Some(year), Some(month), None),
            Some(format!("{year:04}-{month:02}"))
        );
        prop_assert_eq!(fuzzy_date(Some(year), None, Some(day)), Some(format!("{year:04}")));
    }

    /// A month or day the calendar does not have is no date.
    #[test]
    fn an_unreal_month_or_day_is_no_date(year in 0u32..=9999, month in 1u32..=12, past in 1u32..100) {
        prop_assert_eq!(fuzzy_date(Some(year), Some(0), None), None);
        prop_assert_eq!(fuzzy_date(Some(year), Some(12 + past), None), None);
        prop_assert_eq!(fuzzy_date(Some(year), Some(month), Some(0)), None);
        let len = days_in_month(i32::try_from(year).unwrap_or(0), month);
        prop_assert_eq!(fuzzy_date(Some(year), Some(month), Some(len + past)), None);
    }

    /// Any title reads as a number or as nothing, never a panic; a
    /// number that comes back opens on a digit and holds only digits
    /// and points.
    #[test]
    fn a_title_s_number_is_digits_and_points(title in ".{0,24}|[Ee]pisode [0-9. ]{0,6}.{0,6}") {
        if let Some(number) = listed_number_of(&title) {
            prop_assert!(number.starts_with(|c: char| c.is_ascii_digit()));
            prop_assert!(number.chars().all(|c| c.is_ascii_digit() || c == '.'));
        }
    }

    /// `Episode <number>` reads back as the number, however the title
    /// goes on after it and whatever space surrounds it.
    #[test]
    fn an_episode_title_reads_back_its_number(
        lead in "[ \t]{0,2}",
        word in "Episode|episode",
        number in "[0-9][0-9.]{0,5}",
        rest in "( - [A-Za-z ]{0,10}|: [A-Za-z]{0,6})?",
        trail in "[ \t]{0,2}",
    ) {
        let title = format!("{lead}{word} {number}{rest}{trail}");
        prop_assert_eq!(listed_number_of(&title), Some(number));
    }

    /// A title that does not open on `Episode ` and a number carries
    /// none.
    #[test]
    fn a_title_without_an_episode_number_carries_none(
        word in "EPISODE|Ep|Episode|Chapter", rest in "[^0-9][^ ]{0,8}|"
    ) {
        let title = format!("{word} {rest}");
        prop_assert_eq!(listed_number_of(&title), None);
    }
}

/// A value a reply could carry for a key, or `None` for the key left
/// out of it.
fn or_missing(value: impl Strategy<Value = Value>) -> impl Strategy<Value = Option<Value>> {
    prop_oneof![1 => Just(None), 6 => value.prop_map(Some)]
}

/// An object of the keys given a value; a key whose value is `None` is
/// left out.
fn object(fields: &[(&str, Option<Value>)]) -> Value {
    let mut out = serde_json::Map::new();
    for (key, value) in fields {
        if let Some(value) = value {
            out.insert((*key).to_string(), value.clone());
        }
    }
    Value::Object(out)
}

/// One relation edge as AniList could send it, any key of the edge or
/// its node left out now and then.
fn edge() -> impl Strategy<Value = Value> {
    let relation = prop_oneof![
        Just(json!("SEQUEL")),
        Just(json!("PREQUEL")),
        Just(json!(null))
    ];
    let kind = prop_oneof![
        Just(json!("ANIME")),
        Just(json!("MANGA")),
        Just(json!(null))
    ];
    let format = prop_oneof![
        Just(json!("TV")),
        Just(json!("MOVIE")),
        Just(json!("MUSIC")),
        Just(json!("ONA")),
        Just(json!(null)),
    ];
    let node = prop_oneof![
        1 => Just(json!(null)),
        4 => (0u32..50, or_missing(kind), or_missing(format)).prop_map(|(id, kind, format)| {
            object(&[("id", Some(json!(id))), ("type", kind), ("format", format)])
        }),
    ];
    (or_missing(relation), or_missing(node))
        .prop_map(|(relation, node)| object(&[("relationType", relation), ("node", node)]))
}

/// A fuzzy date object, null, or absent.
fn date_object() -> impl Strategy<Value = Option<Value>> {
    prop_oneof![
        Just(None),
        Just(Some(Value::Null)),
        fuzzy_parts().prop_map(|(y, m, d)| Some(json!({ "year": y, "month": m, "day": d }))),
    ]
}

/// A streaming-episode title list: absent, null, empty, or titles of
/// every kind, some null.
fn streaming() -> impl Strategy<Value = Option<Value>> {
    let title = prop_oneof![
        Just(json!(null)),
        "Episode [0-9]{1,2}(\\.5)? - [A-Za-z]{1,6}".prop_map(|t| json!(t)),
        "[A-Za-z ]{0,10}".prop_map(|t| json!(t)),
    ];
    prop_oneof![
        Just(None),
        Just(Some(Value::Null)),
        Just(Some(json!([]))),
        prop::collection::vec(title.prop_map(|t| json!({ "title": t })), 1..5)
            .prop_map(|v| Some(Value::Array(v))),
    ]
}

/// A `Media` envelope with every field free.
fn media() -> impl Strategy<Value = Value> {
    let status = prop_oneof![
        Just(json!("FINISHED")),
        Just(json!("RELEASING")),
        Just(json!(null)),
    ];
    let format = prop_oneof![
        Just(json!("TV")),
        Just(json!("MOVIE")),
        Just(json!("MUSIC")),
        Just(json!(null)),
    ];
    let relations = prop_oneof![
        Just(None),
        Just(Some(Value::Null)),
        Just(Some(json!({}))),
        Just(Some(json!({ "edges": null }))),
        prop::collection::vec(edge(), 0..5).prop_map(|e| Some(json!({ "edges": e }))),
    ];
    (
        (
            any::<u32>(),
            proptest::option::of(any::<u32>()),
            proptest::option::of(0u32..30),
        ),
        (status, format),
        (date_object(), date_object()),
        relations,
        streaming(),
    )
        .prop_map(
            |((id, id_mal, episodes), (status, format), (start, end), relations, streaming)| {
                let mut media = json!({
                    "id": id, "idMal": id_mal, "episodes": episodes,
                    "status": status, "format": format,
                });
                for (key, value) in [
                    ("startDate", start),
                    ("endDate", end),
                    ("relations", relations),
                    ("streamingEpisodes", streaming),
                ] {
                    if let Some(value) = value {
                        media[key] = value;
                    }
                }
                media
            },
        )
}

/// A wire date object read the way the parser is specified to read it.
fn model_date(value: Option<&Value>) -> Option<String> {
    let part = |key: &str| {
        value
            .and_then(|v| v.get(key))
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    fuzzy_date(part("year"), part("month"), part("day"))
}

/// Whether a generated `relations` says what the media's relations
/// are. The query asks for `relations { edges { relationType node { id
/// type format } } }`: a key of that left out, `relations` or `edges`
/// given as null, or a sequel whose node is null, leaves the sequels
/// unknown, never none.
fn relations_answer(media: &Value) -> bool {
    let Some(edges) = media
        .get("relations")
        .and_then(|r| r.get("edges"))
        .and_then(Value::as_array)
    else {
        return false;
    };
    edges.iter().all(|e| {
        let (Some(relation), Some(node)) = (e.get("relationType"), e.get("node")) else {
            return false;
        };
        if node.is_null() {
            return relation.as_str() != Some("SEQUEL");
        }
        node.get("type").is_some() && node.get("format").is_some()
    })
}

/// The evidence a generated envelope is specified to read as, `None`
/// where it is no answer.
fn model(media: &Value) -> Option<MediaEvidence> {
    if !relations_answer(media) {
        return None;
    }
    let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
    let num = |v: &Value, key: &str| {
        v.get(key)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    let series = |format: Option<String>| !matches!(format.as_deref(), Some("MOVIE" | "MUSIC"));
    let sequels = media["relations"]["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .filter(|e| text(e, "relationType").as_deref() == Some("SEQUEL"))
        .filter_map(|e| e.get("node").filter(|n| !n.is_null()))
        .filter(|n| text(n, "type").as_deref() == Some("ANIME") && series(text(n, "format")))
        .filter_map(|n| num(n, "id").map(ProviderMediaId))
        .collect();
    let listed = media
        .get("streamingEpisodes")
        .and_then(Value::as_array)
        .filter(|eps| !eps.is_empty())
        .map(|eps| {
            eps.iter()
                .filter_map(|e| listed_number_of(e.get("title")?.as_str()?))
                .collect()
        });
    Some(MediaEvidence {
        show: TrackerShow {
            id: num(media, "id").expect("an id"),
            id_mal: num(media, "idMal"),
            episodes: num(media, "episodes").filter(|n| *n > 0),
            finished: text(media, "status").as_deref() == Some("FINISHED"),
            start: model_date(media.get("startDate")),
            end: model_date(media.get("endDate")),
        },
        series: series(text(media, "format")),
        sequels,
        listed,
    })
}

proptest! {
    /// Any body reads as evidence or an error, never a panic.
    #[test]
    fn the_evidence_parser_answers_any_body(
        body in prop_oneof![
            prop::collection::vec(any::<u8>(), 0..64),
            "[\\[\\]{}\":, 0-9a-zA-Z.-]{0,80}".prop_map(String::into_bytes),
            Just(br#"{"data":{"Media":null}}"#.to_vec()),
        ]
    ) {
        let _ = parse_media_evidence_response(&body);
    }

    /// Any `Media` envelope reads as specified: an unannounced or zero
    /// total is unknown, only `FINISHED` is finished, only an anime
    /// series sequel is a sequel, and an absent or empty episode list
    /// is none while a listed one keeps every number its titles carry.
    /// An envelope whose relations leave out a key the query asks for,
    /// give `relations` or `edges` as null, or a sequel's node as null,
    /// is no answer: its sequels are unknown, never none.
    #[test]
    fn an_envelope_reads_as_its_fields(media in media()) {
        let body = serde_json::to_vec(&json!({ "data": { "Media": media } })).expect("serialise");
        let parsed = parse_media_evidence_response(&body);
        let Some(want) = model(&media) else {
            prop_assert!(parsed.is_err(), "relations that do not say read as an answer");
            return Ok(());
        };
        let got = parsed.expect("an envelope");
        prop_assert_eq!(got.listed.is_none(), media.get("streamingEpisodes")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty));
        prop_assert_eq!(got, want);
    }
}
