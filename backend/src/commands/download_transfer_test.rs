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

fn second_track(server: &MockServer) -> SubtitleTrack {
    SubtitleTrack {
        lang: "pt".into(),
        label: "Português".into(),
        default: false,
        url: format!("{}/subs/pt.vtt", server.uri()),
    }
}

/// Every entry in `dir` whose name ends in `.vtt` — the sidecars at
/// their names and any scratch sibling left behind.
fn vtt_entries(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dest")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".vtt"))
        .collect();
    names.sort();
    names
}

/// A track can land before the transfer fails, since the two run
/// side by side. A sidecar left beside a failed download is one the
/// next attempt keeps as the user's own and refuses to refresh, so
/// nothing this transfer fetched stays at a name once the transfer
/// has failed — while a file the user had at a track's name before
/// the download is not this transfer's to touch.
#[cfg(unix)]
#[tokio::test]
async fn a_failed_transfer_leaves_no_sidecar_it_fetched() {
    let server = MockServer::start().await;
    let dest = tempfile::tempdir().expect("dest");
    Mock::given(method("GET"))
        .and(wm_path("/subs/en.vtt"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("WEBVTT\n\n00:00.000 --> 00:01.000\nhi\n"),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wm_path("/subs/pt.vtt"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("WEBVTT\n\n00:00.000 --> 00:01.000\noi\n"),
        )
        .mount(&server)
        .await;
    let users_own = dest.path().join("Show Episode 1.pt.vtt");
    std::fs::write(&users_own, "WEBVTT\n\ncorrected by hand\n").expect("user file");
    let mut source = source(&server);
    source.subtitles.push(second_track(&server));
    let bin = tempfile::tempdir().expect("bin");
    let path_env = stage_tool(bin.path(), "sleep 2\nexit 1");
    transfer_with_sidecars(
        &reqwest::Client::new(),
        &source,
        dest.path(),
        "Show Episode 1",
        Some("best"),
        &path_env,
        std::time::Duration::from_secs(30),
        &mut |_| {},
    )
    .await
    .expect_err("the transfer failed");
    assert_eq!(
        vtt_entries(dest.path()),
        vec!["Show Episode 1.pt.vtt".to_string()],
        "the fetched track is gone, no scratch remains, the user's file stays"
    );
    assert_eq!(
        std::fs::read_to_string(&users_own).expect("user file"),
        "WEBVTT\n\ncorrected by hand\n",
        "a file the user had at the name is not this transfer's to touch"
    );
}

/// A sidecar takes its name only beside a transfer that succeeded:
/// while the tool is still running, a track that has arrived waits
/// in its scratch, and the name is filled once the media is there.
#[cfg(unix)]
#[tokio::test]
async fn a_sidecar_takes_its_name_only_once_the_transfer_has_succeeded() {
    let server = MockServer::start().await;
    let dest = tempfile::tempdir().expect("dest");
    Mock::given(method("GET"))
        .and(wm_path("/subs/en.vtt"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("WEBVTT\n\n00:00.000 --> 00:01.000\nhi\n"),
        )
        .mount(&server)
        .await;
    let name = dest.path().join("Show Episode 1.en.vtt");
    let seen_early = dest.path().join("sidecar-seen-during-transfer");
    let bin = tempfile::tempdir().expect("bin");
    // The tool looks for the sidecar at its name after the track has
    // had time to land, and marks it if it is already there.
    let path_env = stage_tool(
        bin.path(),
        &format!(
            "sleep 2\nif [ -e '{}' ]; then touch '{}'; fi",
            name.display(),
            seen_early.display()
        ),
    );
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
        !seen_early.exists(),
        "the name was not taken while the transfer was still running"
    );
    assert_eq!(written, vec![name.clone()]);
    assert_eq!(
        vtt_entries(dest.path()),
        vec!["Show Episode 1.en.vtt".to_string()],
        "the sidecar is at its name and no scratch remains"
    );
}
