//! The verifier against signatures made by minisign's own code (the
//! `minisign` crate, a dev-dependency), not by a vector this
//! repository assembled: the prehashed `ED` algorithm, an untrusted
//! comment, a trusted comment and the global signature over it, as
//! `minisign -S` writes them.
//!
//! Every key here is generated for the run and dropped with it. None
//! is, or could be, one this build trusts, and a test holds that.

use std::io::Cursor;

use minisign::{KeyPair, SignatureBox};
use serde_json::json;

use super::{verify, Verdict, TRUSTED_KEYS};

/// A flags file that turns a feature on, which needs a verified
/// signature.
pub(crate) fn file() -> Vec<u8> {
    let doc = json!({ "flags": { "test_a": true, "test_b": false } });
    format!("{doc}\n").into_bytes()
}

pub(crate) struct Signed {
    pub(crate) public_key: String,
    pub(crate) signature: String,
}

/// Sign `body` as `minisign -S -t <comment>` would, with a key made
/// for this call.
pub(crate) fn sign(body: &[u8], trusted_comment: &str) -> Signed {
    let keys = KeyPair::generate_unencrypted_keypair().expect("a throwaway key");
    let sig = minisign::sign(
        Some(&keys.pk),
        &keys.sk,
        Cursor::new(body),
        Some(trusted_comment),
        Some("signature from minisign secret key"),
    )
    .expect("signed");
    Signed {
        public_key: keys.pk.to_base64(),
        signature: sig.into_string(),
    }
}

#[test]
fn a_signature_made_by_minisign_is_in_its_detached_prehashed_format() {
    let s = sign(&file(), "timestamp:1793700000\tfile:flags.json");
    let lines: Vec<&str> = s.signature.lines().collect();
    assert_eq!(lines.len(), 4, "{}", s.signature);
    assert!(lines[0].starts_with("untrusted comment: "));
    assert_eq!(
        lines[2],
        "trusted comment: timestamp:1793700000\tfile:flags.json"
    );
    let parsed = SignatureBox::from_string(&s.signature).expect("minisign reads it back");
    assert!(
        parsed.is_prehashed(),
        "the ED algorithm, minisign's default"
    );
}

#[test]
fn the_verifier_accepts_a_signature_minisign_made() {
    let body = file();
    let s = sign(&body, "flags.json 2026-11-03T12:00:00Z");
    assert_eq!(
        verify(&body, Some(&s.signature), &[&s.public_key]),
        Verdict::Verified
    );
}

#[test]
fn the_verifier_refuses_minisigns_signature_over_other_bytes_or_an_edited_comment() {
    let body = file();
    let s = sign(&body, "flags.json 2026-11-03T12:00:00Z");
    let mut other = body.clone();
    other.insert(0, b' ');
    assert_eq!(
        verify(&other, Some(&s.signature), &[&s.public_key]),
        Verdict::Invalid
    );
    let edited = s.signature.replace("2026-11-03", "2026-11-04");
    assert_ne!(edited, s.signature);
    assert_eq!(
        verify(&body, Some(&edited), &[&s.public_key]),
        Verdict::Invalid
    );
    let stranger = sign(&body, "flags.json");
    assert_eq!(
        verify(&body, Some(&stranger.signature), &[&s.public_key]),
        Verdict::Invalid
    );
}

#[test]
fn no_throwaway_key_is_one_this_build_trusts() {
    let s = sign(&file(), "flags.json");
    assert!(!TRUSTED_KEYS.contains(&s.public_key.as_str()));
    assert_eq!(
        verify(&file(), Some(&s.signature), TRUSTED_KEYS),
        Verdict::Invalid
    );
}
