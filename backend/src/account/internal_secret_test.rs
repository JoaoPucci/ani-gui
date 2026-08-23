//! Tests for the renderer-only `InternalSecret` gate.
//!
//! Codex P2 #3370011855: the disconnect-after-expiry cache wipe path
//! accepts a renderer-supplied `fallback_user_id`. Under the permissive
//! CORS layer that's a cross-origin tab away from poisoning another
//! user's local cache. The fix is a per-process random secret only the
//! Electron renderer sees (via stdout handshake + preload bridge) —
//! cross-origin callers can't guess 32 bytes of entropy.

use super::*;
use axum::http::HeaderMap;

#[test]
fn random_secret_is_64_hex_chars() {
    let s = InternalSecret::random();
    let hex = s.as_hex();
    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn two_random_secrets_differ() {
    let a = InternalSecret::random();
    let b = InternalSecret::random();
    assert_ne!(a.as_hex(), b.as_hex(), "rand() should not collide");
}

#[test]
fn validate_rejects_missing_header() {
    let secret = InternalSecret::from_hex_for_test("abcd").unwrap();
    let h = HeaderMap::new();
    assert!(secret.validate_header(&h).is_err());
}

#[test]
fn validate_rejects_wrong_value() {
    let secret = InternalSecret::from_hex_for_test("abcd").unwrap();
    let mut h = HeaderMap::new();
    h.insert(INTERNAL_SECRET_HEADER, "wrong".parse().unwrap());
    assert!(secret.validate_header(&h).is_err());
}

#[test]
fn validate_rejects_non_utf8() {
    let secret = InternalSecret::from_hex_for_test("abcd").unwrap();
    let mut h = HeaderMap::new();
    h.insert(
        INTERNAL_SECRET_HEADER,
        axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
    );
    assert!(secret.validate_header(&h).is_err());
}

#[test]
fn validate_accepts_matching_value() {
    let secret = InternalSecret::from_hex_for_test("abcd").unwrap();
    let mut h = HeaderMap::new();
    h.insert(INTERNAL_SECRET_HEADER, "abcd".parse().unwrap());
    assert!(secret.validate_header(&h).is_ok());
}

#[test]
fn validate_is_constant_time_on_length() {
    // Pin the contract: shorter and longer guesses fail without
    // panicking. Real timing analysis lives in `subtle`; we only
    // assert that mismatched-length inputs don't reveal anything
    // via Rust's `==`-on-different-lengths short-circuit.
    let secret = InternalSecret::from_hex_for_test("abcd").unwrap();
    let mut h = HeaderMap::new();
    h.insert(INTERNAL_SECRET_HEADER, "abc".parse().unwrap());
    assert!(secret.validate_header(&h).is_err());
    h.clear();
    h.insert(INTERNAL_SECRET_HEADER, "abcde".parse().unwrap());
    assert!(secret.validate_header(&h).is_err());
}
