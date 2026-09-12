use super::*;
use crate::scraper::provider::{StreamSource, SubtitleTrack};
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

/// Serves a track only while the flag file is absent: a signed URL
/// that expires the moment the transfer finishes, which is what the
/// stub tool marks by creating the flag.
struct UntilFlag(std::path::PathBuf);

impl Respond for UntilFlag {
    fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
        if self.0.exists() {
            ResponseTemplate::new(403)
        } else {
            ResponseTemplate::new(200).set_body_string("WEBVTT\n\n00:00.000 --> 00:01.000\nhi\n")
        }
    }
}

#[cfg(unix)]
fn stage_tool(dir: &std::path::Path, script: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("yt-dlp");
    // Writes `video` to whatever path follows `-o`, where a real
    // downloader puts its output.
    let writes_output = "prev=\"\"\nfor a in \"$@\"; do if [ \"$prev\" = \"-o\" ]; then printf 'video' > \"$a\"; fi; prev=\"$a\"; done";
    std::fs::write(
        &p,
        format!("#!/bin/sh\n{script}\n{writes_output}\nexit 0\n"),
    )
    .expect("stub");
    let mut perms = std::fs::metadata(&p).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&p, perms).expect("chmod");
    dir.display().to_string()
}

fn track(server: &MockServer) -> SubtitleTrack {
    SubtitleTrack {
        lang: "en".into(),
        label: "English".into(),
        default: true,
        url: format!("{}/subs/en.vtt", server.uri()),
    }
}

fn source(server: &MockServer) -> StreamSource {
    StreamSource {
        master_url: "https://cdn.example/x/master.m3u8".into(),
        referer: Some("https://embed.example/".into()),
        subtitles: vec![track(server)],
    }
}

/// A provider's subtitle URLs are signed and expire on their own,
/// and a transfer can run for an hour. Fetched after the transfer,
/// every track of a long download could be refused and the episode
/// would land without the subtitles it was resolved with. The
/// sidecar phase runs beside the transfer, so the track is asked for
/// while its URL is as fresh as the stream's.
#[cfg(unix)]
#[tokio::test]
async fn sidecars_are_fetched_while_the_transfer_runs() {
    let server = MockServer::start().await;
    let dest = tempfile::tempdir().expect("dest");
    let flag = dest.path().join("transfer-finished");
    Mock::given(method("GET"))
        .and(wm_path("/subs/en.vtt"))
        .respond_with(UntilFlag(flag.clone()))
        .mount(&server)
        .await;
    let bin = tempfile::tempdir().expect("bin");
    let path_env = stage_tool(bin.path(), &format!("sleep 2\ntouch '{}'", flag.display()));
    let written = transfer_with_sidecars(
        &reqwest::Client::new(),
        &source(&server),
        dest.path(),
        "Show Episode 1",
        Some("best"),
        &path_env,
        std::time::Duration::from_secs(30),
        &mut |_| {},
    )
    .await
    .expect("the transfer completes");
    assert!(
        dest.path().join("Show Episode 1.mp4").exists(),
        "the media is published"
    );
    assert_eq!(
        written,
        vec![dest.path().join("Show Episode 1.en.vtt")],
        "the track was fetched before the transfer finished, so it landed"
    );
}

/// A transfer that fails ends the phase: the error surfaces without
/// waiting on a track that is stalling, and an unfinished sidecar is
/// not left behind.
#[cfg(unix)]
#[tokio::test]
async fn a_failed_transfer_does_not_wait_on_a_stalling_track() {
    let server = MockServer::start().await;
    let dest = tempfile::tempdir().expect("dest");
    Mock::given(method("GET"))
        .and(wm_path("/subs/en.vtt"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("WEBVTT\n")
                .set_delay(std::time::Duration::from_secs(20)),
        )
        .mount(&server)
        .await;
    let bin = tempfile::tempdir().expect("bin");
    let path_env = stage_tool(bin.path(), "exit 1");
    let started = std::time::Instant::now();
    transfer_with_sidecars(
        &reqwest::Client::new(),
        &source(&server),
        dest.path(),
        "Show Episode 1",
        Some("best"),
        &path_env,
        std::time::Duration::from_secs(30),
        &mut |_| {},
    )
    .await
    .expect_err("the transfer failed");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "the failure surfaces without waiting out the track"
    );
    assert!(
        !dest.path().join("Show Episode 1.en.vtt").exists(),
        "no sidecar is left behind a failed transfer"
    );
}
