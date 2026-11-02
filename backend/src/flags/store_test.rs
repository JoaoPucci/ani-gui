use std::path::Path;
use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{refresh, FlagStore, Source, SETTLE_LIMIT};
use crate::flags::cache::{self, CacheEntry, FRESH_FOR};
use crate::flags::decl::FlagValue;
use crate::flags::test_decls::{FALLBACK, KILL, TEST_DECLS};

const NOW: u64 = 1_800_000_000;
const DAY: u64 = 24 * 60 * 60;
const FILE: &str = "/flags/flags.json";

type Pair = (Option<FlagValue>, Option<FlagValue>);
const DEFAULTS: Pair = (Some(FlagValue::Bool(true)), Some(FlagValue::Choice("new")));
const FROM_FILE: Pair = (
    Some(FlagValue::Bool(false)),
    Some(FlagValue::Choice("older")),
);
const FROM_CACHE: Pair = (Some(FlagValue::Bool(true)), Some(FlagValue::Choice("old")));

fn pair(store: &FlagStore) -> Pair {
    let r = store.current();
    (r.get(KILL), r.get(FALLBACK))
}

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

fn source(url: &str, dir: &Path) -> Source {
    Source {
        url: url.to_string(),
        cache_path: dir.join("remote-flags.json"),
        timeout: Duration::from_millis(500),
    }
}

async fn seed_cache(src: &Source, fetched_at: u64, flags: serde_json::Value) {
    let flags = flags.as_object().expect("object").clone();
    cache::write(&src.cache_path, &CacheEntry { fetched_at, flags }).await;
}

fn file_body() -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_string(r#"{"flags":{"test_kill":false,"test_fallback":"older"}}"#)
}

fn cached_flags() -> serde_json::Value {
    serde_json::json!({ "test_fallback": "old" })
}

async fn run(
    resp: ResponseTemplate,
    cache_at: Option<u64>,
) -> (FlagStore, Source, tempfile::TempDir) {
    let td = tempfile::tempdir().expect("tempdir");
    let (server, url) = serve(resp).await;
    let src = source(&url, td.path());
    if let Some(at) = cache_at {
        seed_cache(&src, at, cached_flags()).await;
    }
    let store = FlagStore::pending(TEST_DECLS);
    refresh(&store, &reqwest::Client::new(), &src, NOW).await;
    drop(server);
    (store, src, td)
}

#[test]
fn a_new_store_holds_the_defaults_before_any_refresh() {
    let store = FlagStore::pending(TEST_DECLS);
    assert_eq!(pair(&store), DEFAULTS);
    assert!(!store.is_settled());
    let fixed = FlagStore::fixed(TEST_DECLS);
    assert_eq!(pair(&fixed), DEFAULTS);
    assert!(fixed.is_settled());
}

#[tokio::test]
async fn a_fetched_file_is_resolved_settled_and_cached() {
    let (store, src, _td) = run(file_body(), None).await;
    assert_eq!(pair(&store), FROM_FILE);
    assert!(store.is_settled());
    let cached = cache::read(&src.cache_path).await.expect("cached");
    assert_eq!(cached.fetched_at, NOW);
    assert_eq!(
        cached.flags.get("test_kill"),
        Some(&serde_json::json!(false))
    );
}

#[tokio::test]
async fn a_fetched_file_wins_over_a_fresh_cache_and_replaces_it() {
    let (store, src, _td) = run(file_body(), Some(NOW - DAY)).await;
    assert_eq!(pair(&store), FROM_FILE);
    assert_eq!(
        cache::read(&src.cache_path)
            .await
            .expect("cached")
            .fetched_at,
        NOW
    );
}

#[tokio::test]
async fn a_404_with_no_cache_resolves_to_the_defaults() {
    let (store, _src, _td) = run(ResponseTemplate::new(404), None).await;
    assert_eq!(pair(&store), DEFAULTS);
    assert!(store.is_settled());
}

#[tokio::test]
async fn a_404_falls_back_to_a_fresh_cache() {
    let (store, _src, _td) = run(ResponseTemplate::new(404), Some(NOW - DAY)).await;
    assert_eq!(pair(&store), FROM_CACHE);
    assert!(store.is_settled());
}

#[tokio::test]
async fn a_timeout_falls_back_to_a_fresh_cache() {
    let slow = file_body().set_delay(Duration::from_secs(10));
    let (store, _src, _td) = run(slow, Some(NOW - DAY)).await;
    assert_eq!(pair(&store), FROM_CACHE);
}

#[tokio::test]
async fn a_timeout_with_no_cache_resolves_to_the_defaults() {
    let slow = file_body().set_delay(Duration::from_secs(10));
    let (store, _src, _td) = run(slow, None).await;
    assert_eq!(pair(&store), DEFAULTS);
    assert!(store.is_settled());
}

#[tokio::test]
async fn malformed_json_falls_back_to_a_fresh_cache_and_keeps_it() {
    let bad = ResponseTemplate::new(200).set_body_string("{\"flags\":");
    let (store, src, _td) = run(bad, Some(NOW - DAY)).await;
    assert_eq!(pair(&store), FROM_CACHE);
    assert_eq!(
        cache::read(&src.cache_path).await.expect("kept").fetched_at,
        NOW - DAY
    );
}

#[tokio::test]
async fn a_stale_cache_is_not_used() {
    let stale = NOW - FRESH_FOR.as_secs() - 1;
    let (store, _src, _td) = run(ResponseTemplate::new(500), Some(stale)).await;
    assert_eq!(pair(&store), DEFAULTS);
}

#[tokio::test]
async fn a_cache_dated_in_the_future_is_not_used() {
    let (store, _src, _td) = run(ResponseTemplate::new(500), Some(NOW + DAY)).await;
    assert_eq!(pair(&store), DEFAULTS);
}

#[tokio::test]
async fn a_cached_value_this_build_does_not_allow_resolves_to_the_default() {
    let td = tempfile::tempdir().expect("tempdir");
    let (_server, url) = serve(ResponseTemplate::new(404)).await;
    let src = source(&url, td.path());
    seed_cache(
        &src,
        NOW,
        serde_json::json!({ "test_kill": "off", "test_fallback": "newest" }),
    )
    .await;
    let store = FlagStore::pending(TEST_DECLS);
    refresh(&store, &reqwest::Client::new(), &src, NOW).await;
    assert_eq!(pair(&store), DEFAULTS);
}

#[tokio::test]
async fn settled_answers_once_the_refresh_is_done() {
    let td = tempfile::tempdir().expect("tempdir");
    let (_server, url) = serve(file_body()).await;
    let src = source(&url, td.path());
    let store = FlagStore::pending(TEST_DECLS);
    let waiter = {
        let store = store.clone();
        tokio::spawn(async move { store.settled().await })
    };
    refresh(&store, &reqwest::Client::new(), &src, NOW).await;
    let r = tokio::time::timeout(Duration::from_secs(2), waiter)
        .await
        .expect("settled() returns once refreshed")
        .expect("join");
    assert_eq!((r.get(KILL), r.get(FALLBACK)), FROM_FILE);
}

#[tokio::test(start_paused = true)]
async fn settled_never_waits_past_its_limit() {
    let store = FlagStore::pending(TEST_DECLS);
    let started = tokio::time::Instant::now();
    let r = store.settled().await;
    assert_eq!((r.get(KILL), r.get(FALLBACK)), DEFAULTS);
    assert!(started.elapsed() >= SETTLE_LIMIT);
    assert!(started.elapsed() < SETTLE_LIMIT + Duration::from_secs(1));
}

#[tokio::test]
async fn the_launch_refresh_runs_off_the_caller() {
    // The shipped URL is never reached in a test: the task is aborted
    // straight away. What matters is that spawning returns at once
    // and leaves the store usable.
    let td = tempfile::tempdir().expect("tempdir");
    let store = FlagStore::pending(TEST_DECLS);
    let handle = super::spawn_launch_refresh(
        store.clone(),
        reqwest::Client::new(),
        td.path().join("remote-flags.json"),
    );
    handle.abort();
    assert_eq!(pair(&store), DEFAULTS);
}
