//! A test run never asks GitHub for the flags file.
//!
//! The real binary, run the way the integration tests and the
//! end-to-end suites run it (`ANI_GUI_UNATTENDED=1`), with every
//! HTTPS request routed through a listener the test holds: the
//! listener hears each host the backend reaches for. A normal launch
//! is the control — the same listener hears it ask for
//! raw.githubusercontent.com — so the silence of a test run is the
//! test run's doing.
//!
//! It runs on Windows too: there the registry's proxy settings fill only
//! what the environment leaves empty, and the test sets the HTTPS
//! proxy in the environment.

mod common;

use std::io::Read;
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

const RAW: &str = "raw.githubusercontent.com";

/// A loopback listener that reports the first line of every
/// connection: for a proxied HTTPS request, `CONNECT <host>:443`.
fn proxy() -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for sock in listener.incoming().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut sock = sock;
                let mut buf = [0u8; 512];
                let n = sock.read(&mut buf).unwrap_or(0);
                let line = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let _ = tx.send(line);
            });
        }
    });
    (url, rx)
}

fn through(cmd: &mut std::process::Command, proxy: &str) {
    for var in ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"] {
        cmd.env(var, proxy);
    }
    cmd.env_remove("NO_PROXY").env_remove("no_proxy");
}

async fn flags(base: &str) -> serde_json::Value {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client")
        .get(format!("{base}/api/flags"))
        .send()
        .await
        .expect("flags")
        .json()
        .await
        .expect("json")
}

fn heard_raw(rx: &mpsc::Receiver<String>, within: Duration) -> Option<String> {
    let deadline = std::time::Instant::now() + within;
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(line) if line.contains(RAW) => return Some(line),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}

fn stop(mut backend: common::Backend) {
    let _ = backend.child.kill();
    let _ = backend.child.wait();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_test_run_makes_no_request_for_the_flags_file() {
    let home = tempfile::tempdir().expect("tempdir");
    let (url, rx) = proxy();
    let mut cmd = common::command(
        Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        home.path(),
    );
    through(&mut cmd, &url);
    let backend = common::start(cmd);
    let body = flags(&backend.api_base).await;
    let heard = heard_raw(&rx, Duration::from_secs(3));
    stop(backend);
    assert_eq!(body["origin"], "test", "{body}");
    assert_eq!(heard, None);
}

/// The declared flags at their defaults: none in a build without the
/// `test-run-key` feature, every release; the probe a test-run
/// build declares, off, in one with it.
fn defaults() -> serde_json::Value {
    if cfg!(feature = "test-run-key") {
        serde_json::json!({ "test_run_probe": false })
    } else {
        serde_json::json!({})
    }
}

/// The real binary in a test run whose file is `files[0]`, with the
/// rest written beside it, as `(name, contents)`. The flags it
/// answers with, and whether it reached for the live file.
async fn test_run_with(files: &[(&str, &str)]) -> (serde_json::Value, Option<String>) {
    let home = tempfile::tempdir().expect("tempdir");
    for (name, body) in files {
        std::fs::write(home.path().join(name), body).expect("write");
    }
    let (url, rx) = proxy();
    let mut cmd = common::command(
        Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        home.path(),
    );
    through(&mut cmd, &url);
    cmd.env("ANI_GUI_TEST_FLAGS_FILE", home.path().join(files[0].0));
    let backend = common::start(cmd);
    let body = flags(&backend.api_base).await;
    let heard = heard_raw(&rx, Duration::from_secs(2));
    stop(backend);
    (body, heard)
}

/// Naming a file keeps the run a test run and off the network, and the
/// file decides the flags: one that says nothing leaves every declared
/// flag at its default.
#[tokio::test(flavor = "multi_thread")]
async fn a_test_run_takes_the_file_the_test_names() {
    let (body, heard) = test_run_with(&[("flags.json", r#"{"flags":{}}"#)]).await;
    assert_eq!(
        body,
        serde_json::json!({ "origin": "test", "flags": defaults() })
    );
    assert_eq!(heard, None);
}

/// The binary a test-run build makes, with the probe: a file signed by
/// the test-run key turns it on; the same file unsigned, or signed by a
/// key no build trusts, leaves it off. The end-to-end suite runs the
/// same states on the packaged app (`electron/e2e/flags/`).
#[cfg(feature = "test-run-key")]
mod the_probe {
    use super::*;

    const ON: &str = include_str!("fixtures/flags-signature/probe-on.json");
    const BY_TEST_RUN_KEY: &str =
        include_str!("fixtures/flags-signature/probe-on.json.by-test-run-key.minisig");
    const BY_KEY_TWO: &str =
        include_str!("fixtures/flags-signature/probe-on.json.by-key-two.minisig");

    fn probe(body: &serde_json::Value) -> &serde_json::Value {
        &body["flags"]["test_run_probe"]
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_signed_by_the_test_run_key_turns_it_on() {
        let (body, _) =
            test_run_with(&[("flags.json", ON), ("flags.json.minisig", BY_TEST_RUN_KEY)]).await;
        assert_eq!(probe(&body), true, "{body}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_same_file_unsigned_leaves_it_off() {
        let (body, _) = test_run_with(&[("flags.json", ON)]).await;
        assert_eq!(probe(&body), false, "{body}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_same_file_signed_by_an_untrusted_key_leaves_it_off() {
        let (body, _) =
            test_run_with(&[("flags.json", ON), ("flags.json.minisig", BY_KEY_TWO)]).await;
        assert_eq!(probe(&body), false, "{body}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_normal_launch_asks_for_the_flags_file() {
    let home = tempfile::tempdir().expect("tempdir");
    let (url, rx) = proxy();
    let mut cmd = common::command(
        Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        home.path(),
    );
    through(&mut cmd, &url);
    cmd.env_remove("ANI_GUI_UNATTENDED");
    let backend = common::start(cmd);
    let heard = heard_raw(&rx, Duration::from_secs(10));
    stop(backend);
    let line = heard.expect("a normal launch asks for the file");
    assert!(
        line.starts_with("CONNECT raw.githubusercontent.com:443"),
        "{line}"
    );
}
