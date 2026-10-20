//! The backend listens on one loopback origin per class of traffic.
//!
//! Chromium opens at most six connections at a time to one host and
//! port over plain HTTP, and a request past the sixth waits for a
//! socket whatever it is for. Everything the renderer asked the
//! backend for used to share one origin, so requests that hold a
//! socket for as long as a provider takes — availability probes, play
//! resolutions, download progress, cold posters — could fill all six
//! and leave the player's playlist and segment fetches, and the next
//! page's metadata, queued behind them.
//!
//! So the backend binds a listener per lane and announces each in its
//! handshake, the stream proxy's URLs are stamped with the stream
//! lane's origin, and every lane serves the whole router: what reaches
//! a lane is the renderer's choice, and a URL composed against any of
//! them still answers.
//!
//! The real binary, run the way Electron runs it.

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The lanes beside the API that the handshake must announce.
const LANES: [&str; 5] = ["stream", "image", "probe", "resolve", "download"];

fn start() -> (common::Backend, tempfile::TempDir) {
    let home = tempfile::tempdir().expect("tempdir");
    let backend = common::start(common::command(
        Path::new(env!("CARGO_BIN_EXE_ani-gui-backend")),
        home.path(),
    ));
    (backend, home)
}

fn stop(mut backend: common::Backend) {
    let _ = backend.child.kill();
    let _ = backend.child.wait();
}

#[test]
fn the_handshake_announces_a_distinct_origin_for_every_lane() {
    let (backend, _home) = start();
    let announced = backend.lanes.clone();
    let api = backend.api_base.clone();
    stop(backend);

    let names: BTreeSet<&str> = announced.keys().map(String::as_str).collect();

    assert_eq!(
        names,
        LANES.into_iter().collect(),
        "lanes announced: {announced:?}"
    );
    let mut origins: BTreeSet<&str> = announced.values().map(String::as_str).collect();
    origins.insert(&api);
    assert_eq!(
        origins.len(),
        LANES.len() + 1,
        "every lane is an origin of its own: api {api}, {announced:?}"
    );
    for base in announced.values().chain(std::iter::once(&api)) {
        assert!(
            base.starts_with("http://127.0.0.1:"),
            "a lane stays on the loopback: {base}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn every_lane_serves_the_whole_router() {
    let (backend, _home) = start();
    let client = reqwest::Client::new();
    let mut bases: Vec<String> = backend.lanes.values().cloned().collect();
    bases.push(backend.api_base.clone());
    let mut failures = Vec::new();
    for base in &bases {
        for route in ["/healthz", "/api/app-info"] {
            match client.get(format!("{base}{route}")).send().await {
                Ok(r) if r.status().is_success() => {}
                other => failures.push(format!("{base}{route}: {other:?}")),
            }
        }
    }
    stop(backend);
    assert_eq!(
        bases.len(),
        LANES.len() + 1,
        "every lane is served: {bases:?}"
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_stream_proxy_is_stamped_with_the_stream_lane() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/master.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,RESOLUTION=640x360\nlow/index.m3u8\n",
        ))
        .mount(&upstream)
        .await;

    let (backend, _home) = start();
    let api = backend.api_base.clone();
    let stream = backend.lanes.get("stream").cloned();
    let client = reqwest::Client::new();

    let info: serde_json::Value = client
        .get(format!("{api}/api/app-info"))
        .send()
        .await
        .expect("app-info")
        .json()
        .await
        .expect("app-info json");
    let session: serde_json::Value = client
        .post(format!("{api}/api/sessions"))
        .json(&serde_json::json!({
            "upstream_url": format!("{}/master.m3u8", upstream.uri()),
            "referer": "",
        }))
        .send()
        .await
        .expect("session")
        .json()
        .await
        .expect("session json");
    let media_url = session["media_url"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let master = match client.get(&media_url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(e) => format!("{e:?}"),
    };
    stop(backend);

    let stream = stream.expect("a stream lane is announced");
    assert_eq!(info["proxy_base_url"], serde_json::json!(stream));
    assert!(
        media_url.starts_with(&format!("{stream}/s/")),
        "the player's URL is on the stream lane ({stream}): {media_url}"
    );
    let variant = master
        .lines()
        .find(|l| !l.starts_with('#') && !l.is_empty())
        .unwrap_or_default();
    assert!(
        variant.starts_with(&format!("{stream}/s/")),
        "the rewritten playlist names the stream lane ({stream}): {master}"
    );
}
