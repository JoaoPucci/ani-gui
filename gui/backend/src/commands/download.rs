//! Download an episode via `ani-cli -d`. Mirrors the play command's
//! shape (same disambiguation, same Kitsu-driven select-index logic),
//! but instead of registering a stream session it spawns yt-dlp /
//! ffmpeg / aria2c via ani-cli to write an mp4 to disk.
//!
//! Progress lines (aria2c / yt-dlp / ffmpeg stderr) are forwarded to
//! the SSE handler in `api::get_download_stream` so the renderer can
//! show a live progress bar in the dock.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::commands::play::anidb_client_with_base;
use crate::commands::play_native_resolve::{resolve_native_bounded, NativeResolveRequest};
use crate::error::{AniError, Result};

/// Wire payload for the download endpoint. A near-clone of [`PlayArgs`]
/// (so the renderer can pass through the same metadata it gathered for
/// /api/play), with one extra field: an explicit destination directory
/// from the folder picker. When `None`, the resolver falls back to
/// `paths::download_dir()`.
#[derive(Debug, Clone, Deserialize)]
pub struct DownloadArgs {
    /// Canonical Kitsu title (drives the ani-cli search step).
    pub title: String,
    /// Episode number, as a string (matches the CLI's `-e <n>` shape).
    pub episode: String,
    /// `"sub"` or `"dub"`.
    pub mode: String,
    /// `"best"` / `"worst"` / `"1080"` / etc. Defaults to `"best"`.
    #[serde(default)]
    pub quality: Option<String>,
    /// Kitsu's authoritative episode count — feeds the same
    /// disambiguator the play path uses.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// Year the show first aired (Kitsu `start_date` year). Plumbed
    /// to the picker as the year tie-break; see [`PlayArgs::year`].
    #[serde(default)]
    pub year: Option<u32>,
    /// Kitsu's subtype (`TV`, `movie`, `special`, `OVA`, `ONA`) —
    /// the same format-disproof signal [`PlayArgs::subtype`] carries
    /// on the play path.
    #[serde(default)]
    pub subtype: Option<String>,
    /// Fallback titles tried when the canonical title returns no
    /// allanime hits. Same wire forms as [`PlayArgs::alt_titles`].
    #[serde(default, deserialize_with = "deserialize_alt_titles")]
    pub alt_titles: Vec<String>,
    /// Kitsu id of the show being downloaded; logged for traceability.
    #[serde(default)]
    pub kitsu_id: Option<String>,
    /// Absolute path to the directory the download lands in. The
    /// frontend's confirmation modal opens on `paths::download_dir()`
    /// and lets the user pick a different folder; the chosen path
    /// arrives here. `None` triggers the same default-resolution
    /// chain on the backend.
    #[serde(default)]
    pub download_dir: Option<String>,
}

/// SSE event body for each progress line forwarded from the downloader
/// (aria2c / yt-dlp / ffmpeg). Frontend renders the latest line under
/// each active download row.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    /// Raw stderr line from aria2c / yt-dlp / ffmpeg, ANSI-stripped.
    pub line: String,
}

/// SSE final-event body. `dest_dir` is the directory the file landed
/// in (so the renderer can fire a "reveal in folder" intent without
/// guessing the exact filename — ani-cli's name templating depends on
/// the upstream's `allanime_title`, which we don't surface here).
#[derive(Debug, Clone, Serialize)]
pub struct DownloadResponse {
    /// Directory the file was written to. Renderer feeds this to
    /// `revealInFolder` for the completion toast.
    pub dest_dir: String,
}

/// Drive a download from `args`. Picks the same (title, candidate
/// index) pair as the equivalent /api/play call so what was watched is
/// what gets saved, then spawns ani-cli with `-d` + the chosen
/// destination directory. `on_progress` is invoked for every stderr
/// line ani-cli forwards (aria2c progress, yt-dlp fragment events,
/// etc.).
///
/// # Errors
/// - [`AniError::Config`] when no destination is supplied and the
///   default resolver returns `None` (no `$XDG_DOWNLOAD_DIR`, no
///   `$HOME` — the renderer should always pass an explicit dir).
/// - [`AniError::Io`] if the destination directory can't be created.
/// - Otherwise propagates from [`spawn_download`].
pub async fn download_with_progress<F>(
    state: &AppState,
    args: &DownloadArgs,
    on_progress: F,
) -> Result<DownloadResponse>
where
    F: FnMut(DownloadProgress) + Send,
{
    let path_env = std::env::var("PATH").unwrap_or_default();
    download_with_tools(state, args, &path_env, on_progress).await
}

/// [`download_with_progress`] with the tool-search PATH explicit —
/// the seam the stub-tool tests drive.
pub(crate) async fn download_with_tools<F>(
    state: &AppState,
    args: &DownloadArgs,
    path_env: &str,
    mut on_progress: F,
) -> Result<DownloadResponse>
where
    F: FnMut(DownloadProgress) + Send,
{
    let dest = resolve_dest(args)?;
    std::fs::create_dir_all(&dest).map_err(|_| AniError::Io)?;

    // A "1-12"-shaped episode is the Download All/Range UI: the
    // script's own -e loop, one pick then one transfer per episode.
    if let Some((first, last)) = super::download_range::episode_range(&args.episode) {
        let quality = args.quality.as_deref().unwrap_or("best");
        super::download_range::download_range(
            state,
            args,
            first,
            last,
            quality,
            &dest,
            path_env,
            &mut on_progress,
        )
        .await?;
        return Ok(DownloadResponse {
            dest_dir: dest.to_string_lossy().into_owned(),
        });
    }

    // Resolve the stream natively — the same walk, disambiguation
    // and episode mapping as the play path — then hand the master URL
    // to the download tool directly, exactly as 5.0's own download()
    // would. The one-hour transfer deadline stays: yt-dlp / ffmpeg
    // keep stderr quiet mid-transfer, so no shorter timeout can be
    // informed by progress.
    let quality = args.quality.as_deref().unwrap_or("best");
    // Downloads are always a user waiting at the dock — interactive
    // priority, like the play path's non-prefetch requests.
    let prio = crate::scraper::gate::ScrapePriority::Interactive;
    let client = anidb_client_with_base(state, state.anidb_base.as_deref(), prio)?;
    let request = NativeResolveRequest {
        title: &args.title,
        alt_titles: &args.alt_titles,
        episode: &args.episode,
        mode: &args.mode,
        quality,
        expected_count: args.episode_count,
        year: args.year,
        subtype: args.subtype.as_deref(),
    };
    let resolve_started_at = tokio::time::Instant::now();
    let mut forward = |p: crate::anicli::parser::ProgressLine| {
        let text = match p {
            crate::anicli::parser::ProgressLine::Banner { text }
            | crate::anicli::parser::ProgressLine::Other { text } => text,
            crate::anicli::parser::ProgressLine::LinksFetched { provider } => {
                format!("{provider} links fetched")
            }
            crate::anicli::parser::ProgressLine::Searching { provider } => {
                format!("Searching {provider}...")
            }
            crate::anicli::parser::ProgressLine::Matched { title } => {
                format!("Matched {title}")
            }
        };
        on_progress(DownloadProgress { line: text });
    };
    // Bounded like the play path: a provider that accepts connections
    // but stalls must not pin the resolve past the gate's half-open
    // trial window. The hour-long deadline below covers the transfer
    // alone.
    let resolved = resolve_native_bounded(&client, request, &mut forward).await;
    // Resolution and transfer are separate stages now, so the breaker
    // learns from the fresh resolution outcome alone — the stale
    // whole-run signal the subprocess forced is gone. Same mapping
    // as play: answered verdicts are health, weather is distress, a
    // gate refusal records nothing.
    if let Some(outcome) = crate::commands::play_native_outcome::breaker_outcome(prio, &resolved) {
        let observed_at = resolved
            .as_ref()
            .err()
            .and_then(|ne| ne.failed_at)
            .or_else(|| client.transport().last_attempt_at())
            .unwrap_or(resolve_started_at);
        state.anidb_gate.record(outcome, observed_at);
    }
    let resolved = resolved.map_err(|ne| ne.error)?;

    tracing::info!(
        slug = %resolved.slug,
        episode = %args.episode,
        mode = %args.mode,
        dest = %dest.display(),
        "download: spawning tool on natively resolved stream",
    );
    let file_stem = format!("{} Episode {}", resolved.title, args.episode);
    spawn_download_tool(
        &resolved.master_url,
        &dest,
        &file_stem,
        Some(quality),
        path_env,
        std::time::Duration::from_secs(60 * 60),
        &mut |line| {
            tracing::info!(line = %line, "download.tool.stderr");
            on_progress(DownloadProgress {
                line: line.to_string(),
            });
        },
    )
    .await?;

    Ok(DownloadResponse {
        dest_dir: dest.to_string_lossy().into_owned(),
    })
}

/// Resolve the destination directory from args + paths::download_dir.
/// Errors when neither path is available (no XDG, no HOME, no
/// override) — that case is unreachable from the GUI, which always
/// passes a value from the modal.
fn resolve_dest(args: &DownloadArgs) -> Result<PathBuf> {
    if let Some(s) = args.download_dir.as_deref().filter(|s| !s.is_empty()) {
        return Ok(PathBuf::from(s));
    }
    crate::config::paths::download_dir().ok_or(AniError::Config)
}

fn deserialize_alt_titles<'de, D>(d: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        List(Vec<String>),
        Joined(String),
    }
    Option::<Wire>::deserialize(d).map(|opt| match opt {
        None => Vec::new(),
        Some(Wire::List(v)) => v,
        Some(Wire::Joined(s)) => s
            .split('\n')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect(),
    })
}

/// Spawn the download tool directly on a resolved stream URL —
/// yt-dlp when present (v5's own arguments: fragment-retrying,
/// 16-way concurrent), falling back to ffmpeg stream-copy when
/// yt-dlp is missing or fails, exactly as ani-cli 5.0's download()
/// chains them. Streams each stderr line into `on_line`; the child
/// dies with a dropped future (kill_on_drop), which is what the
/// dock's Cancel rides.
///
/// `path_env` is the PATH searched for the tools — the caller passes
/// the process environment; tests stage stub executables.
///
/// # Errors
/// [`AniError::FfmpegMissing`] when neither tool is on `path_env`
/// (the typed error the install modal renders);
/// [`AniError::Scraper`] when the chosen tool exits non-zero;
/// [`AniError::Timeout`] past the transfer deadline.
pub(crate) async fn spawn_download_tool<F>(
    master_url: &str,
    dest: &std::path::Path,
    file_stem: &str,
    quality: Option<&str>,
    path_env: &str,
    timeout: std::time::Duration,
    on_line: &mut F,
) -> Result<()>
where
    F: FnMut(&str) + Send,
{
    let target = dest.join(format!("{file_stem}.mp4"));
    let suffixes = crate::scraper::anidb::EXE_SUFFIXES;
    let ytdlp = find_tool(path_env, "yt-dlp", suffixes);
    let ffmpeg = find_tool(path_env, "ffmpeg", suffixes);
    if ytdlp.is_none() && ffmpeg.is_none() {
        // The typed error the frontend's install modal renders.
        return Err(AniError::FfmpegMissing);
    }
    if let Some(exe) = ytdlp {
        let mut cmd = tokio::process::Command::new(exe);
        cmd.arg(master_url)
            .arg("--no-skip-unavailable-fragments")
            .arg("--fragment-retries")
            .arg("infinite")
            .arg("-N")
            .arg("16")
            .arg("-o")
            .arg(&target);
        // v5 downloads the variant select_quality chose; the same
        // preference expressed through yt-dlp's format sort.
        match quality {
            Some("worst") => {
                cmd.arg("-S").arg("+res");
            }
            Some(q) if q.chars().all(|c| c.is_ascii_digit()) && !q.is_empty() => {
                cmd.arg("-S").arg(format!("res:{q}"));
            }
            _ => {}
        }
        match run_tool(cmd, timeout, on_line).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                // v5's && chain: a failing yt-dlp run retries the
                // whole stream through ffmpeg when one exists.
                if ffmpeg.is_none() {
                    return Err(e);
                }
                on_line("yt-dlp failed; retrying with ffmpeg");
            }
        }
    }
    let exe = ffmpeg.ok_or(AniError::FfmpegMissing)?;
    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg("-extension_picky")
        .arg("0")
        .arg("-loglevel")
        .arg("error")
        .arg("-stats")
        .arg("-i")
        .arg(master_url)
        .arg("-c")
        .arg("copy")
        .arg(&target);
    run_tool(cmd, timeout, on_line).await
}

/// First on-PATH hit for a tool name, widened by the platform's
/// executable suffix table: the scan builds and tests each full path
/// itself, so Windows command resolution (and its PATHEXT widening)
/// never runs — without the explicit table, an installed
/// `yt-dlp.exe` reads as missing. Bare name first within each
/// directory, like the curl transport's resolver.
fn find_tool(path_env: &str, name: &str, suffixes: &[&str]) -> Option<std::path::PathBuf> {
    std::env::split_paths(path_env).find_map(|d| {
        crate::scraper::anidb::candidate_names(name, suffixes)
            .into_iter()
            .map(|file| d.join(file))
            .find(|p| p.is_file())
    })
}

/// Run one download tool to completion, streaming stderr lines.
///
/// # Errors
/// [`AniError::Timeout`] past the deadline, [`AniError::Network`] on
/// spawn failure, [`AniError::Scraper`] on a non-zero exit.
async fn run_tool<F>(
    mut cmd: tokio::process::Command,
    timeout: std::time::Duration,
    on_line: &mut F,
) -> Result<()>
where
    F: FnMut(&str) + Send,
{
    use tokio::io::{AsyncBufReadExt, BufReader};
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // Own process group, so cancellation can address the tool's
    // whole tree: yt-dlp spawns helpers kill_on_drop cannot reach.
    #[cfg(unix)]
    cmd.process_group(0);
    let child = loop {
        match cmd.spawn() {
            Ok(c) => break c,
            // ETXTBSY: a concurrent fork briefly holds the tool's
            // write fd between its fork and exec (fds are CLOEXEC,
            // so the window is microseconds). Real for freshly
            // written executables — retry instead of failing the
            // whole download over it.
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            Err(_) => return Err(AniError::Network),
        }
    };
    // Dropped unreaped — the dock's Cancel aborting the SSE task, or
    // the transfer deadline elapsing — the guard takes the process
    // group down; a waited child is already reaped and the guard
    // stands down by itself.
    let mut child = crate::anicli::process::TreeKillChild::new(child);
    let stderr = child.child_mut().stderr.take().ok_or(AniError::Io)?;
    let drive = async {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            on_line(&line);
        }
        child.child_mut().wait().await.map_err(|_| AniError::Io)
    };
    let status = tokio::time::timeout(timeout, drive)
        .await
        .map_err(|_| AniError::Timeout)??;
    if status.success() {
        Ok(())
    } else {
        Err(AniError::Scraper {
            key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
        })
    }
}

#[cfg(test)]
mod tests {
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
            availability_refreshes:
                crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        }
    }

    /// Provider fixture for the range tests: one show, two episodes,
    /// jpn embeds, validating masters.
    async fn stub_range_show() -> wiremock::MockServer {
        use wiremock::matchers::{method, path};
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(method("GET"))
            .and(path("/browse"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(
                    r#"<a href="/anime/range-show-21"><img alt="Range Show"/></a>"#,
                ),
            )
            .mount(&server)
            .await;
        wiremock::Mock::given(method("GET"))
            .and(path("/api/frontend/anime/21/episodes"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(
                    r#"{"episodes":[{"id":2101,"number":1},{"id":2102,"number":2}]}"#,
                ),
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
        download_with_tools(&state, &args, &path_env, |_p| {})
            .await
            .expect("the range downloads episode by episode");
        let calls = std::fs::read_to_string(&log).expect("the tool ran");
        let lines: Vec<&str> = calls.lines().collect();
        assert_eq!(lines.len(), 2, "one tool run per episode: {calls}");
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
}
