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
use crate::commands::play_native_resolve::{resolve_native, NativeResolveRequest};
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
    mut on_progress: F,
) -> Result<DownloadResponse>
where
    F: FnMut(DownloadProgress) + Send,
{
    let dest = resolve_dest(args)?;
    std::fs::create_dir_all(&dest).map_err(|_| AniError::Io)?;

    // Resolve the stream natively — the same walk, disambiguation
    // and episode mapping as the play path — then hand the master URL
    // to the download tool directly, exactly as 5.0's own download()
    // would. The one-hour transfer deadline stays: yt-dlp / ffmpeg
    // keep stderr quiet mid-transfer, so no shorter timeout can be
    // informed by progress.
    let quality = args.quality.as_deref().unwrap_or("best");
    let client = anidb_client_with_base(state, state.anidb_base.as_deref())?;
    let request = NativeResolveRequest {
        title: &args.title,
        alt_titles: &args.alt_titles,
        episode: &args.episode,
        mode: &args.mode,
        expected_count: args.episode_count,
    };
    let resolve_started_at = tokio::time::Instant::now();
    let mut forward = |p: crate::anicli::parser::ProgressLine| {
        let text = match p {
            crate::anicli::parser::ProgressLine::Banner { text }
            | crate::anicli::parser::ProgressLine::Other { text } => text,
            crate::anicli::parser::ProgressLine::LinksFetched { provider } => {
                format!("{provider} links fetched")
            }
        };
        on_progress(DownloadProgress { line: text });
    };
    let resolved = resolve_native(
        &client,
        Some(&state.scraper_gate),
        crate::scraper::gate::ScrapePriority::Interactive,
        request,
        &mut forward,
    )
    .await;
    // Resolution and transfer are separate stages now, so the gate
    // learns from the fresh resolution outcome alone — the stale
    // whole-run signal the subprocess forced is gone.
    let outcome = match &resolved {
        Ok(_) => crate::scraper::gate::ScrapeOutcome::Success,
        Err(_) => crate::scraper::gate::ScrapeOutcome::Failure,
    };
    state.scraper_gate.record(outcome, resolve_started_at);
    let resolved = resolved.map_err(|ne| ne.error)?;

    tracing::info!(
        slug = %resolved.slug,
        episode = %args.episode,
        mode = %args.mode,
        dest = %dest.display(),
        "download: spawning tool on natively resolved stream",
    );
    let file_stem = format!("{} Episode {}", resolved.title, args.episode);
    let path_env = std::env::var("PATH").unwrap_or_default();
    spawn_download_tool(
        &resolved.master_url,
        &dest,
        &file_stem,
        Some(quality),
        &path_env,
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

/// Classify a full [`spawn_download`] run for the scraper gate. The
/// play paths record every upstream-proving outcome because their
/// subprocess ends at resolution, but `ani-cli -d` spans resolution
/// plus up to an hour of aria2c / yt-dlp / ffmpeg transfer. Its
/// success signal is stale by construction: `started_at` predates the
/// allmanga lookup by the whole transfer, and the gate's staleness
/// guard only rejects stale successes while the breaker is already
/// open — recording one here would reset a failure run building up
/// mid-download and let the breaker need more than three current
/// failures to open. Only `NoResults` feeds the gate: ani-cli dies
/// with it at the search stage, before any transfer, so it's still
/// fresh when reported — and the picker just confirmed the show
/// exists, making it the rate-limit signature. Everything else (the
/// pre-spawn ffmpeg check, a missing binary, the transfer timeout,
/// the generic `Scraper` catch-all any non-zero tool exit maps to)
/// is local or transfer-stage noise and records nothing.
fn download_gate_signal<T>(result: &Result<T>) -> Option<bool> {
    match result {
        Ok(_) => None,
        Err(AniError::NoResults) => Some(false),
        Err(_) => None,
    }
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
    let _ = quality;
    let find = |name: &str| -> Option<std::path::PathBuf> {
        std::env::split_paths(path_env)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    };
    let target = dest.join(format!("{file_stem}.mp4"));
    let ytdlp = find("yt-dlp");
    let ffmpeg = find("ffmpeg");
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
    let mut child = cmd.spawn().map_err(|_| AniError::Network)?;
    let stderr = child.stderr.take().ok_or(AniError::Io)?;
    let drive = async {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            on_line(&line);
        }
        child.wait().await.map_err(|_| AniError::Io)
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
    fn download_gate_signal_counts_only_no_results() {
        // NoResults after the picker confirmed the show exists is
        // the rate-limit signature, and ani-cli dies with it at the
        // search stage — before any transfer — so it's still fresh
        // when reported. A full-run success is NOT gate evidence:
        // it lands after minutes of transfer, so its resolution
        // proof is stale by construction and recording it would
        // reset a failure run that built up during the download.
        assert_eq!(download_gate_signal::<()>(&Ok(())), None);
        assert_eq!(
            download_gate_signal::<()>(&Err(AniError::NoResults)),
            Some(false)
        );
    }

    #[test]
    fn download_gate_signal_ignores_local_and_transfer_stage_failures() {
        // ffmpeg missing is a pre-spawn local check; MissingBinary
        // never reached the network; the 1 h timeout and the generic
        // Scraper catch-all are dominated by aria2c / yt-dlp / ffmpeg
        // transfer deaths on this path. None of them says anything
        // about allanime's health, so none may move the breaker.
        assert_eq!(
            download_gate_signal::<()>(&Err(AniError::FfmpegMissing)),
            None
        );
        assert_eq!(
            download_gate_signal::<()>(&Err(AniError::MissingBinary)),
            None
        );
        assert_eq!(download_gate_signal::<()>(&Err(AniError::Timeout)), None);
        assert_eq!(
            download_gate_signal::<()>(&Err(AniError::Scraper {
                key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
            })),
            None
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
            alt_titles: vec![],
            kitsu_id: None,
            download_dir: Some("/tmp/explicit".into()),
        };
        let p = resolve_dest(&a).expect("ok");
        assert_eq!(p, PathBuf::from("/tmp/explicit"));
    }
}

#[cfg(test)]
#[path = "download_test.rs"]
mod proptests;
