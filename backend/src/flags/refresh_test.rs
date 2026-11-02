//! The launch refresh against a scripted fetcher, on a paused clock.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::{refresh, retry_delay, RETRY_DELAYS, RETRY_EVERY};
use crate::flags::cache::{self, CacheEntry, FRESH_FOR};
use crate::flags::decl::FlagValue;
use crate::flags::resolve::{parse_document, Document};
use crate::flags::store::{FlagStore, Origin, SETTLE_CAP};
use crate::flags::test_decls::{FALLBACK, KILL, TEST_DECLS};

const NOW: u64 = 1_800_000_000;
const DAY: u64 = 24 * 60 * 60;

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

fn file_doc() -> Document {
    parse_document(br#"{"flags":{"test_kill":false,"test_fallback":"older"}}"#).expect("doc")
}

fn cached_doc() -> Document {
    parse_document(br#"{"flags":{"test_fallback":"old"}}"#).expect("doc")
}

/// A fetcher that plays `script` attempt by attempt — each a delay and
/// what the attempt returns — and fails every attempt past its end.
#[derive(Clone)]
struct Script {
    steps: Arc<Vec<(Duration, Option<Document>)>>,
    started: Arc<AtomicUsize>,
    finished: Arc<AtomicUsize>,
}

impl Script {
    fn new(steps: Vec<(Duration, Option<Document>)>) -> Self {
        Self {
            steps: Arc::new(steps),
            started: Arc::default(),
            finished: Arc::default(),
        }
    }

    fn fetcher(
        &self,
    ) -> impl FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Document>> + Send>>
    {
        let me = self.clone();
        move || {
            let i = me.started.fetch_add(1, Ordering::SeqCst);
            let (delay, result) = me.steps.get(i).cloned().unwrap_or((Duration::ZERO, None));
            let finished = Arc::clone(&me.finished);
            Box::pin(async move {
                tokio::time::sleep(delay).await;
                finished.fetch_add(1, Ordering::SeqCst);
                result
            })
        }
    }

    fn attempts(&self) -> usize {
        self.started.load(Ordering::SeqCst)
    }

    fn finished(&self) -> usize {
        self.finished.load(Ordering::SeqCst)
    }
}

fn cache_in(dir: &Path) -> PathBuf {
    dir.join("remote-flags.json")
}

async fn seed(path: &Path, fetched_at: u64, flags: Document) {
    cache::write(path, &CacheEntry { fetched_at, flags }).await;
}

/// Run a launch's refresh in its own task; hand back the store and the
/// task.
fn launch(cache: Option<PathBuf>, script: &Script) -> (FlagStore, tokio::task::JoinHandle<()>) {
    let store = FlagStore::pending(TEST_DECLS);
    let task = {
        let store = store.clone();
        let fetch = script.fetcher();
        tokio::spawn(async move { refresh(&store, cache.as_deref(), fetch, || NOW).await })
    };
    (store, task)
}

#[test]
fn retries_wait_three_ten_and_thirty_minutes_then_hourly() {
    assert_eq!(retry_delay(0), Duration::from_secs(180));
    assert_eq!(retry_delay(1), Duration::from_secs(600));
    assert_eq!(retry_delay(2), Duration::from_secs(1800));
    assert_eq!(retry_delay(3), RETRY_EVERY);
    assert_eq!(retry_delay(u32::MAX), RETRY_EVERY);
    assert_eq!(RETRY_DELAYS.len(), 3);
    assert_eq!(RETRY_EVERY, Duration::from_secs(3600));
}

#[tokio::test(start_paused = true)]
async fn a_fetch_within_the_cap_decides_the_flags_and_is_cached() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = cache_in(td.path());
    let script = Script::new(vec![(Duration::from_millis(300), Some(file_doc()))]);
    let (store, task) = launch(Some(path.clone()), &script);
    task.await.expect("join");
    assert_eq!(store.origin(), Origin::File);
    assert_eq!(pair(&store), FROM_FILE);
    let saved = cache::read(&path).await.expect("cached");
    assert_eq!((saved.fetched_at, saved.flags), (NOW, file_doc()));
    assert_eq!(script.attempts(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_fetch_past_the_cap_is_saved_for_the_next_launch_without_changing_this_one() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = cache_in(td.path());
    seed(&path, NOW - DAY, cached_doc()).await;
    let script = Script::new(vec![(Duration::from_secs(20), Some(file_doc()))]);
    let (store, task) = launch(Some(path.clone()), &script);

    store.decided().await;
    // Decided at the cap from the cache, with the fetch still out.
    assert_eq!(store.origin(), Origin::Cache);
    assert_eq!(pair(&store), FROM_CACHE);
    assert_eq!(script.finished(), 0);

    task.await.expect("join");
    assert_eq!(script.finished(), 1);
    // The session keeps what it decided...
    assert_eq!(store.origin(), Origin::Cache);
    assert_eq!(pair(&store), FROM_CACHE);
    // ...and the file is kept for the next launch.
    let saved = cache::read(&path).await.expect("cached");
    assert_eq!(saved.flags, file_doc());

    // The next launch misses the cap again, and uses what was saved.
    let next = Script::new(vec![(Duration::from_secs(20), None)]);
    let (store, task) = launch(Some(path.clone()), &next);
    store.decided().await;
    assert_eq!(store.origin(), Origin::Cache);
    assert_eq!(pair(&store), FROM_FILE);
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn a_slow_fetch_with_no_cache_decides_the_defaults_at_the_cap() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = cache_in(td.path());
    let script = Script::new(vec![(Duration::from_secs(20), Some(file_doc()))]);
    let (store, task) = launch(Some(path.clone()), &script);
    let started = tokio::time::Instant::now();
    store.decided().await;
    assert!(started.elapsed() <= SETTLE_CAP, "{:?}", started.elapsed());
    assert_eq!(store.origin(), Origin::Defaults);
    task.await.expect("join");
    assert_eq!(pair(&store), DEFAULTS);
    assert_eq!(cache::read(&path).await.expect("cached").flags, file_doc());
}

#[tokio::test(start_paused = true)]
async fn a_fetch_that_never_answers_still_decides_at_the_cap() {
    let script = Script::new(vec![(Duration::from_secs(1_000_000), Some(file_doc()))]);
    let (store, task) = launch(None, &script);
    let started = tokio::time::Instant::now();
    store.decided().await;
    assert!(started.elapsed() <= SETTLE_CAP, "{:?}", started.elapsed());
    assert_eq!(store.origin(), Origin::Defaults);
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn a_failed_fetch_is_retried_with_backoff_and_saved_when_one_succeeds() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = cache_in(td.path());
    let script = Script::new(vec![
        (Duration::ZERO, None),
        (Duration::ZERO, None),
        (Duration::ZERO, Some(file_doc())),
    ]);
    let (store, task) = launch(Some(path.clone()), &script);
    store.decided().await;
    assert_eq!(store.origin(), Origin::Defaults);
    let started = tokio::time::Instant::now();
    task.await.expect("join");
    assert_eq!(script.attempts(), 3);
    assert!(started.elapsed() >= retry_delay(0) + retry_delay(1));
    assert_eq!(cache::read(&path).await.expect("cached").flags, file_doc());
    // The success came mid-session: the session's flags stay.
    assert_eq!(store.origin(), Origin::Defaults);
    assert_eq!(pair(&store), DEFAULTS);
}

#[tokio::test(start_paused = true)]
async fn a_session_that_never_gets_the_file_keeps_retrying_hourly_and_its_flags() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = cache_in(td.path());
    seed(&path, NOW - DAY, cached_doc()).await;
    let script = Script::new(Vec::new());
    let (store, task) = launch(Some(path.clone()), &script);
    store.decided().await;
    // Three hours: the first attempt, then retries at 3, 13, 43, 103
    // and 163 minutes.
    tokio::time::sleep(Duration::from_secs(3 * 60 * 60)).await;
    assert_eq!(script.attempts(), 6);
    assert_eq!(store.origin(), Origin::Cache);
    assert_eq!(pair(&store), FROM_CACHE);
    assert_eq!(
        cache::read(&path).await.expect("kept").fetched_at,
        NOW - DAY
    );
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn without_a_cache_directory_a_failed_fetch_is_not_retried() {
    let script = Script::new(Vec::new());
    let (store, task) = launch(None, &script);
    task.await.expect("join");
    assert_eq!(store.origin(), Origin::Defaults);
    assert_eq!(script.attempts(), 1);
}

#[tokio::test(start_paused = true)]
async fn without_a_cache_directory_a_fetch_within_the_cap_still_decides() {
    let script = Script::new(vec![(Duration::from_millis(10), Some(file_doc()))]);
    let (store, task) = launch(None, &script);
    task.await.expect("join");
    assert_eq!(store.origin(), Origin::File);
    assert_eq!(pair(&store), FROM_FILE);
}

#[tokio::test(start_paused = true)]
async fn a_stale_or_future_cache_is_not_used() {
    for at in [NOW - FRESH_FOR.as_secs() - 1, NOW + DAY] {
        let td = tempfile::tempdir().expect("tempdir");
        let path = cache_in(td.path());
        seed(&path, at, cached_doc()).await;
        let script = Script::new(Vec::new());
        let (store, task) = launch(Some(path), &script);
        store.decided().await;
        assert_eq!(store.origin(), Origin::Defaults, "{at}");
        assert_eq!(pair(&store), DEFAULTS, "{at}");
        task.abort();
    }
}

#[tokio::test(start_paused = true)]
async fn a_cached_value_this_build_does_not_allow_resolves_to_the_default() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = cache_in(td.path());
    let bad =
        parse_document(br#"{"flags":{"test_kill":"off","test_fallback":"newest"}}"#).expect("doc");
    seed(&path, NOW, bad).await;
    let script = Script::new(Vec::new());
    let (store, task) = launch(Some(path), &script);
    store.decided().await;
    assert_eq!(store.origin(), Origin::Cache);
    assert_eq!(pair(&store), DEFAULTS);
    task.abort();
}

/// Everything a scoped subscriber wrote.
#[derive(Clone, Default)]
struct Captured(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}

#[test]
fn the_outcome_is_logged_under_the_backends_default_filter() {
    // The binary's filter when RUST_LOG is unset.
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("ani_gui=info"))
        .with_ansi(false)
        .with_writer(captured.clone())
        .finish();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    // See `meta::anilist_gate_test`: a second live dispatcher makes
    // callsite registration ask this test's subscriber too.
    let _second = tracing::Dispatch::new(tracing_subscriber::registry());
    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        runtime.block_on(async {
            let script = Script::new(Vec::new());
            let store = FlagStore::pending(TEST_DECLS);
            refresh(&store, None, script.fetcher(), || NOW).await;
        });
    });
    let out = String::from_utf8(captured.0.lock().expect("lock").clone()).expect("utf8");
    assert!(
        out.lines()
            .any(|l| l.contains("remote flags settled") && l.contains("defaults")),
        "{out}"
    );
}
