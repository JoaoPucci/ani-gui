//! Tests for the `/api/account` wire types. Moved here with the types
//! themselves so the handler module's CRAP stays under the ratchet.

use super::*;
use crate::account::pkce::PkceMethod;

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
fn tokens_response_round_trip_from_tokens() {
    // From<Tokens> for TokensResponse pins the wire shape — the shared
    // response used by the connect flow must not drop refresh_token /
    // expires_at_epoch_s on the way out.
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
fn refresh_request_deserialise() {
    let req: RefreshRequest = serde_json::from_str(r#"{"refresh_token":"rt-123"}"#).unwrap();
    assert_eq!(req.refresh_token, "rt-123");
}

#[test]
fn auth_url_response_serialise_round_trip() {
    let r = AuthUrlResponse {
        url: "https://anilist.co/x".into(),
    };
    let s = serde_json::to_string(&r).unwrap();
    assert!(s.contains("anilist.co/x"));
}

#[test]
fn disconnect_fallback_query_defaults_to_none_when_field_missing() {
    let q: DisconnectFallbackQuery = serde_urlencoded::from_str("").unwrap();
    assert!(q.fallback_user_id.is_none());
}

#[test]
fn disconnect_fallback_query_extracts_user_id() {
    let q: DisconnectFallbackQuery = serde_urlencoded::from_str("fallback_user_id=u7").unwrap();
    assert_eq!(q.fallback_user_id.as_deref(), Some("u7"));
}
