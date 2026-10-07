//! An HTTP error answer carries the variant's stable key beside its
//! `kind`, as the SSE error event does: the backend returns keys, the
//! frontend resolves them (AGENTS.md §4), and a key that never leaves
//! the backend resolves nothing.

use axum::response::IntoResponse;
use http_body_util::BodyExt;

use crate::error::AniError;

/// One of each variant. The match below has no wildcard, so a new
/// variant fails to compile here until it is listed.
fn every_variant() -> Vec<AniError> {
    let all = vec![
        AniError::Scraper {
            key: "error.scraper.custom_test_key",
        },
        AniError::Timeout,
        AniError::NoResults,
        AniError::EpisodeUnavailable,
        AniError::RateLimited {
            retry_after_secs: Some(9),
        },
        AniError::ParseFailed { detail: "x".into() },
        AniError::FfmpegMissing,
        AniError::Upstream { status: 503 },
        AniError::Network,
        AniError::GateRefused,
        AniError::PlayerSpawnFailed {
            binary: "vlc".into(),
        },
        AniError::SyncplaySpawnFailed {
            binary: "syncplay".into(),
        },
        AniError::Cache,
        AniError::Io,
        AniError::Config,
        AniError::Metadata,
        AniError::UnsupportedPkce,
        AniError::InvalidKitsuId,
        AniError::InvalidToken,
    ];
    for e in &all {
        match e {
            AniError::Scraper { .. }
            | AniError::Timeout
            | AniError::NoResults
            | AniError::EpisodeUnavailable
            | AniError::RateLimited { .. }
            | AniError::ParseFailed { .. }
            | AniError::FfmpegMissing
            | AniError::Upstream { .. }
            | AniError::Network
            | AniError::GateRefused
            | AniError::PlayerSpawnFailed { .. }
            | AniError::SyncplaySpawnFailed { .. }
            | AniError::Cache
            | AniError::Io
            | AniError::Config
            | AniError::Metadata
            | AniError::UnsupportedPkce
            | AniError::InvalidKitsuId
            | AniError::InvalidToken => {}
        }
    }
    all
}

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
    }
    .into_response();
    let bytes = r.into_body().collect().await.expect("body").to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(v["retry_after_secs"], 9, "{v}");
    assert_eq!(v["key"], "error.network.rate_limited", "{v}");
}
