//! The flags file's signature: which files verify, and under which keys.

use super::{verify, Verdict};

const FILE: &[u8] = include_bytes!("../../tests/fixtures/flags-signature/flags.json");
const SIG: &str = include_str!("../../tests/fixtures/flags-signature/flags.json.minisig");
const SIG_BY_TWO: &str =
    include_str!("../../tests/fixtures/flags-signature/flags.json.by-key-two.minisig");
const KEY_ONE: &str = include_str!("../../tests/fixtures/flags-signature/test-key-one.pub");
const KEY_TWO: &str = include_str!("../../tests/fixtures/flags-signature/test-key-two.pub");

fn keys(list: &[&'static str]) -> Vec<&'static str> {
    list.iter().map(|k| k.trim()).collect()
}

#[test]
fn a_file_signed_by_a_trusted_key_is_verified() {
    assert_eq!(
        verify(FILE, Some(SIG), &keys(&[KEY_ONE])),
        Verdict::Verified
    );
    assert_eq!(
        verify(FILE, Some(SIG), &keys(&[KEY_TWO, KEY_ONE])),
        Verdict::Verified,
        "any trusted key will do, so a rotation can ship the next key first"
    );
}

#[test]
fn a_missing_signature_is_unsigned() {
    assert_eq!(verify(FILE, None, &keys(&[KEY_ONE])), Verdict::Unsigned);
}

#[test]
fn a_signature_by_a_key_this_build_does_not_trust_is_refused() {
    assert_eq!(
        verify(FILE, Some(SIG_BY_TWO), &keys(&[KEY_ONE])),
        Verdict::Invalid
    );
}

#[test]
fn no_trusted_key_trusts_nothing() {
    assert_eq!(verify(FILE, Some(SIG), &[]), Verdict::Invalid);
}

#[test]
fn a_tampered_file_is_refused() {
    let tampered = String::from_utf8(FILE.to_vec())
        .expect("utf-8")
        .replace("\"test_a\": true", "\"test_a\": false");
    assert_ne!(tampered.as_bytes(), FILE);
    assert_eq!(
        verify(tampered.as_bytes(), Some(SIG), &keys(&[KEY_ONE])),
        Verdict::Invalid
    );
}

#[test]
fn a_tampered_trusted_comment_is_refused() {
    let sig = SIG.replace("timestamp:1793700000", "timestamp:1893700000");
    assert_ne!(sig, SIG);
    assert_eq!(
        verify(FILE, Some(&sig), &keys(&[KEY_ONE])),
        Verdict::Invalid
    );
}

#[test]
fn a_signature_that_is_not_one_is_refused() {
    for junk in ["", "hello", "untrusted comment: x\nnot base64\n"] {
        assert_eq!(
            verify(FILE, Some(junk), &keys(&[KEY_ONE])),
            Verdict::Invalid,
            "{junk:?}"
        );
    }
    assert_eq!(
        verify(FILE, Some(SIG), &["not a key"]),
        Verdict::Invalid,
        "a malformed trusted key trusts nothing"
    );
}

/// Pinned until the maintainer generates the signing key and adds its
/// public half. Adding it is the one-line change to `TRUSTED_KEYS`,
/// and this test changes with it; no other test depends on the list
/// being empty.
#[test]
fn the_shipped_build_trusts_no_key_until_the_maintainer_adds_one() {
    assert!(std::hint::black_box(super::TRUSTED_KEYS).is_empty());
    assert_eq!(super::SIGNATURE_SUFFIX, ".minisig");
}

#[test]
fn no_test_key_is_ever_trusted() {
    for key in [KEY_ONE, KEY_TWO] {
        assert!(
            !std::hint::black_box(super::TRUSTED_KEYS).contains(&key.trim()),
            "a fixture key, whose secret anyone can derive, is in TRUSTED_KEYS"
        );
    }
}
