//! Property coverage for the MAL timestamp parser.
//!
//! Its own file rather than an append to `mal_user_test.rs`, matching
//! the other property modules on this branch: appended blocks collide
//! with every later addition to the same file.
//!
//! Every strategy function returns a `BoxedStrategy`. Composed without
//! boxing, the nested combinator types grow into symbol names tens of
//! thousands of characters long, and the Windows test build's debug info
//! hits the linker's PDB limit (LNK4319). Box each new strategy too.

use crate::meta::mal_user_parse::parse_iso8601_to_epoch;

proptest::proptest! {
    /// The same wall clock with an offset names an instant exactly
    /// that offset away from the same wall clock in UTC. Stated over
    /// arbitrary clocks and offsets because the failure this guards
    /// was silent: every MAL row carried the same skew, so nothing
    /// looked wrong until the rail began sorting them against
    /// AniList's true Unix seconds.
    #[test]
    fn an_offset_shifts_the_instant_by_exactly_that_offset(
        hh in 0i64..24,
        mm in 0i64..60,
        ss in 0i64..60,
        oh in 0i64..15,
        om in 0i64..60,
    ) {
        let wall = format!("2026-07-30T{hh:02}:{mm:02}:{ss:02}");
        let utc = parse_iso8601_to_epoch(&format!("{wall}Z"));
        let ahead = parse_iso8601_to_epoch(&format!("{wall}+{oh:02}:{om:02}"));
        let behind = parse_iso8601_to_epoch(&format!("{wall}-{oh:02}:{om:02}"));
        let offset = oh * 3600 + om * 60;

        // Ahead of UTC is EARLIER in absolute terms: 09:00+09:00 is
        // midnight UTC, not nine hours past it.
        proptest::prop_assert_eq!(utc - ahead, offset);
        proptest::prop_assert_eq!(behind - utc, offset);
    }

    /// A suffix that cannot be read means "already UTC" — the same
    /// tolerance the rest of this parser applies, so one odd row is
    /// placed by its wall clock rather than dropped from the list.
    #[test]
    fn an_unreadable_suffix_is_treated_as_utc(suffix in "[a-yA-Y!-/]{0,6}") {
        let wall = "2026-07-30T12:34:56";
        proptest::prop_assert_eq!(
            parse_iso8601_to_epoch(&format!("{wall}{suffix}")),
            parse_iso8601_to_epoch(&format!("{wall}Z")),
        );
    }
}

mod evidence {
    //! Property coverage for MAL's `media_evidence` parser.

    use crate::account::episode_map::TrackerShow;
    use crate::account::provider::{MediaEvidence, ProviderMediaId};
    use crate::meta::mal_user::evidence::parse_media_evidence;
    use proptest::prelude::*;
    use serde_json::{json, Map, Value};

    /// An optional field: absent from the body, sent as null, or sent.
    fn field(value: impl Strategy<Value = Value> + 'static) -> BoxedStrategy<Option<Value>> {
        prop_oneof![Just(None), Just(Some(Value::Null)), value.prop_map(Some)].boxed()
    }

    fn one_of(words: &'static [&'static str]) -> BoxedStrategy<Value> {
        proptest::sample::select(words)
            .prop_map(|w| json!(w))
            .boxed()
    }

    /// One `related_anime` row: a node and a relation, the relation
    /// any MAL sends, null, or left out.
    fn related() -> BoxedStrategy<Value> {
        (
            0u32..50,
            field(one_of(&["sequel", "prequel", "side_story", "Sequel"])),
        )
            .prop_map(|(id, relation)| {
                let mut row = json!({ "node": { "id": id } });
                if let Some(relation) = relation {
                    row["relation_type"] = relation;
                }
                row
            })
            .boxed()
    }

    /// An `/anime/{id}` body with every evidence field free.
    fn wire() -> BoxedStrategy<Value> {
        (
            any::<u32>(),
            field((0u32..30).prop_map(|n| json!(n))),
            field(one_of(&[
                "finished_airing",
                "currently_airing",
                "not_yet_aired",
            ])),
            field(one_of(&["tv", "movie", "music", "ova", "special"])),
            field("[0-9-]{0,10}|.{0,6}".prop_map(|s| json!(s))),
            field("[0-9-]{0,10}|.{0,6}".prop_map(|s| json!(s))),
            proptest::option::of(prop::collection::vec(related(), 0..5)),
        )
            .prop_map(|(id, episodes, status, media_type, start, end, related)| {
                let mut wire = Map::new();
                wire.insert("id".into(), json!(id));
                for (key, value) in [
                    ("num_episodes", episodes),
                    ("status", status),
                    ("media_type", media_type),
                    ("start_date", start),
                    ("end_date", end),
                ] {
                    if let Some(value) = value {
                        wire.insert(key.into(), value);
                    }
                }
                if let Some(related) = related {
                    wire.insert("related_anime".into(), Value::Array(related));
                }
                Value::Object(wire)
            })
            .boxed()
    }

    /// The evidence a generated body is specified to read as, `None`
    /// where it is no answer. The read asks for `num_episodes`, `status`,
    /// `media_type` and `related_anime`, whose rows carry their relation:
    /// a body that leaves one of them out, or a row that leaves out its
    /// relation, does not say what the anime is or what its sequels are.
    fn model(wire: &Value) -> Option<MediaEvidence> {
        for asked in ["num_episodes", "status", "media_type"] {
            wire.get(asked)?;
        }
        let rows = wire.get("related_anime")?.as_array()?;
        if rows.iter().any(|r| r.get("relation_type").is_none()) {
            return None;
        }
        let text = |key: &str| wire.get(key).and_then(Value::as_str).map(str::to_string);
        let num = |v: &Value, key: &str| {
            v.get(key)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
        };
        let sequels = rows
            .iter()
            .filter(|r| r.get("relation_type").and_then(Value::as_str) == Some("sequel"))
            .filter_map(|r| num(&r["node"], "id").map(ProviderMediaId))
            .collect();
        Some(MediaEvidence {
            show: TrackerShow {
                id: num(wire, "id").expect("an id"),
                id_mal: None,
                episodes: num(wire, "num_episodes").filter(|n| *n > 0),
                finished: text("status").as_deref() == Some("finished_airing"),
                start: text("start_date"),
                end: text("end_date"),
            },
            series: !matches!(text("media_type").as_deref(), Some("movie" | "music")),
            sequels,
            listed: None,
        })
    }

    proptest! {
        /// Any body reads as evidence or an error, never a panic.
        #[test]
        fn the_evidence_parser_answers_any_body(
            body in prop_oneof![
                prop::collection::vec(any::<u8>(), 0..64),
                "[\\[\\]{}\":, 0-9a-z_.-]{0,80}".prop_map(String::into_bytes),
                Just(br#"{"id":1,"related_anime":null}"#.to_vec()),
            ]
        ) {
            let _ = parse_media_evidence(&body);
        }

        /// Any anime body reads as specified: MAL's `0` or nothing for
        /// the length is unknown, only `finished_airing` is finished, a
        /// movie or a music video is no series, dates are kept as sent,
        /// only a `sequel` row is a sequel, and MAL lists no numbers.
        /// A body without `related_anime`, or with a row that leaves its
        /// relation out, is no answer: its sequels are unknown, never
        /// none. An empty `related_anime` is no sequels.
        #[test]
        fn an_anime_body_reads_as_its_fields(wire in wire()) {
            let body = serde_json::to_vec(&wire).expect("serialise");
            let parsed = parse_media_evidence(&body);
            let Some(want) = model(&wire) else {
                prop_assert!(parsed.is_err(), "related anime that do not say read as an answer");
                return Ok(());
            };
            prop_assert_eq!(parsed.expect("an anime"), want);
        }
    }
}
