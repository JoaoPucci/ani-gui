use super::*;

/// A directory `find_tool` accepts as holding an installed yt-dlp.
///
/// For cases whose subject is the provider walk rather than the
/// transfer: `download_with_tools` refuses before it resolves anything
/// when no tool is installed, so a walk case with an empty search path
/// never reaches the walk. Nothing here is ever spawned — these cases
/// end before a tool would run — which is why a file is enough.
/// Portable because `is_executable` reduces to `is_file` off unix.
fn dir_with_a_findable_tool() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("bin");
    let p = dir.path().join("yt-dlp");
    std::fs::write(&p, "#!/bin/sh\nexit 0\n").expect("write stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&p).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&p, perms).expect("chmod");
    }
    dir
}

#[cfg(unix)]
/// Shell that writes `body` to whatever path the tool is handed after
/// `-o`, which is where a real downloader puts its output.
///
/// Stubs used to hardcode the target name, which was the same thing
/// while the tools wrote there directly. It is not any more: a
/// transfer writes to a scratch name and the finished file is
/// published afterwards, so a stub that writes to the target is
/// standing in for something no tool does.
fn writes_its_output(body: &str) -> String {
    format!(
        "prev=\"\"\nfor a in \"$@\"; do if [ \"$prev\" = \"-o\" ]; then          printf '{body}' > \"$a\"; fi; prev=\"$a\"; done"
    )
}

#[cfg(unix)]
fn stage_tool(dir: &std::path::Path, name: &str, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{script}\n")).expect("write stub");
    let mut perms = std::fs::metadata(&p).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&p, perms).expect("chmod");
}

#[cfg(unix)]
#[test]
fn find_tool_scans_past_a_non_executable_file() {
    // An extracted binary that never got chmod +x is a regular file
    // with the right name. Stopping the search there hides a usable
    // yt-dlp in a later directory and the spawn dies with Network —
    // an install with no ffmpeg then fails despite shipping a
    // working downloader.
    use std::os::unix::fs::PermissionsExt;
    let dud = tempfile::tempdir().expect("dud");
    let good = tempfile::tempdir().expect("good");
    std::fs::write(dud.path().join("yt-dlp"), b"not executable").expect("stage");
    let real = good.path().join("yt-dlp");
    std::fs::write(&real, b"#!/bin/sh\nexit 0\n").expect("stage");
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let path_env = format!("{}:{}", dud.path().display(), good.path().display());
    assert_eq!(
        find_tool(&path_env, "yt-dlp", &[""]),
        Some(real),
        "the scan continues past an unusable entry"
    );
}

#[cfg(unix)]
#[test]
fn find_tool_widens_names_with_the_platform_suffix_table() {
    // Windows installs name the tools yt-dlp.exe / ffmpeg.exe.
    // The scan builds and tests each full path itself — Windows
    // command resolution never runs, so PATHEXT cannot widen the
    // name for us. The suffix table is explicit, like the curl
    // transport's resolver.
    let bin = tempfile::tempdir().expect("bin");
    stage_tool(bin.path(), "yt-dlp.exe", "exit 0");
    let path_env = bin.path().display().to_string();
    assert_eq!(
        find_tool(&path_env, "yt-dlp", &["", ".exe"]),
        Some(bin.path().join("yt-dlp.exe"))
    );
    // The bare name still wins where both exist.
    stage_tool(bin.path(), "ffmpeg.exe", "exit 0");
    stage_tool(bin.path(), "ffmpeg", "exit 0");
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
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!(
            "echo \"ytdlp $*\" >&2\n{}\nexit 0",
            writes_its_output("video")
        ),
    );
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
        dest.path().join("Show Episode 2.mp4").exists(),
        "the published file carries the name the CLI would have used"
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
        anidb_base: Some(anidb_base.to_string()),
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: td.path().join("history"),
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
    // values; the script's own `-e 1-12` looped over episodes. The
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
        &format!(
            "echo \"$*\" >> '{}'\n{}\nexit 0",
            log.display(),
            writes_its_output("video")
        ),
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
        dest.path().join("Range Show Episode 1.mp4").exists(),
        "first run publishes episode 1"
    );
    assert!(
        dest.path().join("Range Show Episode 2.mp4").exists(),
        "second run publishes episode 2"
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
        &format!(
            "echo \"$*\" >> '{}'\n{}\nexit 0",
            log.display(),
            writes_its_output("video")
        ),
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
        calls.contains("master.m3u8"),
        "the bundled yt-dlp carried the transfer: {calls}"
    );
    assert!(
        dest.path().join("Range Show Episode 1.mp4").exists(),
        "and its output was published under the episode's name"
    );
}

#[tokio::test(start_paused = true)]
async fn a_range_downloads_pick_stops_at_the_resolution_deadline() {
    // The range path walks aliases and candidate listings itself
    // rather than going through resolve_native_bounded, so a slow
    // provider can leave it resolving for minutes before the first
    // transfer starts — past the gate's half-open trial lifetime.
    // The pick carries the same outer deadline the play path does;
    // the per-episode transfer deadlines are separate and stay.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(19))
                .set_body_string(r#"<div class="grid"><p>No results.</p></div>"#),
        )
        .mount(&server)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = native_test_state(&td, &server.uri());
    let dest = tempfile::tempdir().expect("dest");
    let args: DownloadArgs = serde_json::from_value(serde_json::json!({
        "title": "Slow Show",
        "episode": "1-4",
        "mode": "sub",
        "alt_titles": "a\nb\nc\nd\ne\nf",
        "download_dir": dest.path().to_string_lossy(),
    }))
    .expect("args");
    let bin = dir_with_a_findable_tool();
    let got = download_with_tools(&state, &args, &bin.path().display().to_string(), |_p| {}).await;
    assert!(
        matches!(got, Err(AniError::Timeout)),
        "a stalled pick ends as a timeout: {got:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_ffmpeg_fallback_shares_the_transfer_deadline() {
    // The one-hour ceiling is the transfer's, not each tool's. A
    // yt-dlp run that burns most of it and then fails must leave the
    // ffmpeg fallback only what remains — otherwise a download can
    // stay active for nearly two hours under a deadline documented
    // as one.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(bin.path(), "yt-dlp", "sleep 0.6; exit 1");
    stage_tool(bin.path(), "ffmpeg", "sleep 0.6; exit 0");
    let started = std::time::Instant::now();
    let got = spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_millis(900),
        &mut |_l: &str| {},
    )
    .await;
    assert!(
        matches!(got, Err(AniError::Timeout)),
        "the fallback runs against what is left of the deadline: {got:?}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_millis(1_600),
        "and the whole chain ends at the deadline, not at one per tool"
    );
}

// Linux only, deliberately: ETXTBSY on exec of a file another
// process holds open for writing is a Linux guarantee. macOS does not
// report it — the spawn there simply succeeds — so the retry these
// two tests drive is unreachable on that platform and asserting it
// would be asserting the harness, not the code. The retry itself is
// platform-independent; only this way of provoking it is not.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_permanently_busy_executable_gives_up_at_the_deadline() {
    // The busy retry sits before the child exists, so the transfer
    // timeout never covers it: a tool held open for writing forever
    // spins the loop forever and the SSE request never reaches its
    // promised ceiling. The spawn loop runs under the same deadline.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(bin.path(), "yt-dlp", "exit 0");
    let _held = std::fs::OpenOptions::new()
        .write(true)
        .open(bin.path().join("yt-dlp"))
        .expect("hold the tool open for writing");
    let got = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        spawn_download_tool(
            "https://cdn.example/x/master.m3u8",
            dest.path(),
            "Show Episode 1",
            None,
            &bin.path().display().to_string(),
            std::time::Duration::from_millis(400),
            &mut |_l: &str| {},
        ),
    )
    .await
    .expect("the spawn loop must not outlive its deadline");
    assert!(
        matches!(got, Err(AniError::Timeout)),
        "a tool that never becomes runnable ends as a timeout: {got:?}"
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_busy_executable_is_retried_rather_than_failed() {
    // exec of a file another process still holds open for WRITING
    // fails with ETXTBSY. It is transient by construction — the
    // writer closes microseconds later (an auto-update re-staging a
    // binary, the suite staging stubs) — so the spawn retries
    // instead of reporting the transient network verdict over a tool
    // that is about to be perfectly usable.
    //
    // A held write handle reproduces it exactly: the spawn cannot
    // exec while it lives, and must succeed once it is dropped.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let marker = dest.path().join("ran");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("touch '{}'; exit 0", marker.display()),
    );
    let held = std::fs::OpenOptions::new()
        .write(true)
        .open(bin.path().join("yt-dlp"))
        .expect("hold the tool open for writing");
    let releaser = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        drop(held);
    });
    let got = spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(20),
        &mut |_l: &str| {},
    )
    .await;
    releaser.await.expect("releaser");
    assert!(
        got.is_ok(),
        "a busy executable must be waited out, not reported as a failure: {got:?}"
    );
    assert!(marker.exists(), "the retried spawn actually ran the tool");
}

#[cfg(unix)]
#[tokio::test]
async fn the_download_tool_runs_in_a_normalized_environment() {
    // §5's subprocess contract: nothing the child prints may depend
    // on the terminal that launched the backend. yt-dlp and ffmpeg
    // both colorize when the inherited environment says to, and
    // every stderr line goes straight to the dock — whose
    // DownloadProgress promises ANSI-stripped text. The spawn sets
    // TERM=dumb and NO_COLOR=1, and the relay strips escapes anyway.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(
        bin.path(),
        "yt-dlp",
        // Report the inherited environment, then emit a colored line.
        "echo \"TERM=$TERM NO_COLOR=$NO_COLOR\" >&2; printf '\\033[1;31mred progress\\033[0m\\n' >&2; exit 0",
    );
    let mut lines: Vec<String> = Vec::new();
    // The parent's own environment is left alone — mutating it would
    // race every other test in the process. The assertion holds
    // whatever it says, because the spawn sets these explicitly.
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |l: &str| lines.push(l.to_string()),
    )
    .await
    .expect("the stub succeeds");
    assert!(
        lines.iter().any(|l| l.contains("TERM=dumb")),
        "the child must see TERM=dumb: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("NO_COLOR=1")),
        "the child must see NO_COLOR=1: {lines:?}"
    );
    assert!(
        lines.iter().all(|l| !l.contains('\u{1b}')),
        "no escape sequence may reach the dock: {lines:?}"
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
            "{writer}\n\
             echo 'WARNING: out: Possible MPEG-TS in MP4 container or malformed AAC timestamps. Install ffmpeg to fix this automatically' >&2\n\
             exit 0",
            writer = writes_its_output("\\107TSDATA")
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
    let bin = dir_with_a_findable_tool();
    let err = download_with_tools(&state, &args, &bin.path().display().to_string(), |_p| {})
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
    let _probe_scope = crate::spawn::TREE_KILL_PROBE_SCOPE.lock().await;
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

#[tokio::test]
async fn a_missing_tool_refuses_before_the_provider_is_touched() {
    // With neither yt-dlp nor ffmpeg installed the resolution cannot
    // be used for anything, so spending it is spending the user's
    // time and the provider's patience to arrive at an answer that
    // was knowable at the click. The install modal should be the
    // first thing that happens, not the thing that happens after a
    // walk — or instead of the walk's own failure, which is worse
    // still: a network error then hides the real cause.
    // A bare server with nothing mounted: the assertion is that it was
    // never asked, so a fixture that answers would only add a way for
    // the case to be about something else. wiremock records unmatched
    // requests too, which is what makes the empty log mean anything.
    let server = wiremock::MockServer::start().await;
    let td = tempfile::tempdir().expect("td");
    let state = native_test_state(&td, &server.uri());
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
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
        .expect_err("no tool means no download");
    assert!(
        matches!(err, AniError::FfmpegMissing),
        "the absent tool is what the user is told about: {err:?}"
    );
    let hits = server
        .received_requests()
        .await
        .expect("the mock server records requests");
    assert!(
        hits.is_empty(),
        "the provider was walked before the tool check: {} request(s)",
        hits.len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_repackage_failure_retries_through_an_installed_ffmpeg() {
    // yt-dlp's warning names the condition — MPEG-TS left under the
    // .mp4 name — and suggests ffmpeg as the fix. It is not a report
    // that ffmpeg is absent, and the run condemns itself on the
    // warning alone, so an install with a working ffmpeg reached the
    // install modal and was told to install what it already had.
    //
    // The mislabeled file still goes: whatever ffmpeg writes must not
    // land on top of half a transfer in the wrong container.
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
    stage_tool(
        bin.path(),
        "ffmpeg",
        &format!(
            "printf 'GOODMP4' > '{target}'\nexit 0",
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
    let got = download_with_tools(&state, &args, &path_env, |_p| {}).await;
    assert!(
        got.is_ok(),
        "an installed ffmpeg must get the retry rather than the install modal: {got:?}"
    );
    assert_eq!(
        std::fs::read(&target).expect("ffmpeg wrote the file"),
        b"GOODMP4",
        "the retry's output replaces the mislabeled file"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_ffmpeg_retry_never_meets_what_ytdlp_left() {
    // The warning names two conditions and only one leaves MPEG-TS, so
    // the malformed-AAC half used to leave a real MP4 sitting under the
    // target name — and ffmpeg, spawned without `-y`, then refused to
    // write it. The retry failed on exactly the machines it exists for.
    //
    // Neither half can reach the target now. yt-dlp writes to a name
    // this run invented, that name is dropped when its own report
    // condemns it, and the retry gets a fresh one. The case pins that:
    // the two tools are handed different paths, and neither is the
    // user's.
    let server = stub_range_show().await;
    let td = tempfile::tempdir().expect("td");
    let state = native_test_state(&td, &server.uri());
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Range Show Episode 1.mp4");
    let seen = dest.path().join("outputs");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!(
            // A real MP4 header, not MPEG-TS: the shape that used to
            // survive the discard and block the retry.
            "prev=\"\"\nfor a in \"$@\"; do if [ \"$prev\" = \"-o\" ]; then \
             echo \"$a\" >> '{seen}'; printf '\\000\\000\\000\\040ftypmp42' > \"$a\"; fi; prev=\"$a\"; done\n\
             echo 'WARNING: out: Possible MPEG-TS in MP4 container or malformed AAC timestamps. Install ffmpeg to fix this automatically' >&2\n\
             exit 0",
            seen = seen.display()
        ),
    );
    stage_tool(
        bin.path(),
        "ffmpeg",
        &format!(
            "prev=\"\"\nfor a in \"$@\"; do if [ \"$prev\" = \"-i\" ]; then :; fi; prev=\"$a\"; done\n\
             last=\"\"\nfor a in \"$@\"; do last=\"$a\"; done\n\
             echo \"$last\" >> '{seen}'\nprintf 'GOODMP4' > \"$last\"\nexit 0",
            seen = seen.display()
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
    download_with_tools(&state, &args, &path_env, |_p| {})
        .await
        .expect("the retry runs and publishes");

    let outputs: Vec<String> = std::fs::read_to_string(&seen)
        .expect("both tools recorded their output path")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(outputs.len(), 2, "both tools ran: {outputs:?}");
    assert_ne!(
        outputs[0], outputs[1],
        "the retry must start from a name yt-dlp never touched: {outputs:?}"
    );
    assert!(
        !outputs.iter().any(|o| o == &target.to_string_lossy()),
        "and neither tool is handed the user's own file: {outputs:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("GOODMP4"),
        "what ffmpeg produced is what gets published"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn ffmpeg_also_writes_somewhere_other_than_the_target() {
    // The ffmpeg leg of the scratch rule. Its sibling above covers
    // yt-dlp, and the two reach the tool by different branches — the
    // one that only runs when yt-dlp is absent had no coverage of
    // where it writes.
    //
    // This case used to stage a file at the target and asserted that
    // ffmpeg was not handed it. That staging no longer runs a tool at
    // all: a download into a folder that already has the episode now
    // returns before spawning anything, so the assertion was reading
    // an argv file the stub never wrote. What it was protecting — an
    // earlier download surviving — is asserted directly by
    // `a_file_that_predates_the_download_is_kept` and by
    // `an_episode_already_in_the_folder_is_not_downloaded_again`. What
    // is left is the property those two do not reach, staged so the
    // tool actually runs.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let argv = dest.path().join("argv");
    stage_tool(
        bin.path(),
        "ffmpeg",
        &format!(
            // ffmpeg takes its output as the last positional argument
            // rather than after `-o`, so the yt-dlp writer would fire
            // for neither the right path nor at all.
            "printf '%s\\n' \"$@\" > '{argv}'\n             for a in \"$@\"; do last=\"$a\"; done\n             printf 'video' > \"$last\"\nexit 0",
            argv = argv.display()
        ),
    );
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Ffmpeg Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |_l: &str| {},
    )
    .await
    .expect("the download succeeds");

    let target = dest.path().join("Ffmpeg Show Episode 1.mp4");
    let seen = std::fs::read_to_string(&argv).expect("the stub recorded its argv");
    assert!(
        !seen.lines().any(|a| a == target.to_string_lossy()),
        "ffmpeg must be handed a scratch path, not the target: {seen}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("video"),
        "and what it wrote has to end up at the target"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_repackage_retry_may_replace_the_file_it_condemned() {
    // The other half. Here yt-dlp wrote the target during this run and
    // reported it unusable, so the retry owns that path — and the
    // removal ahead of it is best-effort, which is why the flag has to
    // carry the case where the file could not be unlinked.
    let server = stub_range_show().await;
    let td = tempfile::tempdir().expect("td");
    let state = native_test_state(&td, &server.uri());
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Range Show Episode 1.mp4");
    let argv = dest.path().join("argv");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!(
            "printf '\\000\\000\\000\\040ftypmp42' > '{target}'\n\
             echo 'WARNING: out: Possible MPEG-TS in MP4 container or malformed AAC timestamps. Install ffmpeg to fix this automatically' >&2\n\
             exit 0",
            target = target.display()
        ),
    );
    stage_tool(
        bin.path(),
        "ffmpeg",
        &format!(
            "printf '%s\\n' \"$@\" > '{argv}'\nprintf 'OK' > '{target}'\nexit 0",
            argv = argv.display(),
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
    download_with_tools(&state, &args, &bin.path().display().to_string(), |_p| {})
        .await
        .expect("the retry runs");
    let seen = std::fs::read_to_string(&argv).expect("stub recorded its argv");
    assert!(
        seen.lines().any(|a| a == "-y"),
        "the retry owns the target and must be allowed to replace it: {seen:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn two_downloads_of_one_target_do_not_overlap() {
    // The dock permits a second download of an episode already
    // downloading — `add` mints a fresh row per click without
    // consulting the ones it has — and both resolve to the same
    // `<title> Episode <n>.mp4`. Two tools then write one path.
    //
    // That was always a lost write. The repackage retry made it worse:
    // it removes the target and passes `-y`, so the invocation that
    // arrives second can delete and overwrite a file the first one has
    // already finished, and the dock reports both as complete.
    //
    // The stub brackets its run in a shared log. Serialized, the log
    // reads start/end/start/end; overlapping, start/start/end/end.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let log = dest.path().join("order");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!(
            "echo start >> '{log}'\nsleep 0.3\necho end >> '{log}'\nexit 0",
            log = log.display()
        ),
    );
    let path_env = bin.path().display().to_string();
    let mut sink_one = |_l: &str| {};
    let mut sink_two = |_l: &str| {};
    let one = spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Same Show Episode 1",
        None,
        &path_env,
        std::time::Duration::from_secs(10),
        &mut sink_one,
    );
    let two = spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Same Show Episode 1",
        None,
        &path_env,
        std::time::Duration::from_secs(10),
        &mut sink_two,
    );
    let (a, b) = tokio::join!(one, two);
    a.expect("first run");
    b.expect("second run");
    let order = std::fs::read_to_string(&log).expect("the stub logged");
    assert_eq!(
        order.split_whitespace().collect::<Vec<_>>(),
        vec!["start", "end", "start", "end"],
        "two writers of one target must take turns, not overlap: {order:?}"
    );
}

#[cfg(unix)]
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waiting_for_a_same_process_download_is_charged_to_the_deadline() {
    // The two waits are the same wait as far as the user is concerned:
    // a queued download sitting behind one that already owns the file.
    // Only the cross-instance one was charged to the transfer's
    // ceiling, because the deadline was created between them — so a
    // duplicate click waited out the first download's whole hour and
    // then started its own hour, while the same wait against another
    // app instance ended at the deadline.
    //
    // The duplicate here is a second call for the same target with the
    // first still running. The first stub outlives the second's
    // deadline; the second must give up at its own, not inherit a
    // fresh one when the mutex finally hands over.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let ran = dest.path().join("ran");
    stage_tool(
        bin.path(),
        "yt-dlp",
        // Records the run before sleeping, so a spawn that is later
        // killed at its deadline still leaves evidence it happened.
        &format!(
            "echo ran >> '{ran}'\nsleep 1.5\nexit 0",
            ran = ran.display()
        ),
    );
    let path_env = bin.path().display().to_string();
    let dir = dest.path().to_path_buf();
    let held = {
        let path_env = path_env.clone();
        let dir = dir.clone();
        tokio::spawn(async move {
            let mut sink = |_l: &str| {};
            spawn_download_tool(
                "https://cdn.example/x/master.m3u8",
                &dir,
                "Queued Show Episode 1",
                None,
                &path_env,
                std::time::Duration::from_secs(10),
                &mut sink,
            )
            .await
        })
    };
    // Let the first call reach the tool before the duplicate arrives.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let mut sink = |_l: &str| {};
    let queued = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        spawn_download_tool(
            "https://cdn.example/x/master.m3u8",
            &dir,
            "Queued Show Episode 1",
            None,
            &path_env,
            std::time::Duration::from_millis(300),
            &mut sink,
        ),
    )
    .await;
    let queued = queued.expect("the queued download must end at its own deadline");
    assert!(
        matches!(queued, Err(AniError::Timeout)),
        "a download that spends its deadline waiting for a same-process holder \
         has timed out, exactly as it would waiting for another instance: {queued:?}"
    );
    held.await.expect("join").expect("the first download runs");
    // The discriminating assertion. Timeout alone proves nothing here:
    // a queued call handed a fresh deadline still spawns its tool and
    // still times out on the transfer. What separates the two is
    // whether it spawned at all.
    let runs = std::fs::read_to_string(&ran).expect("the stub logged");
    assert_eq!(
        runs.lines().count(),
        1,
        "the queued download spent its deadline waiting, so it must never have \
         reached the tool: {runs:?}"
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn waiting_for_another_instance_is_bounded_by_the_transfer_deadline() {
    // `flock` parks the thread that calls it, and a parked thread is
    // not a future: cancelling the download drops the JoinHandle while
    // the closure stays inside the call. The dock's Cancel then looks
    // like it worked and the blocking pool is one thread down, so
    // repeated start-and-cancel walks the pool towards starving every
    // other blocking caller — and none of it is visible.
    //
    // The wait therefore belongs on the async side, where it has to be
    // bounded by something. The transfer deadline is the obvious
    // bound: a download blocked on another instance is spending the
    // same wait the transfer itself was given.
    //
    // The case wraps its own timeout around the call so an unbounded
    // wait fails the assertion instead of hanging the suite with
    // nothing to read.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let ran = dest.path().join("ran");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("echo ran >> '{ran}'\nexit 0", ran = ran.display()),
    );
    let target = dest.path().join("Contended Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path for the target");
    std::fs::create_dir_all(lock_path.parent().expect("lock dir")).expect("stage the lock dir");
    let foreign = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .expect("lock file");
    fs4::FileExt::lock(&foreign).expect("foreign lock");

    let mut sink = |_l: &str| {};
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        spawn_download_tool(
            "https://cdn.example/x/master.m3u8",
            dest.path(),
            "Contended Show Episode 1",
            None,
            &bin.path().display().to_string(),
            std::time::Duration::from_millis(300),
            &mut sink,
        ),
    )
    .await;
    fs4::FileExt::unlock(&foreign).expect("unlock");

    let outcome = outcome.expect(
        "waiting for another instance must end at the transfer deadline, not park a thread \
         until the holder releases",
    );
    assert!(
        matches!(outcome, Err(AniError::Timeout)),
        "a wait that runs out of deadline is a timeout: {outcome:?}"
    );
    assert!(
        !ran.exists(),
        "the tool ran without the cross-instance lock being held"
    );
}

#[test]
fn the_cross_instance_lock_lives_beside_the_file_it_guards() {
    // A lock is only worth holding if every contender takes the same
    // one, so its location may not depend on anything a process can be
    // configured with. Two attempts at this failed that test: the app
    // cache directory moves with the build profile, and the profile
    // fix still left it under `$XDG_CACHE_HOME`, which is an
    // environment variable — an installed release and a source build
    // launched with different cache roots write the same download
    // folder and take different locks.
    //
    // The one location that cannot diverge is the one derived from
    // what they are contending for. Both instances were handed the
    // same destination directory, or they would not be racing at all.
    let dest = std::path::Path::new("/tmp/ani-gui-lock-probe");
    let target = dest.join("Show Episode 1.mp4");
    let path = target_lock_path(&target).expect("a lock path for the target");
    assert!(
        path.starts_with(dest),
        "the lock must sit under the target's own directory, or no \
         environment-dependent root can be trusted to match: {}",
        path.display()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_episode_already_in_the_folder_is_not_downloaded_again() {
    // Publication keeps whatever is already at the name, so a transfer
    // into a folder that has the episode cannot change the outcome. It
    // can still spend an episode's worth of the user's bandwidth and
    // most of the hour it is allowed, and then throw the result away.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let ran = dest.path().join("ran");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!(
            "echo ran >> '{ran}'\n{writer}\nexit 0",
            ran = ran.display(),
            writer = writes_its_output("replacement")
        ),
    );
    let target = dest.path().join("Owned Show Episode 1.mp4");
    std::fs::write(&target, b"already here").expect("stage the episode");

    let mut lines = Vec::new();
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Owned Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |l: &str| lines.push(l.to_string()),
    )
    .await
    .expect("having the episode already is not a failure");

    assert!(
        !ran.exists(),
        "no tool should run for a file that cannot be replaced"
    );
    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("already here"),
        "and the episode there is untouched"
    );
    assert!(
        lines.iter().any(|l| l.contains("already")),
        "the dock is told why nothing happened: {lines:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_publish_that_fails_still_takes_the_scratch_with_it() {
    // The guard was disarmed before publication rather than after, so
    // a publish that could not install the file left the finished
    // transfer sitting in the download folder — a whole episode, under
    // a hidden name, with nothing that would ever remove it.
    //
    // A target name past what the filesystem accepts is the cheap way
    // to make publication fail: the scratch name is short and gets
    // written normally, and only the install is impossible.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("{}\nexit 0", writes_its_output("a whole episode")),
    );
    let stem = "n".repeat(300);
    let got = spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        &stem,
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |_l: &str| {},
    )
    .await;
    assert!(got.is_err(), "a file that cannot be installed is a failure");

    let leftovers: Vec<String> = std::fs::read_dir(dest.path())
        .expect("dest")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("part"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "a transfer that could not be published must not be left behind: {leftovers:?}"
    );
}

#[test]
fn a_failed_link_free_publish_leaves_no_claim_behind() {
    // The no-hard-links path claims the name by creating it and then
    // renames onto its own empty file. If that rename does not happen,
    // the claim IS the episode as far as everything downstream can
    // tell: zero bytes at the target, which the next download reads as
    // somebody else's finished file and stands down from. One failure
    // poisons that name for good.
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Show Episode 1.mp4");
    let missing = dest.path().join("never-written.part.mp4");
    let got = publish_without_links(&missing, &target);
    assert!(
        got.is_err(),
        "a publish with nothing to install is a failure"
    );
    assert!(
        !target.exists(),
        "and it must not leave its own claim standing as an episode"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_download_publishes_even_when_the_lock_cannot_be_taken() {
    // The lock stopped being the thing that makes this safe when
    // publishing took over. It is an optimization — it saves a second
    // click the bandwidth — so failing to take one is a reason to skip
    // the saving, not to refuse a download the filesystem would have
    // accepted. A destination deep enough that the target fits and the
    // lock path does not is the case that used to be refused outright.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("{}\nexit 0", writes_its_output("video")),
    );
    let target = dest.path().join("Unlockable Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");
    // A directory where the lock file belongs: it cannot be opened for
    // writing, which is what an unusable lock location looks like.
    std::fs::create_dir_all(&lock_path).expect("stage the obstruction");

    let mut sink = |_l: &str| {};
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Unlockable Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut sink,
    )
    .await
    .expect("an unusable lock must not refuse the download");
    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("video"),
        "the episode is published without the lock's help"
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_takes_the_tools_own_temporaries_with_it() {
    // The transfer does not write to the `-o` path while it runs.
    // yt-dlp's `--part` is on by default — "use .part files instead of
    // writing directly into output file" — so the bytes land in
    // `<out>.part`, with `<out>.ytdl` holding fragment-resume state
    // and a `<out>.part-FragN` per in-flight fragment beside it. A
    // guard that removes the exact `-o` path removes the one name
    // nothing was ever written to, and the episode-sized file stays.
    //
    // The fix is not to enumerate a tool's naming conventions. That is
    // the same mistake as reconstructing which spellings are one file,
    // and it ends the same way: right until the tool adds a suffix. The
    // scratch name carries a uuid this run generated, so anything
    // derived from it is ours by construction.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(
        bin.path(),
        "yt-dlp",
        "prev=\"\"\nfor a in \"$@\"; do \
         if [ \"$prev\" = \"-o\" ]; then \
         printf 'half an episode' > \"$a.part\"; \
         printf 'resume state' > \"$a.ytdl\"; \
         printf 'a fragment' > \"$a.part-Frag7\"; \
         fi; prev=\"$a\"; done\nsleep 30\nexit 0",
    );
    let path_env = bin.path().display().to_string();
    let dir = dest.path().to_path_buf();
    let running = tokio::spawn(async move {
        let mut sink = |_l: &str| {};
        spawn_download_tool(
            "https://cdn.example/x/master.m3u8",
            &dir,
            "Interrupted Show Episode 1",
            None,
            &path_env,
            std::time::Duration::from_secs(60),
            &mut sink,
        )
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    running.abort();
    let _ = running.await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let leftovers = ours_in(dest.path());
    assert!(
        leftovers.is_empty(),
        "the tool's own temporaries are the transfer too: {leftovers:?}"
    );
}

/// Every file in `dir` this app's transfer could have put there. The
/// scratch name is `.ani-gui-<pid>-<uuid>…`, and nothing a tool
/// derives from it loses that prefix.
///
/// Files only, because `.ani-gui-locks` shares the prefix and is not
/// transfer data — it is the lock directory, which has to sit on the
/// destination's own volume and outlives any one download. The
/// production sweep is narrower still: it matches the full scratch
/// file name, which no other entry can begin with.
#[cfg(unix)]
fn ours_in(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .expect("dest")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".ani-gui-"))
        .collect()
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_a_download_takes_its_scratch_file_with_it() {
    // Cancel drops the future mid-transfer. None of the error branches
    // run, so nothing below the await gets to clean up — and what is
    // left is not a stray marker but most of an episode, tens or
    // hundreds of megabytes, hidden in the user's download folder.
    // Cancel a few times and it is gigabytes.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("{}\nsleep 30\nexit 0", writes_its_output("half an episode")),
    );
    let path_env = bin.path().display().to_string();
    let dir = dest.path().to_path_buf();
    let running = tokio::spawn(async move {
        let mut sink = |_l: &str| {};
        spawn_download_tool(
            "https://cdn.example/x/master.m3u8",
            &dir,
            "Cancelled Show Episode 1",
            None,
            &path_env,
            std::time::Duration::from_secs(60),
            &mut sink,
        )
        .await
    });
    // Long enough for the stub to have written something.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    running.abort();
    let _ = running.await;
    // The drop runs on the task's own thread; give it a moment to land.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let leftovers: Vec<String> = std::fs::read_dir(dest.path())
        .expect("dest")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("part"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "a cancelled transfer must not leave its partial file behind: {leftovers:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_tool_writes_somewhere_other_than_the_target() {
    // Writing straight to the final name is what made every
    // concurrency question here a question about names. While the
    // transfer is in flight the file is incomplete, and it is sitting
    // at the name the user's file manager, the dock, and any other
    // download are all looking at.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let argv = dest.path().join("argv");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!(
            "printf '%s\n' \"$@\" > '{argv}'\nprev=\"\"\nfor a in \"$@\"; do \
             if [ \"$prev\" = \"-o\" ]; then printf 'video' > \"$a\"; fi; prev=\"$a\"; done\nexit 0",
            argv = argv.display()
        ),
    );
    let mut sink = |_l: &str| {};
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Scratch Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut sink,
    )
    .await
    .expect("the download succeeds");

    let target = dest.path().join("Scratch Show Episode 1.mp4");
    let seen = std::fs::read_to_string(&argv).expect("the stub recorded its argv");
    assert!(
        !seen.lines().any(|a| a == target.to_string_lossy()),
        "the tool must be handed a scratch path, not the target: {seen}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("video"),
        "and what it wrote has to end up at the target"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_file_that_appears_mid_transfer_is_not_replaced() {
    // The race, staged exactly: the stub publishes a rival file at the
    // target while this transfer is still running, which is what
    // another download finishing first looks like from in here.
    // Whoever lands first owns the name; the one that arrives second
    // discards its own copy rather than deleting a finished file.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Contested Show Episode 1.mp4");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!(
            "printf 'winner' > '{target}'\nprev=\"\"\nfor a in \"$@\"; do \
             if [ \"$prev\" = \"-o\" ]; then printf 'loser' > \"$a\"; fi; prev=\"$a\"; done\nexit 0",
            target = target.display()
        ),
    );
    let mut sink = |_l: &str| {};
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Contested Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut sink,
    )
    .await
    .expect("losing the race is not a failure — the episode is there");
    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("winner"),
        "the file that got there first must survive"
    );
    let leftovers: Vec<_> = std::fs::read_dir(dest.path())
        .expect("dest")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("part"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "the discarded copy must not be left behind: {leftovers:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_abandoned_claim_does_not_block_the_episode_forever() {
    // Where hard links are unsupported, publication claims the name by
    // creating it empty and then renames onto its own claim. Those are
    // two calls, and a process that dies between them — a crash, a
    // power cut, a kill — leaves the empty claim standing.
    //
    // Nothing then distinguishes it from a finished download. The
    // early skip reads it as the episode and every later attempt
    // reports success without transferring, so the name is unusable
    // until the user finds a zero-byte file in their downloads and
    // deletes it by hand.
    //
    // A zero-length file is not an episode. Nothing plays it, no
    // transfer produces it, and it is exactly what this failure
    // leaves.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("{}\nexit 0", writes_its_output("the episode")),
    );
    let target = dest.path().join("Abandoned Show Episode 1.mp4");
    std::fs::write(&target, b"").expect("stage the claim a crash left");
    age_it(&target);

    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Abandoned Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |_l: &str| {},
    )
    .await
    .expect("the download runs");

    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("the episode"),
        "an empty file must not stand in for an episode nobody has"
    );
}

/// Backdate `path` far enough that the grace period has passed.
fn age_it(path: &std::path::Path) {
    let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open to backdate")
        .set_modified(old)
        .expect("backdate");
}

#[test]
fn a_claim_another_publisher_is_still_using_is_left_alone() {
    // The claim is only abandoned once nobody is coming back for it.
    // Between another publisher's `create_new` and its `rename` the
    // claim is live, and deleting it there lets both transfers install
    // — each one's rename replacing the other's file, and the loser's
    // error cleanup deleting a target it no longer owns.
    //
    // That is not two copies of one thing. The target name is
    // `<title> Episode <n>.mp4` and carries neither mode nor quality,
    // so the sub and the dub of an episode collide on it, and so do
    // 1080p and 720p. What is lost is a different file.
    //
    // Nothing here can ask who owns a path. What it can ask is how
    // long the claim has been sitting there: a live one is two
    // syscalls old, an abandoned one is as old as the crash.
    let dir = tempfile::tempdir().expect("dir");
    let scratch = dir.path().join(".ani-gui-test.part.mp4");
    let target = dir.path().join("Contended Show Episode 1.mp4");
    std::fs::write(&scratch, b"the dub").expect("a finished transfer");
    std::fs::write(&target, b"").expect("another publisher's live claim");

    assert_eq!(
        publish(&scratch, &target).expect("publication succeeds"),
        Published::AlreadyThere,
        "a claim made moments ago belongs to a publisher still running"
    );
    assert_eq!(
        std::fs::metadata(&target).expect("target").len(),
        0,
        "and its claim is still standing for it to rename onto"
    );
}

#[test]
fn a_directory_at_the_target_is_a_failure_not_a_download() {
    // `AlreadyExists` from the link is not always another episode.
    // A directory, a fifo, anything that is not a regular file gives
    // the same error, and reporting that as a completed download tells
    // the user their episode is in a folder where there is nothing
    // playable at all.
    let dir = tempfile::tempdir().expect("dir");
    let scratch = dir.path().join(".ani-gui-test.part.mp4");
    let target = dir.path().join("Blocked Show Episode 1.mp4");
    std::fs::write(&scratch, b"the episode").expect("a finished transfer");
    std::fs::create_dir(&target).expect("something else holds the name");

    assert!(
        publish(&scratch, &target).is_err(),
        "nothing playable is at the target, so this did not succeed"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_directory_at_the_target_refuses_before_the_transfer() {
    // And it refuses up front, for the same reason the already-here
    // case does: the outcome cannot change, so an episode of the
    // user's bandwidth should not be spent finding that out.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let ran = dest.path().join("ran");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("echo ran >> '{}'\nexit 0", ran.display()),
    );
    std::fs::create_dir(dest.path().join("Blocked Show Episode 1.mp4")).expect("an obstruction");

    let got = spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Blocked Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |_l: &str| {},
    )
    .await;
    assert!(got.is_err(), "the episode cannot land at that name");
    assert!(!ran.exists(), "and no tool should have run to discover it");
}

#[test]
fn publication_takes_over_an_abandoned_claim() {
    // The other half, at the boundary: reaching publication with a
    // claim already at the target must install rather than report the
    // episode present. Otherwise the download runs every time and
    // discards its result every time — the transfer is spent and the
    // name is still unusable.
    let dir = tempfile::tempdir().expect("dir");
    let scratch = dir.path().join(".ani-gui-test.part.mp4");
    let target = dir.path().join("Claimed Show Episode 1.mp4");
    std::fs::write(&scratch, b"the episode").expect("a finished transfer");
    std::fs::write(&target, b"").expect("the claim");
    age_it(&target);

    assert_eq!(
        publish(&scratch, &target).expect("publication succeeds"),
        Published::Installed
    );
    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("the episode")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_tool_that_writes_an_empty_file_installs_nothing() {
    // And the app must not create the poison itself. A tool that exits
    // cleanly having written an empty file has downloaded nothing;
    // installing that under the episode's name would put a file there
    // that no later attempt could replace under the old rule, and that
    // the user would have to delete by hand.
    //
    // The neighbouring case for a tool that writes no file at all ends
    // as a success with nothing installed. This is the same event with
    // one more syscall in it.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("{}\nexit 0", writes_its_output("")),
    );
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Empty Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut |_l: &str| {},
    )
    .await
    .expect("a tool that wrote nothing is not a failure");
    assert!(
        !dest.path().join("Empty Show Episode 1.mp4").exists(),
        "an empty transfer must not take the episode's name"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_file_that_predates_the_download_is_kept() {
    // The other half. I first wrote this asserting the opposite —
    // that re-downloading replaces what is there — and the repository
    // already said otherwise: the confirm dialog asks for a directory
    // and never for permission to overwrite, so a file at that name is
    // one the user did not agree to lose. That rule was enforced on
    // the ffmpeg path alone, by withholding `-y`, while a yt-dlp run
    // wrote straight over the same file. Publishing applies it to
    // both.
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Redownloaded Show Episode 1.mp4");
    std::fs::write(&target, b"earlier").expect("stage an earlier download");
    stage_tool(
        bin.path(),
        "yt-dlp",
        &format!("{}\nexit 0", writes_its_output("fresh")),
    );
    let mut sink = |_l: &str| {};
    spawn_download_tool(
        "https://cdn.example/x/master.m3u8",
        dest.path(),
        "Redownloaded Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(10),
        &mut sink,
    )
    .await
    .expect("finding the episode already there is not a failure");
    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("earlier"),
        "a file the user already had is not one they agreed to lose"
    );
}
