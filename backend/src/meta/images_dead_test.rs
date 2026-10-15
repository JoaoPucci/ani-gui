use super::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// A URL the upstream says is gone is marked dead, keyed like the disk
// cache, so a reader of a remembered image URL can tell before asking
// the proxy again. Transient answers mark nothing.

async fn fetch(
    status: u16,
) -> (
    crate::cache::SqlitePool,
    String,
    Result<(Vec<u8>, &'static str)>,
) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/b.jpg"))
        .respond_with(ResponseTemplate::new(status).set_body_bytes(b"\xff\xd8\xff\xe0".to_vec()))
        .mount(&server)
        .await;
    let pool = crate::cache::open_in_memory().expect("pool");
    let dir = tempfile::TempDir::new().expect("tempdir");
    let url = format!("{}/b.jpg", server.uri());
    let result = get_or_fetch(&reqwest::Client::new(), dir.path(), &pool, &url).await;
    (pool, url, result)
}

#[tokio::test]
async fn an_upstream_404_marks_the_url_dead() {
    let (pool, url, result) = fetch(404).await;
    assert!(matches!(result, Err(AniError::Upstream { status: 404 })));
    assert!(is_dead(&pool, &url));
}

#[tokio::test]
async fn an_upstream_410_marks_the_url_dead() {
    let (pool, url, result) = fetch(410).await;
    assert!(matches!(result, Err(AniError::Upstream { status: 410 })));
    assert!(is_dead(&pool, &url));
}

#[tokio::test]
async fn a_transient_upstream_failure_marks_nothing() {
    for status in [429, 500, 503] {
        let (pool, url, result) = fetch(status).await;
        assert!(result.is_err(), "{status}");
        assert!(
            !is_dead(&pool, &url),
            "{status} is not a verdict on the URL"
        );
    }
}

#[tokio::test]
async fn a_served_image_is_not_dead_and_a_mark_can_be_forgotten() {
    let (pool, url, result) = fetch(200).await;
    assert!(result.is_ok());
    assert!(!is_dead(&pool, &url));
    mark_dead(&pool, &url);
    assert!(is_dead(&pool, &url));
    forget_dead(&pool, &url);
    assert!(!is_dead(&pool, &url));
}

#[test]
fn the_mark_is_keyed_like_the_disk_cache() {
    // A re-signed Kitsu URL is the same object; the mark follows the
    // canonical key, as the cached bytes do.
    let pool = crate::cache::open_in_memory().expect("pool");
    mark_dead(&pool, "https://x/a.jpg?X-Amz-Signature=1&X-Amz-Date=2");
    assert!(is_dead(
        &pool,
        "https://x/a.jpg?X-Amz-Signature=3&X-Amz-Date=4"
    ));
    assert!(!is_dead(&pool, "https://x/b.jpg"));
}
