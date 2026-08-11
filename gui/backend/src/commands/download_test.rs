use super::*;

#[cfg(unix)]
fn stage_tool(dir: &std::path::Path, name: &str, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{script}\n")).expect("write stub");
    let mut perms = std::fs::metadata(&p).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&p, perms).expect("chmod");
}

#[test]
fn find_tool_widens_names_with_the_platform_suffix_table() {
    // Windows installs name the tools yt-dlp.exe / ffmpeg.exe.
    // The scan builds and tests each full path itself — Windows
    // command resolution never runs, so PATHEXT cannot widen the
    // name for us. The suffix table is explicit, like the curl
    // transport's resolver.
    let bin = tempfile::tempdir().expect("bin");
    std::fs::write(bin.path().join("yt-dlp.exe"), b"").expect("stage");
    let path_env = bin.path().display().to_string();
    assert_eq!(
        find_tool(&path_env, "yt-dlp", &["", ".exe"]),
        Some(bin.path().join("yt-dlp.exe"))
    );
    // The bare name still wins where both exist.
    std::fs::write(bin.path().join("ffmpeg.exe"), b"").expect("stage");
    std::fs::write(bin.path().join("ffmpeg"), b"").expect("stage");
    assert_eq!(
        find_tool(&path_env, "ffmpeg", &["", ".exe"]),
        Some(bin.path().join("ffmpeg"))
    );
    // A name on no suffix stays a miss.
    assert_eq!(find_tool(&path_env, "aria2c", &["", ".exe"]), None);
}

#[cfg(unix)]
#[tokio::test]
async fn tool_spawn_prefers_ytdlp_and_passes_v5_arguments() {
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(bin.path(), "yt-dlp", "echo \"ytdlp $*\" >&2; exit 0");
    stage_tool(bin.path(), "ffmpeg", "echo ffmpeg-ran >&2; exit 0");
    let mut lines = Vec::new();
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Show Episode 2",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |l: &str| lines.push(l.to_string()),
    )
    .await
    .expect("yt-dlp path succeeds");
    let joined = lines.join("\n");
    assert!(joined.contains("ytdlp"), "yt-dlp ran: {joined}");
    assert!(
        joined.contains("--fragment-retries infinite") && joined.contains("-N 16"),
        "v5's arguments ride along: {joined}"
    );
    assert!(
        joined.contains("Show Episode 2.mp4"),
        "the target name matches the CLI's shape: {joined}"
    );
    assert!(!joined.contains("ffmpeg-ran"), "no fallback on success");
}

#[cfg(unix)]
#[tokio::test]
async fn tool_spawn_maps_quality_onto_ytdlps_resolution_sort() {
    // v5's -d downloads the variant select_quality chose; the
    // native tool spawn hands yt-dlp the master and expresses the
    // same preference through its resolution sort.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(bin.path(), "yt-dlp", "echo \"ytdlp $*\" >&2; exit 0");
    let mut lines = Vec::new();
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "X",
        Some("720"),
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |l: &str| lines.push(l.to_string()),
    )
    .await
    .expect("runs");
    assert!(
        lines.join("\n").contains("-S res:720"),
        "numeric quality becomes a resolution sort: {lines:?}"
    );

    let mut lines = Vec::new();
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "X",
        Some("worst"),
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |l: &str| lines.push(l.to_string()),
    )
    .await
    .expect("runs");
    assert!(
        lines.join("\n").contains("-S +res"),
        "worst prefers the smallest variant: {lines:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn tool_spawn_falls_back_to_ffmpeg_when_ytdlp_fails() {
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(bin.path(), "yt-dlp", "echo boom >&2; exit 1");
    stage_tool(bin.path(), "ffmpeg", "echo \"ffmpeg $*\" >&2; exit 0");
    let mut lines = Vec::new();
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Show Episode 2",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |l: &str| lines.push(l.to_string()),
    )
    .await
    .expect("ffmpeg fallback succeeds");
    let joined = lines.join("\n");
    assert!(
        joined.contains("-c copy"),
        "ffmpeg stream-copies the resolved url: {joined}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn tool_spawn_with_no_tools_is_a_config_error() {
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let got = spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "X",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(5),
        &mut |_l: &str| {},
    )
    .await;
    assert!(matches!(got, Err(AniError::FfmpegMissing)));
}

fn native_test_state(td: &tempfile::TempDir, anidb_base: &str) -> crate::app::AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::sync::Arc;
    crate::app::AppState {
        allanime_base: None,
        anidb_base: Some(anidb_base.to_string()),
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: std::path::PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        botan_shim_bin: None,
        history_path: td.path().join("ani-hsts"),
        scraper_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        image_cache_dir: td.path().join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem cache pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: td.path().join("config.toml"),
        state_dir: std::path::PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// Provider fixture for the range tests: one show, two episodes,
/// jpn embeds, validating masters. Unix-gated with the stub-tool
/// tests that drive it.
#[cfg(unix)]
async fn stub_range_show() -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/browse"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"<a href="/anime/range-show-21"><img alt="Range Show"/></a>"#),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/api/frontend/anime/21/episodes"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"episodes":[{"id":2101,"number":1},{"id":2102,"number":2}]}"#),
        )
        .mount(&server)
        .await;
    for ep in [2101u64, 2102] {
        wiremock::Mock::given(method("GET"))
            .and(path(format!("/api/frontend/episode/{ep}/languages")))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(format!(
                    r#"{{"languages":[{{"code":"jpn","embed_url":"{}/embed/{ep}"}}]}}"#,
                    server.uri()
                )),
            )
            .mount(&server)
            .await;
        wiremock::Mock::given(method("GET"))
            .and(path(format!("/embed/{ep}")))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(format!(
                    "player.setup({{ file: '{}/m/{ep}/master.m3u8' }});",
                    server.uri()
                )),
            )
            .mount(&server)
            .await;
        wiremock::Mock::given(method("GET"))
            .and(path(format!("/m/{ep}/master.m3u8")))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
            .mount(&server)
            .await;
    }
    server
}

#[cfg(unix)]
#[tokio::test]
async fn a_range_download_resolves_and_spawns_per_episode() {
    // The Download All/Range UI sends "1-12"-shaped episode
    // values; ani-cli's own `-e 1-12` looped over episodes. The
    // native path must do the same: one pick, then per-episode
    // resolution and one tool run per episode — not hand the
    // whole range to the episode resolver, which matches a single
    // tag and dies with NoResults before any transfer starts.
    let server = stub_range_show().await;
    let td = tempfile::tempdir().expect("td");
    let state = native_test_state(&td, &server.uri());
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let log = dest.path().join("calls.log");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("echo \"$*\" >> '{}'; exit 0", log.display()),
    );
    let args: DownloadArgs = serde_json::from_value(serde_json::json!({
        "title": "Range Show",
        "episode": "1-2",
        "mode": "sub",
        "download_dir": dest.path().to_string_lossy(),
    }))
    .expect("args");
    let path_env = bin.path().display().to_string();
    let mut progress: Vec<String> = Vec::new();
    download_with_tools(&state, &args, &path_env, |p| progress.push(p.line))
        .await
        .expect("the range downloads episode by episode");
    let calls = std::fs::read_to_string(&log).expect("the tool ran");
    let lines: Vec<&str> = calls.lines().collect();
    assert_eq!(lines.len(), 2, "one tool run per episode: {calls}");
    assert!(
        progress.iter().any(|l| l.starts_with("Playing episode 1")),
        "the range loop announces each episode in the shape the dock's \
         progress parser consumes (`Playing episode N`): {progress:?}"
    );
    assert!(
        progress.iter().any(|l| l.starts_with("Playing episode 2")),
        "both iterations announce: {progress:?}"
    );
    assert!(
        lines[0].contains("Range Show Episode 1.mp4"),
        "first run targets episode 1: {}",
        lines[0]
    );
    assert!(
        lines[1].contains("Range Show Episode 2.mp4"),
        "second run targets episode 2: {}",
        lines[1]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_range_download_stops_at_the_first_failing_episode() {
    // "1-3" on a two-episode listing: episodes 1 and 2 transfer,
    // episode 3 resolves to the typed dead end and the loop stops
    // there — the script's own loop dies mid-range the same way.
    let server = stub_range_show().await;
    let td = tempfile::tempdir().expect("td");
    let state = native_test_state(&td, &server.uri());
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let log = dest.path().join("calls.log");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("echo \"$*\" >> '{}'; exit 0", log.display()),
    );
    let args: DownloadArgs = serde_json::from_value(serde_json::json!({
        "title": "Range Show",
        "episode": "1-3",
        "mode": "sub",
        "download_dir": dest.path().to_string_lossy(),
    }))
    .expect("args");
    let path_env = bin.path().display().to_string();
    let err = download_with_tools(&state, &args, &path_env, |_p| {})
        .await
        .expect_err("episode 3 does not exist");
    assert!(matches!(err, AniError::NoResults));
    let calls = std::fs::read_to_string(&log).expect("the first two episodes ran");
    assert_eq!(
        calls.lines().count(),
        2,
        "the loop stops at the dead episode: {calls}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_download_finds_the_bundled_tools_without_a_system_install() {
    // Packaged builds place yt-dlp (and on Windows ffmpeg) under
    // resources/bin — state.bundled_bin. A user with no global
    // install must not see FfmpegMissing: the bundle joins the tool
    // search ahead of PATH, exactly as the subprocess path prepended
    // it through its options and the curl resolver ranks its
    // bundled directory first.
    let server = stub_range_show().await;
    let td = tempfile::tempdir().expect("td");
    let mut state = native_test_state(&td, &server.uri());
    let bundle = tempfile::tempdir().expect("bundle");
    let dest = tempfile::tempdir().expect("dest");
    let log = dest.path().join("calls.log");
    stage_tool(
        bundle.path(),
        "yt-dlp",
        &format!("echo \"$*\" >> '{}'; exit 0", log.display()),
    );
    state.bundled_bin = Some(bundle.path().to_path_buf());
    let args: DownloadArgs = serde_json::from_value(serde_json::json!({
        "title": "Range Show",
        "episode": "1",
        "mode": "sub",
        "download_dir": dest.path().to_string_lossy(),
    }))
    .expect("args");
    download_with_tools(&state, &args, "", |_p| {})
        .await
        .expect("the bundled tool downloads");
    let calls = std::fs::read_to_string(&log).expect("the bundled tool ran");
    assert!(
        calls.contains("Range Show Episode 1.mp4"),
        "the bundled yt-dlp carried the transfer: {calls}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_mislabeled_ytdlp_success_is_discarded_and_typed() {
    // Without ffmpeg, yt-dlp can exit 0 after leaving raw MPEG-TS
    // under the requested .mp4 name, reporting it only through a
    // stderr warning. The subprocess runner detected that report,
    // deleted the mislabeled file and surfaced FfmpegMissing; the
    // direct runner must not launder the same run into a success
    // that leaves a corrupt download in the dock as completed.
    let server = stub_range_show().await;
    let td = tempfile::tempdir().expect("td");
    let state = native_test_state(&td, &server.uri());
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Range Show Episode 1.mp4");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!(
            "printf '\\107TSDATA' > '{target}'\n\
             echo 'WARNING: out: Possible MPEG-TS in MP4 container or malformed AAC timestamps. Install ffmpeg to fix this automatically' >&2\n\
             exit 0",
            target = target.display()
        ),
    );
    let args: DownloadArgs = serde_json::from_value(serde_json::json!({
        "title": "Range Show",
        "episode": "1",
        "mode": "sub",
        "download_dir": dest.path().to_string_lossy(),
    }))
    .expect("args");
    let path_env = bin.path().display().to_string();
    let err = download_with_tools(&state, &args, &path_env, |_p| {})
        .await
        .expect_err("a mislabeled file is not a success");
    assert!(
        matches!(err, AniError::FfmpegMissing),
        "the tool's own report says ffmpeg was needed and absent: {err:?}"
    );
    assert!(
        !target.exists(),
        "the mislabeled file must not survive under the .mp4 name"
    );
}

#[tokio::test]
async fn a_range_download_surfaces_the_walks_clean_miss() {
    // The walk's own verdicts pass through untranslated: an
    // all-clean no-match is the typed NoResults, and no tool runs.
    use wiremock::matchers::{method, path};
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/browse"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"<div class="grid"><p>No results.</p></div>"#),
        )
        .mount(&server)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = native_test_state(&td, &server.uri());
    let dest = tempfile::tempdir().expect("dest");
    let args: DownloadArgs = serde_json::from_value(serde_json::json!({
        "title": "Ghost Show",
        "episode": "1-2",
        "mode": "sub",
        "download_dir": dest.path().to_string_lossy(),
    }))
    .expect("args");
    let err = download_with_tools(&state, &args, "", |_p| {})
        .await
        .expect_err("nothing matches");
    assert!(matches!(err, AniError::NoResults));
}

#[cfg(unix)]
#[tokio::test]
async fn cancelling_a_download_kills_the_tools_descendants() {
    // yt-dlp spawns its own helpers (ffmpeg for merges); aborting
    // the SSE task drops the Child, and kill_on_drop takes down
    // only the direct process. The teardown must address the
    // whole tree, like the subprocess downloader's group-kill
    // guard did — otherwise Cancel leaves an orphaned transfer
    // writing the file after the UI removed the row.
    //
    // The teardown must be the REAL one: a concurrently held
    // no-op probe would swallow the kill this test exists to
    // observe.
    let _probe_scope = crate::anicli::process::TREE_KILL_PROBE_SCOPE.lock().await;
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let pidfile = dest.path().join("helper.pid");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("sleep 30 &\necho $! > '{}'\nwait", pidfile.display()),
    );
    let path_env = bin.path().display().to_string();
    let dest_dir = dest.path().to_path_buf();
    let task = tokio::spawn(async move {
        let _ = spawn_download_tool(
            "https://cdn.example/x/master.m3u8",
            &dest_dir,
            "Show Episode 1",
            None,
            &path_env,
            std::time::Duration::from_secs(60),
            &mut |_l: &str| {},
        )
        .await;
    });
    let mut waited_ms = 0u32;
    while !pidfile.exists() {
        assert!(waited_ms < 5_000, "helper never reported its pid");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        waited_ms += 50;
    }
    task.abort();
    let _ = task.await;
    let pid = std::fs::read_to_string(&pidfile)
        .expect("pidfile")
        .trim()
        .to_string();
    // The helper must die with the cancellation, not linger for
    // its full sleep. kill -0 probes liveness portably.
    let mut waited_ms = 0u32;
    let died = loop {
        let alive = std::process::Command::new("kill")
            .arg("-0")
            .arg(&pid)
            .status()
            .expect("probe")
            .success();
        if !alive {
            break true;
        }
        if waited_ms >= 5_000 {
            break false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        waited_ms += 100;
    };
    if !died {
        let _ = std::process::Command::new("kill")
            .arg("-9")
            .arg(&pid)
            .status();
    }
    assert!(died, "the tool's descendant outlived the cancellation");
}

#[test]
fn download_args_round_trips_through_json_with_optional_fields() {
    // Wire shape mirror — same fields the renderer sends. Quality,
    // episode_count, alt_titles, kitsu_id, download_dir are all
    // optional; the modal only requires title + episode + mode.
    let body = serde_json::json!({
        "title": "Naruto Shippuuden",
        "episode": "5",
        "mode": "sub",
        "quality": "1080",
        "alt_titles": ["NARUTO -ナルト- 疾風伝"],
        "download_dir": "/tmp/dl",
    });
    let parsed: DownloadArgs = serde_json::from_value(body).expect("parse");
    assert_eq!(parsed.title, "Naruto Shippuuden");
    assert_eq!(parsed.episode, "5");
    assert_eq!(parsed.mode, "sub");
    assert_eq!(parsed.quality.as_deref(), Some("1080"));
    assert_eq!(
        parsed.alt_titles,
        vec!["NARUTO -ナルト- 疾風伝".to_string()]
    );
    assert_eq!(parsed.download_dir.as_deref(), Some("/tmp/dl"));
}

#[test]
fn download_args_alt_titles_accepts_newline_joined_string_for_sse_query_path() {
    // serde_urlencoded (the SSE GET path) can't decode repeated
    // ?alt_titles=a&alt_titles=b as a Vec, so the renderer joins
    // with \n. Same trick PlayArgs uses.
    let body = serde_json::json!({
        "title": "x",
        "episode": "1",
        "mode": "sub",
        "alt_titles": "a\nb\nc",
    });
    let parsed: DownloadArgs = serde_json::from_value(body).expect("parse");
    assert_eq!(
        parsed.alt_titles,
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
}

#[test]
fn resolve_dest_prefers_explicit_args_over_paths_helper() {
    let a = DownloadArgs {
        title: "x".into(),
        episode: "1".into(),
        mode: "sub".into(),
        quality: None,
        episode_count: None,
        year: None,
        subtype: None,
        alt_titles: vec![],
        kitsu_id: None,
        download_dir: Some("/tmp/explicit".into()),
    };
    let p = resolve_dest(&a).expect("ok");
    assert_eq!(p, PathBuf::from("/tmp/explicit"));
}
