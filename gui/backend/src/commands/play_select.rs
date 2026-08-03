//! The custom serde deserializers that decode `PlayArgs` from both
//! the `POST /api/play` JSON body and the `GET /api/play/stream`
//! query string. EventSource is GET-only and `serde_urlencoded` only
//! knows strings, so the SSE path needs looser coercion than a plain
//! `bool` / `Vec<String>` field would accept.
//!
//! `deserialize_with = "..."` attributes on `PlayArgs` use the full
//! `crate::commands::play_select::*` paths since the helpers don't
//! live next to the struct.
//!
//! Nothing in here is async or stateful — no AppState, no network,
//! no filesystem.

use serde::Deserialize;

/// Accept either a JSON array of strings or a single newline-joined
/// string for `alt_titles`. The string form is the SSE-query path —
/// `serde_urlencoded` can't decode `alt_titles=a&alt_titles=b` as a Vec.
///
/// # Errors
/// Propagates from the underlying deserializer if the field is
/// neither a list nor a string nor null.
pub fn deserialize_alt_titles<'de, D>(d: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        List(Vec<String>),
        Joined(String),
    }
    Option::<Wire>::deserialize(d).map(|opt| match opt {
        None => Vec::new(),
        Some(Wire::List(v)) => v,
        Some(Wire::Joined(s)) => s
            .split('\n')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect(),
    })
}

/// Accept JSON bool OR `"1"` / `"true"` / `"yes"` strings — the SSE GET
/// path goes through `serde_urlencoded` which only knows strings, so a
/// plain `bool` field would reject `?prefetch=1`. Anything else
/// (`"0"`, `"false"`, `null`, missing) decodes as `false`.
///
/// # Errors
/// Propagates from the underlying deserializer if the field is
/// neither a bool nor a string nor null.
pub fn deserialize_loose_bool<'de, D>(d: D) -> std::result::Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Bool(bool),
        Str(String),
    }
    Option::<Wire>::deserialize(d).map(|opt| match opt {
        None => false,
        Some(Wire::Bool(b)) => b,
        Some(Wire::Str(s)) => matches!(s.as_str(), "1" | "true" | "yes"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct AltOnly {
        #[serde(default, deserialize_with = "deserialize_alt_titles")]
        alt_titles: Vec<String>,
    }

    #[derive(Deserialize)]
    struct BoolOnly {
        #[serde(default, deserialize_with = "deserialize_loose_bool")]
        flag: bool,
    }

    #[test]
    fn alt_titles_decodes_a_json_array() {
        let v: AltOnly = serde_json::from_str(r#"{"alt_titles":["a","b"]}"#).expect("ok");
        assert_eq!(v.alt_titles, vec!["a", "b"]);
    }

    #[test]
    fn alt_titles_splits_a_newline_joined_string() {
        // The `\n` form is what the SSE-query path produces — the
        // frontend joins kitsu titles with newlines because
        // serde_urlencoded can't deserialize repeated keys.
        let v: AltOnly = serde_urlencoded::from_str("alt_titles=a%0Ab%0Ac").expect("ok");
        assert_eq!(v.alt_titles, vec!["a", "b", "c"]);
    }

    #[test]
    fn alt_titles_filters_empty_segments_out_of_the_joined_form() {
        let v: AltOnly = serde_urlencoded::from_str("alt_titles=a%0A%0Ab").expect("ok");
        assert_eq!(v.alt_titles, vec!["a", "b"]);
    }

    #[test]
    fn alt_titles_treats_explicit_null_and_missing_field_as_empty() {
        let null: AltOnly = serde_json::from_str(r#"{"alt_titles":null}"#).expect("ok");
        assert!(null.alt_titles.is_empty());
        let missing: AltOnly = serde_json::from_str(r#"{}"#).expect("ok");
        assert!(missing.alt_titles.is_empty());
    }

    #[test]
    fn loose_bool_accepts_truthy_strings() {
        for s in ["1", "true", "yes"] {
            let qs = format!("flag={s}");
            let v: BoolOnly = serde_urlencoded::from_str(&qs).expect("ok");
            assert!(v.flag, "expected true for {s:?}");
        }
    }

    #[test]
    fn loose_bool_rejects_other_strings_as_false() {
        // Anything that isn't 1/true/yes is false — including the
        // literal "false"/"0" the frontend may send when the user
        // toggled the flag back off. Pin the contract.
        for s in ["0", "false", "no", "wat", ""] {
            let qs = format!("flag={s}");
            let v: BoolOnly = serde_urlencoded::from_str(&qs).expect("ok");
            assert!(!v.flag, "expected false for {s:?}");
        }
    }

    #[test]
    fn loose_bool_passes_through_explicit_json_bools() {
        let t: BoolOnly = serde_json::from_str(r#"{"flag":true}"#).expect("ok");
        let f: BoolOnly = serde_json::from_str(r#"{"flag":false}"#).expect("ok");
        assert!(t.flag);
        assert!(!f.flag);
    }

    #[test]
    fn loose_bool_treats_null_and_missing_field_as_false() {
        let null: BoolOnly = serde_json::from_str(r#"{"flag":null}"#).expect("ok");
        assert!(!null.flag);
        let missing: BoolOnly = serde_json::from_str(r#"{}"#).expect("ok");
        assert!(!missing.flag);
    }
}
