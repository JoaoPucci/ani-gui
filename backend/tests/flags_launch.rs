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

/// The shipped build declares no flags yet, so the file's values cannot
/// show here; what this proves is that naming a file keeps the run a
/// test run and off the network. That a test run's file decides the
/// flags is `flags::launch`'s own tests, with test declarations.
#[tokio::test(flavor = "multi_thread")]
async fn a_test_run_takes_the_file_the_test_names() {
    let home = tempfile::tempdir().expect("tempdir");
    let file = home.path().join("flags.json");
    std::fs::write(&file, r#"{"flags":{}}"#).expect("write");
    let (url, rx) = proxy();
    let mut cmd = common::command(
        Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        home.path(),
    );
    through(&mut cmd, &url);
    cmd.env("ANI_GUI_TEST_FLAGS_FILE", &file);
    let backend = common::start(cmd);
    let body = flags(&backend.api_base).await;
    let heard = heard_raw(&rx, Duration::from_secs(2));
    stop(backend);
    assert_eq!(body, serde_json::json!({ "origin": "test", "flags": {} }));
    assert_eq!(heard, None);
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
