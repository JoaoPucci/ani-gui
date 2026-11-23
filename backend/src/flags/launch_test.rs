use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Duration;

use proptest::prelude::*;
use tokio::io::AsyncReadExt;

use super::{
    plan, plan_from_env, start, start_trusting, LaunchPlan, TEST_FLAGS_ENV, UNATTENDED_ENV,
};
use crate::flags::decl::FlagValue;
use crate::flags::store::{FlagStore, Origin};
use crate::flags::test_decls::{A, B, TEST_DECLS};

fn os(s: &str) -> Option<&OsStr> {
    Some(OsStr::new(s))
}

#[test]
fn a_normal_launch_reads_the_live_file_and_ignores_a_test_file() {
    let cache = Some(PathBuf::from("/c/remote-flags.json"));
    for unattended in [None, os("0"), os(""), os("true"), os("yes"), os(" 1")] {
        assert_eq!(
            plan(unattended, os("/t/flags.json"), cache.clone()),
            LaunchPlan::Live {
                cache: cache.clone()
            },
            "{unattended:?}"
        );
    }
}

#[test]
fn a_test_run_takes_the_tests_file_or_none() {
    assert_eq!(
        plan(os("1"), os("/t/flags.json"), None),
        LaunchPlan::Test {
            file: Some(PathBuf::from("/t/flags.json"))
        }
    );
    assert_eq!(plan(os("1"), None, None), LaunchPlan::Test { file: None });
    assert_eq!(plan(os("1"), os(""), None), LaunchPlan::Test { file: None });
}

proptest! {
    #[test]
    fn only_the_exact_switch_makes_a_test_run(unattended in proptest::option::of(".{0,4}"), file in proptest::option::of("[a-z/]{0,8}")) {
        let p = plan(unattended.as_deref().map(OsStr::new), file.as_deref().map(OsStr::new), None);
        prop_assert_eq!(matches!(p, LaunchPlan::Test { .. }), unattended.as_deref() == Some("1"));
    }
}

#[test]
fn the_plan_reads_this_process_environment() {
    let _guard = crate::config::paths::TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let saved = (
        std::env::var_os(UNATTENDED_ENV),
        std::env::var_os(TEST_FLAGS_ENV),
    );
    std::env::set_var(UNATTENDED_ENV, "1");
    std::env::set_var(TEST_FLAGS_ENV, "/t/flags.json");
    let test_run = plan_from_env(None);
    std::env::remove_var(UNATTENDED_ENV);
    let normal = plan_from_env(None);
    for (k, v) in [(UNATTENDED_ENV, saved.0), (TEST_FLAGS_ENV, saved.1)] {
        match v {
            Some(v) => std::env::set_var(k, v),
            None => std::env::remove_var(k),
        }
    }
    assert_eq!(
        test_run,
        LaunchPlan::Test {
            file: Some(PathBuf::from("/t/flags.json"))
        }
    );
    assert_eq!(normal, LaunchPlan::Live { cache: None });
}

/// A loopback listener standing in for an HTTPS proxy: every request a
/// client sends through it arrives here as a `CONNECT` line, so the
/// listener sees every host the client tried to reach.
struct Proxy {
    url: String,
    seen: tokio::sync::mpsc::UnboundedReceiver<String>,
}

async fn proxy() -> Proxy {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, seen) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 512];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let first = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let _ = tx.send(first);
                // Dropped unanswered: the request fails, as offline.
            });
        }
    });
    Proxy { url, seen }
}

fn client_through(proxy: &Proxy) -> reqwest::Client {
    reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(&proxy.url).expect("proxy"))
        .build()
        .expect("client")
}

fn pair(store: &FlagStore) -> (Option<FlagValue>, Option<FlagValue>) {
    let r = store.current();
    (r.get(A), r.get(B))
}

const SIGNED_FILE: &str = include_str!("../../tests/fixtures/flags-signature/flags.json");
const SIGNATURE: &str = include_str!("../../tests/fixtures/flags-signature/flags.json.minisig");
const KEY_ONE: &str = include_str!("../../tests/fixtures/flags-signature/test-key-one.pub");

/// The fixture's test key, as a build's key list.
fn key_one() -> &'static [&'static str] {
    let key: &'static str = Box::leak(KEY_ONE.trim().to_owned().into_boxed_str());
    Box::leak(vec![key].into_boxed_slice())
}

/// The signed fixture written as a test run's file, its signature
/// beside it when `signed`.
fn test_file(dir: &std::path::Path, signed: bool) -> std::path::PathBuf {
    let file = dir.join("flags.json");
    std::fs::write(&file, SIGNED_FILE).expect("write");
    if signed {
        std::fs::write(dir.join("flags.json.minisig"), SIGNATURE).expect("write");
    }
    file
}

/// A test run's file is admitted like a fetched one, so the values it
/// turns on show only under a signature the build trusts: here the
/// fixture's test key.
#[tokio::test]
async fn a_test_run_with_a_file_takes_its_flags_and_makes_no_request() {
    let td = tempfile::tempdir().expect("tempdir");
    let file = test_file(td.path(), true);
    let mut proxy = proxy().await;
    let store = FlagStore::pending(TEST_DECLS);
    start_trusting(
        store.clone(),
        client_through(&proxy),
        LaunchPlan::Test { file: Some(file) },
        key_one(),
    )
    .await
    .expect("join");
    assert_eq!(store.origin(), Origin::Test);
    assert_eq!(
        pair(&store),
        (Some(FlagValue::Bool(true)), Some(FlagValue::Bool(false)))
    );
    let none = tokio::time::timeout(Duration::from_millis(300), proxy.seen.recv()).await;
    assert!(none.is_err(), "a test run reached the network: {none:?}");
}

/// The same file with no signature beside it, or signed by a key the
/// shipped build does not trust, turns nothing on.
#[tokio::test]
async fn a_test_runs_unverified_file_cannot_turn_a_feature_on() {
    for (signed, shipped_keys) in [(false, false), (true, true)] {
        let td = tempfile::tempdir().expect("tempdir");
        let file = test_file(td.path(), signed);
        let store = FlagStore::pending(TEST_DECLS);
        let plan = LaunchPlan::Test { file: Some(file) };
        let task = if shipped_keys {
            start(store.clone(), reqwest::Client::new(), plan)
        } else {
            start_trusting(store.clone(), reqwest::Client::new(), plan, key_one())
        };
        task.await.expect("join");
        assert_eq!(store.origin(), Origin::Test);
        assert_eq!(
            pair(&store),
            (Some(FlagValue::Bool(false)), Some(FlagValue::Bool(false))),
            "signed {signed}, shipped keys {shipped_keys}"
        );
    }
}

#[tokio::test]
async fn a_test_run_without_a_file_or_with_an_unreadable_one_takes_the_defaults() {
    let td = tempfile::tempdir().expect("tempdir");
    for file in [None, Some(td.path().join("missing.json"))] {
        let mut proxy = proxy().await;
        let store = FlagStore::pending(TEST_DECLS);
        start(
            store.clone(),
            client_through(&proxy),
            LaunchPlan::Test { file: file.clone() },
        )
        .await
        .expect("join");
        assert_eq!(store.origin(), Origin::Test, "{file:?}");
        assert_eq!(
            pair(&store),
            (Some(FlagValue::Bool(false)), Some(FlagValue::Bool(false)))
        );
        let none = tokio::time::timeout(Duration::from_millis(300), proxy.seen.recv()).await;
        assert!(none.is_err(), "a test run reached the network: {none:?}");
    }
}

/// The control for the two above: the same client, on a normal
/// launch, does reach for raw.githubusercontent.com — so the silence
/// there is the test run's doing, not a proxy that hears nothing.
#[tokio::test]
async fn a_normal_launch_asks_for_the_live_file() {
    let mut proxy = proxy().await;
    let store = FlagStore::pending(TEST_DECLS);
    let task = start(
        store.clone(),
        client_through(&proxy),
        LaunchPlan::Live { cache: None },
    );
    let first = tokio::time::timeout(Duration::from_secs(10), proxy.seen.recv())
        .await
        .expect("a request")
        .expect("a line");
    assert!(
        first.starts_with("CONNECT raw.githubusercontent.com:443"),
        "{first}"
    );
    task.abort();
}
