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
            let request = client
                .get(format!("{base}{route}"))
                .header("x-ani-gui-internal-secret", &backend.secret);
            match request.send().await {
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
async fn the_stream_proxy_base_is_the_stream_lane() {
    let (backend, _home) = start();
    let api = backend.api_base.clone();
    let stream = backend.lanes.get("stream").cloned();
    let info = app_info(&api, &backend.secret).await;
    stop(backend);

    let stream = stream.expect("a stream lane is announced");
    assert_eq!(info["proxy_base_url"], serde_json::json!(stream));
}

async fn app_info(api: &str, secret: &str) -> serde_json::Value {
    reqwest::Client::new()
        .get(format!("{api}/api/app-info"))
        .header("x-ani-gui-internal-secret", secret)
        .send()
        .await
        .expect("app-info")
        .json()
        .await
        .expect("app-info json")
}

/// The file the backend keeps its metadata cache in, found under the
/// test's home, where it lands on Linux and macOS.
#[cfg(not(target_os = "windows"))]
fn metadata_db(home: &Path) -> std::path::PathBuf {
    fn find(dir: &Path) -> Option<std::path::PathBuf> {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find(&path) {
                    return Some(found);
                }
            } else if path.file_name().is_some_and(|n| n == "metadata.sqlite") {
                return Some(path);
            }
        }
        None
    }
    find(home).unwrap_or_else(|| panic!("no metadata.sqlite under {}", home.display()))
}

/// A play's session, opened through the play route as the renderer
/// opens one, and the playlist the proxy serves for it, both name the
/// stream lane.
///
/// The backend answers the play from its resolution cache — the test
/// turns replays on through the settings route and writes the row the
/// play reads, naming a stream it serves — since a resolve would walk
/// a provider. Not on Windows: there the backend's cache is the dev
/// profile's, outside the test's home, and turning replays on would
/// change that profile's settings.
#[cfg(not(target_os = "windows"))]
#[tokio::test(flavor = "multi_thread")]
async fn a_plays_session_and_its_playlist_name_the_stream_lane() {
    use ani_gui::commands::play_resolution_cache::{cache_key, put, CachedResolution};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let upstream = MockServer::start().await;
    for verb in ["GET", "HEAD"] {
        Mock::given(method(verb))
            .and(path("/master.m3u8"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000,RESOLUTION=640x360\nlow/index.m3u8\n",
            ))
            .mount(&upstream)
            .await;
    }

    let (backend, home) = start();
    let api = backend.api_base.clone();
    let stream = backend.lanes.get("stream").cloned();
    let client = reqwest::Client::new();
    let secret = backend.secret.clone();

    let mut settings: serde_json::Value = client
        .get(format!("{api}/api/settings"))
        .header("x-ani-gui-internal-secret", &secret)
        .send()
        .await
        .expect("settings")
        .json()
        .await
        .expect("settings json");
    settings["cache_resolutions"] = serde_json::json!(true);
    let put_settings = client
        .put(format!("{api}/api/settings"))
        .header("x-ani-gui-internal-secret", &secret)
        .json(&settings)
        .send()
        .await
        .expect("put settings");
    assert!(put_settings.status().is_success(), "{put_settings:?}");

    let pool = ani_gui::cache::open_pool(&metadata_db(home.path())).expect("the backend's cache");
    put(
        &pool,
        &cache_key("Lanes Test Show", "sub", "1", None, None, None),
        &CachedResolution {
            upstream_url: format!("{}/master.m3u8", upstream.uri()),
            referer: String::new(),
            media_kind: ani_gui::proxy::MediaKind::Hls,
            show_id: "hianime:lanes-test-show-1".into(),
            show_title: "Lanes Test Show".into(),
            resolved_slot: None,
            played: Some("1".into()),
            former_episode: None,
            subtitles: Vec::new(),
            kitsu_id: None,
            versions: Vec::new(),
            version: None,
            episode_unverified: false,
        },
    );

    let session: serde_json::Value = client
        .post(format!("{api}/api/play"))
        .header("x-ani-gui-internal-secret", &secret)
        .json(&serde_json::json!({
            "title": "Lanes Test Show",
            "mode": "sub",
            "episode": "1",
        }))
        .send()
        .await
        .expect("play")
        .json()
        .await
        .expect("play json");
    let media_url = session["media_url"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let master = match client.get(&media_url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(e) => format!("{e:?}"),
    };
    stop(backend);

    assert_eq!(
        session["cache_hit"],
        serde_json::json!(true),
        "the play was the replay of the row: {session}"
    );
    let stream = stream.expect("a stream lane is announced");
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
