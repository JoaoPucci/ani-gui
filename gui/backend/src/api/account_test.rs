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

#[test]
fn bearer_from_headers_accepts_extra_whitespace_after_scheme() {
    use axum::http::HeaderMap;
    let mut h = HeaderMap::new();
    h.insert(AUTHORIZATION, "Bearer    spaced-token  ".parse().unwrap());
    assert_eq!(bearer_from_headers(&h).unwrap(), "spaced-token");
}

#[test]
fn tokens_response_round_trip_from_tokens() {
    // From<Tokens> for TokensResponse pins the wire shape. Codex P2
    // #3369941703 wired bearer auth onto get_cached_list /
    // delete_list_cache; this test pins that the shared response
    // shape used by the connect flow doesn't accidentally drop the
    // refresh_token / expires_at_epoch_s on the way out.
    let t = crate::commands::account::tokens_from_bearer("xyz");
    let resp: TokensResponse = t.into();
    let s = serde_json::to_string(&resp).unwrap();
    assert!(s.contains("\"access_token\":\"xyz\""));
    assert!(s.contains("\"refresh_token\":null"));
    assert!(s.contains("\"expires_at_epoch_s\":0"));
}

#[test]
fn auth_url_request_deserialise() {
    // Pin the wire shape from the renderer side — the renderer's
    // PkceWire JSON matches the backend's expectation.
    let body = r#"{
        "state": "csrf-token",
        "pkce": { "verifier": "v", "challenge": "c", "method": "plain" }
    }"#;
    let req: AuthUrlRequest = serde_json::from_str(body).unwrap();
    assert_eq!(req.state, "csrf-token");
    assert_eq!(req.pkce.method, "plain");
}

#[test]
fn exchange_code_request_deserialise() {
    let body = r#"{
        "code": "auth-code",
        "pkce": { "verifier": "v", "challenge": "v", "method": "plain" }
    }"#;
    let req: ExchangeCodeRequest = serde_json::from_str(body).unwrap();
    assert_eq!(req.code, "auth-code");
}

#[test]
fn list_request_deserialise_with_user_id() {
    let body = r#"{ "user_id": "u-7" }"#;
    let req: ListRequest = serde_json::from_str(body).unwrap();
    assert_eq!(req.user_id, "u-7");
}

#[test]
fn auth_url_response_serialise_round_trip() {
    let r = AuthUrlResponse {
        url: "https://anilist.co/x".into(),
    };
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains("anilist.co/x"));
}
