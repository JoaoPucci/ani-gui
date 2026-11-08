//! The launch fetches the file's signature beside it, and only a file
//! verified under a trusted key may turn a feature on.

use std::time::Duration;

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{fetch, fetch_trusting, FETCH_TIMEOUT, SIGNATURE_GRACE};
use crate::flags::test_decls::{A, B};

const FILE: &str = include_str!("../../tests/fixtures/flags-signature/flags.json");
const SIG: &str = include_str!("../../tests/fixtures/flags-signature/flags.json.minisig");
const SIG_BY_TWO: &str =
    include_str!("../../tests/fixtures/flags-signature/flags.json.by-key-two.minisig");
const KEY_ONE: &str = include_str!("../../tests/fixtures/flags-signature/test-key-one.pub");

async fn serve_delayed(file_ms: u64, sig: ResponseTemplate) -> (MockServer, String) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/flags/flags.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(FILE)
                .set_delay(Duration::from_millis(file_ms)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/flags/flags.json.minisig"))
        .respond_with(sig)
        .mount(&server)
        .await;
    let url = format!("{}/flags/flags.json", server.uri());
    (server, url)
}

async fn serve(signature: Option<&str>) -> (MockServer, String) {
    let sig = match signature {
        Some(s) => ResponseTemplate::new(200).set_body_string(s),
        None => ResponseTemplate::new(404).set_body_string("404: Not Found"),
    };
    serve_delayed(0, sig).await
}

async fn fetch_under_key_one(
    url: &str,
    timeout: Duration,
) -> Option<crate::flags::resolve::Document> {
    fetch_trusting(&reqwest::Client::new(), url, timeout, &[KEY_ONE.trim()]).await
}

#[tokio::test]
async fn a_file_signed_by_a_trusted_key_turns_its_feature_on() {
    let (server, url) = serve(Some(SIG)).await;
    let doc = fetch_under_key_one(&url, FETCH_TIMEOUT)
        .await
        .expect("document");
    assert_eq!(doc.get(A), Some(&json!(true)));
    assert_eq!(doc.get(B), Some(&json!(false)));
    let mut asked: Vec<String> = server
        .received_requests()
        .await
        .expect("recorded")
        .iter()
        .map(|r| r.url.path().to_string())
        .collect();
    asked.sort();
    assert_eq!(asked, ["/flags/flags.json", "/flags/flags.json.minisig"]);
}

#[tokio::test]
async fn an_unsigned_or_wrongly_signed_file_keeps_its_offs_and_loses_its_ons() {
    for sig in [None, Some(SIG_BY_TWO), Some("junk")] {
        let (_s, url) = serve(sig).await;
        let doc = fetch_under_key_one(&url, FETCH_TIMEOUT)
            .await
            .expect("document");
        assert_eq!(doc.get(A), None, "{sig:?}");
        assert_eq!(doc.get(B), Some(&json!(false)), "{sig:?}");
    }
}

#[tokio::test]
async fn the_shipped_build_trusts_no_signature_yet() {
    let (_s, url) = serve(Some(SIG)).await;
    let doc = fetch(&reqwest::Client::new(), &url, FETCH_TIMEOUT)
        .await
        .expect("document");
    assert_eq!(doc.get(A), None);
    assert_eq!(doc.get(B), Some(&json!(false)));
}

#[tokio::test]
async fn an_oversized_signature_is_no_signature() {
    let padded = format!(
        "{SIG}{}",
        " ".repeat(crate::flags::signature::MAX_SIGNATURE_BYTES)
    );
    let (_s, url) = serve(Some(&padded)).await;
    let doc = fetch_under_key_one(&url, FETCH_TIMEOUT)
        .await
        .expect("document");
    assert_eq!(doc.get(A), None);
}

#[tokio::test]
async fn a_signature_that_never_comes_costs_the_turn_on_not_the_flags() {
    let hung = ResponseTemplate::new(200)
        .set_body_string(SIG)
        .set_delay(Duration::from_secs(5));
    let (_s, url) = serve_delayed(0, hung).await;
    let started = std::time::Instant::now();
    let doc = fetch_under_key_one(&url, Duration::from_millis(800))
        .await
        .expect("the flags arrive without their signature");
    assert_eq!(doc.get(A), None);
    assert_eq!(doc.get(B), Some(&json!(false)));
    assert!(started.elapsed() < Duration::from_secs(3));
}

/// A file that arrives at once and a signature that hangs: the attempt
/// ends within the launch's wait, its offs kept and its ons dropped.
/// The signature's hold is bounded by its grace after the file, not by
/// the launch's wait: a file arriving late in the wait, beside a hung
/// signature, ends past it by up to the grace.
#[tokio::test]
async fn a_hung_signature_beside_a_prompt_file_ends_within_the_launchs_wait() {
    let hung = ResponseTemplate::new(200)
        .set_body_string(SIG)
        .set_delay(Duration::from_secs(60));
    let (_s, url) = serve_delayed(0, hung).await;
    let started = std::time::Instant::now();
    let doc = fetch_under_key_one(&url, FETCH_TIMEOUT)
        .await
        .expect("the flags arrive");
    assert!(
        started.elapsed() < crate::flags::store::SETTLE_CAP,
        "{:?}",
        started.elapsed()
    );
    assert_eq!(doc.get(B), Some(&json!(false)));
    assert_eq!(doc.get(A), None);
}

#[tokio::test]
async fn the_signature_is_asked_for_alongside_the_file_not_after_it() {
    // The signature takes longer than the grace it gets after the
    // file, so only a request made alongside the file arrives in time.
    let slow = ResponseTemplate::new(200)
        .set_body_string(SIG)
        .set_delay(SIGNATURE_GRACE + Duration::from_millis(1000));
    let (_s, url) = serve_delayed(2000, slow).await;
    let doc = fetch_under_key_one(&url, FETCH_TIMEOUT)
        .await
        .expect("document");
    assert_eq!(doc.get(A), Some(&json!(true)));
}

#[tokio::test]
async fn a_file_that_fails_does_not_wait_for_its_signature() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/flags/flags.json"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/flags/flags.json.minisig"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(SIG)
                .set_delay(Duration::from_secs(60)),
        )
        .mount(&server)
        .await;
    let url = format!("{}/flags/flags.json", server.uri());
    let started = std::time::Instant::now();
    let doc = fetch_under_key_one(&url, FETCH_TIMEOUT).await;
    assert_eq!(doc, None);
    assert!(started.elapsed() < Duration::from_secs(2));
}
