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
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{verify, Verdict, TRUSTED_KEYS};
use crate::flags::remote::{fetch_trusting, FETCH_TIMEOUT};
use crate::flags::resolve::resolve;
use crate::flags::DECLARED;
use crate::notices::{NOTICES_KEY, NOTIFICATION_CENTER};

/// A flags file that turns the notification center on and carries a
/// notice: both need a verified signature.
fn file() -> Vec<u8> {
    let doc = json!({ "flags": {
        "notification_center": true,
        "notices": { "format": 1, "donation_ask": 1, "items": [{ "id": "a", "title": "A notice" }] }
    }});
    format!("{doc}\n").into_bytes()
}

struct Signed {
    public_key: String,
    signature: String,
}

/// Sign `body` as `minisign -S -t <comment>` would, with a key made
/// for this call.
fn sign(body: &[u8], trusted_comment: &str) -> Signed {
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

async fn serve(body: Vec<u8>, signature: Option<String>) -> (MockServer, String) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/flags/flags.json"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&server)
        .await;
    let sig = match signature {
        Some(s) => ResponseTemplate::new(200).set_body_string(s),
        None => ResponseTemplate::new(404),
    };
    Mock::given(method("GET"))
        .and(path("/flags/flags.json.minisig"))
        .respond_with(sig)
        .mount(&server)
        .await;
    let url = format!("{}/flags/flags.json", server.uri());
    (server, url)
}

/// End to end through the launch's fetch: a file minisign signed, under
/// a trusted key, turns the center on and keeps its notices.
#[tokio::test]
async fn a_file_minisign_signed_turns_the_center_on_and_keeps_its_notices() {
    let body = file();
    let s = sign(&body, "flags.json");
    let (_server, url) = serve(body, Some(s.signature)).await;
    let doc = fetch_trusting(
        &reqwest::Client::new(),
        &url,
        FETCH_TIMEOUT,
        &[&s.public_key],
    )
    .await
    .expect("the file");
    assert!(doc.get(NOTICES_KEY).is_some());
    assert!(resolve(DECLARED, Some(&doc)).enabled(&NOTIFICATION_CENTER));
}

/// The same file unsigned, or signed by a key the build does not
/// trust: the center stays off and the notices are gone.
#[tokio::test]
async fn the_same_file_unverified_leaves_the_center_off_and_drops_its_notices() {
    let body = file();
    let trusted = sign(&body, "flags.json");
    let stranger = sign(&body, "flags.json");
    for signature in [None, Some(stranger.signature)] {
        let (_server, url) = serve(body.clone(), signature).await;
        let doc = fetch_trusting(
            &reqwest::Client::new(),
            &url,
            FETCH_TIMEOUT,
            &[&trusted.public_key],
        )
        .await
        .expect("the flags arrive");
        assert_eq!(doc.get(NOTICES_KEY), None);
        assert!(!resolve(DECLARED, Some(&doc)).enabled(&NOTIFICATION_CENTER));
    }
}
