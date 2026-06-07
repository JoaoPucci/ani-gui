//! Tests for `crate::api::account`. Extracted via `#[path]` so the
//! route + extractor wiring complexity doesn't push `account.rs`'s
//! CCN past the ratchet — per `project_crap_inline_test_gotcha`.

use super::*;
use axum::http::header::AUTHORIZATION;

#[test]
fn parse_provider_accepts_known_slugs() {
    assert_eq!(parse_provider("anilist").unwrap(), ProviderKind::AniList);
    assert_eq!(parse_provider("mal").unwrap(), ProviderKind::MyAnimeList);
    assert_eq!(parse_provider("inhouse").unwrap(), ProviderKind::InHouse);
}

#[test]
fn parse_provider_rejects_unknown_slugs() {
    assert!(matches!(parse_provider(""), Err(AniError::Metadata)));
    assert!(matches!(parse_provider("anil"), Err(AniError::Metadata)));
    assert!(matches!(parse_provider("AniList"), Err(AniError::Metadata)));
}

#[test]
fn bearer_from_headers_extracts_token() {
    let mut h = HeaderMap::new();
    h.insert(AUTHORIZATION, "Bearer abc123".parse().unwrap());
    assert_eq!(bearer_from_headers(&h).unwrap(), "abc123");
}

#[test]
fn bearer_from_headers_rejects_missing() {
    let h = HeaderMap::new();
    assert!(matches!(
        bearer_from_headers(&h),
        Err(AniError::InvalidToken)
    ));
}

#[test]
fn bearer_from_headers_rejects_wrong_scheme() {
    let mut h = HeaderMap::new();
    h.insert(AUTHORIZATION, "Basic abc".parse().unwrap());
    assert!(matches!(
        bearer_from_headers(&h),
        Err(AniError::InvalidToken)
    ));
}

#[test]
fn bearer_from_headers_rejects_empty_token() {
    let mut h = HeaderMap::new();
    h.insert(AUTHORIZATION, "Bearer ".parse().unwrap());
    assert!(matches!(
        bearer_from_headers(&h),
        Err(AniError::InvalidToken)
    ));
}

#[test]
fn pkce_wire_round_trips() {
    let wire = PkceWire {
        verifier: "v".into(),
        challenge: "c".into(),
        method: "plain".into(),
    };
    let p = wire.into_pkce().unwrap();
    assert_eq!(p.method, PkceMethod::Plain);
    assert_eq!(p.verifier, "v");
    assert_eq!(p.challenge, "c");
}

#[test]
fn pkce_wire_rejects_unknown_method() {
    let wire = PkceWire {
        verifier: "v".into(),
        challenge: "c".into(),
        method: "s256".into(), // lowercase — must be S256
    };
    assert!(wire.into_pkce().is_none());
}
