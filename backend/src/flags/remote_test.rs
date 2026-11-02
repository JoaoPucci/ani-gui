use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{fetch, FETCH_TIMEOUT, FLAGS_URL};
use crate::flags::resolve::MAX_BYTES;

const FILE: &str = "/flags/flags.json";

async fn serve(resp: ResponseTemplate) -> (MockServer, String) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(FILE))
        .respond_with(resp)
        .mount(&server)
        .await;
    let url = format!("{}{FILE}", server.uri());
    (server, url)
}

#[test]
fn the_file_is_read_from_the_flags_branch_on_githubs_content_cdn() {
    assert_eq!(
        FLAGS_URL,
        "https://raw.githubusercontent.com/JoaoPucci/ani-gui/flags/flags.json"
    );
    assert_eq!(FETCH_TIMEOUT, Duration::from_secs(5));
}

#[tokio::test]
async fn a_200_flags_document_is_returned() {
    let (_s, url) =
        serve(ResponseTemplate::new(200).set_body_string(r#"{"flags":{"test_kill":false}}"#)).await;
    let doc = fetch(&reqwest::Client::new(), &url, FETCH_TIMEOUT).await;
    let doc = doc.expect("document");
    assert_eq!(doc.get("test_kill"), Some(&serde_json::json!(false)));
}

#[tokio::test]
async fn a_404_is_no_document() {
    let (_s, url) = serve(ResponseTemplate::new(404).set_body_string("404: Not Found")).await;
    assert_eq!(
        fetch(&reqwest::Client::new(), &url, FETCH_TIMEOUT).await,
        None
    );
}

#[tokio::test]
async fn any_other_status_is_no_document() {
    for status in [204, 301, 403, 429, 500, 503] {
        let (_s, url) = serve(
            ResponseTemplate::new(status).set_body_string(r#"{"flags":{"test_kill":false}}"#),
        )
        .await;
        assert_eq!(
            fetch(&reqwest::Client::new(), &url, FETCH_TIMEOUT).await,
            None,
            "{status}"
        );
    }
}

#[tokio::test]
async fn malformed_json_is_no_document() {
    let (_s, url) = serve(ResponseTemplate::new(200).set_body_string("{\"flags\": {")).await;
    assert_eq!(
        fetch(&reqwest::Client::new(), &url, FETCH_TIMEOUT).await,
        None
    );
}

#[tokio::test]
async fn a_slow_response_times_out_as_no_document() {
    let (_s, url) = serve(
        ResponseTemplate::new(200)
            .set_body_string(r#"{"flags":{"test_kill":false}}"#)
            .set_delay(Duration::from_secs(10)),
    )
    .await;
    let started = std::time::Instant::now();
    let got = fetch(&reqwest::Client::new(), &url, Duration::from_millis(200)).await;
    assert_eq!(got, None);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn an_oversized_body_is_no_document() {
    let pad = " ".repeat(MAX_BYTES + 1);
    let (_s, url) = serve(
        ResponseTemplate::new(200)
            .set_body_string(format!(r#"{{"flags":{{"test_kill":false}}}}{pad}"#)),
    )
    .await;
    assert_eq!(
        fetch(&reqwest::Client::new(), &url, FETCH_TIMEOUT).await,
        None
    );
}

#[tokio::test]
async fn an_unreachable_host_is_no_document() {
    // Port 1 on loopback: nothing listens, the connect is refused.
    let got = fetch(
        &reqwest::Client::new(),
        "http://127.0.0.1:1/flags.json",
        FETCH_TIMEOUT,
    )
    .await;
    assert_eq!(got, None);
}
