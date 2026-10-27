//! An HTTP error answer carries the variant's stable key beside its
//! `kind`, as the SSE error event does: the backend returns keys, the
//! frontend resolves them (AGENTS.md §4), and a key that never leaves
//! the backend resolves nothing.

use axum::response::IntoResponse;
use http_body_util::BodyExt;

use crate::error::variants::every_variant;
use crate::error::AniError;

#[tokio::test]
async fn every_http_error_carries_its_kind_and_stable_key() {
    for e in every_variant() {
        let key = e.key();
        let kind = serde_json::to_value(&e).expect("serializes")["kind"].clone();
        let status = e.http_status_code();
        let r = e.into_response();
        assert_eq!(r.status().as_u16(), status, "{key}");
        let bytes = r.into_body().collect().await.expect("body").to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
        assert_eq!(v["key"], key, "{v}");
        assert_eq!(v["kind"], kind, "{v}");
    }
}

/// Variant data rides along unchanged beside the key.
#[tokio::test]
async fn an_http_error_keeps_its_variant_fields() {
    let r = AniError::RateLimited {
        retry_after_secs: Some(9),
        refused_by: None,
    }
    .into_response();
    let bytes = r.into_body().collect().await.expect("body").to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(v["retry_after_secs"], 9, "{v}");
    assert_eq!(v["key"], "error.network.rate_limited", "{v}");
}

/// The tracker that refused rides along with the wait, so the renderer's
/// notice can name it; an error that names none carries no such field.
#[tokio::test]
async fn a_rate_limit_names_the_tracker_that_refused_when_one_did() {
    for (refused_by, expected) in [
        (Some("anilist"), serde_json::json!("anilist")),
        (Some("mal"), serde_json::json!("mal")),
        (None, serde_json::Value::Null),
    ] {
        let r = AniError::RateLimited {
            retry_after_secs: Some(9),
            refused_by,
        }
        .into_response();
        let bytes = r.into_body().collect().await.expect("body").to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
        assert_eq!(v["refused_by"], expected, "{v}");
        assert_eq!(v.get("refused_by").is_some(), refused_by.is_some(), "{v}");
    }
}
