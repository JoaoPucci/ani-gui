//! The flags file's signature: what is trusted, and what is not.

use serde_json::json;

use super::{admit, verify, Verdict};
use crate::flags::resolve::parse_document;
use crate::notices::NOTICES_KEY;

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
    let mut tampered = FILE.to_vec();
    let at = tampered
        .iter()
        .position(|b| *b == b'2')
        .expect("the ask number");
    tampered[at] = b'9';
    assert_eq!(
        verify(&tampered, Some(SIG), &keys(&[KEY_ONE])),
        Verdict::Invalid
    );
}

#[test]
fn a_tampered_trusted_comment_is_refused() {
    let sig = SIG.replace("timestamp:1793700000", "timestamp:1893700000");
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

#[test]
fn only_a_verified_file_keeps_its_notices_and_every_file_keeps_its_flags() {
    let doc = parse_document(FILE).expect("doc");
    let kept = admit(doc.clone(), Verdict::Verified);
    assert_eq!(kept, doc);
    for verdict in [Verdict::Unsigned, Verdict::Invalid] {
        let stripped = admit(doc.clone(), verdict);
        assert_eq!(stripped.get(NOTICES_KEY), None, "{verdict:?}");
        assert_eq!(
            stripped.get("some_feature"),
            Some(&json!(false)),
            "{verdict:?}"
        );
    }
}

#[test]
fn the_shipped_build_trusts_no_key_until_the_maintainer_adds_one() {
    assert!(super::TRUSTED_KEYS.is_empty());
    assert_eq!(super::SIGNATURE_SUFFIX, ".minisig");
}
