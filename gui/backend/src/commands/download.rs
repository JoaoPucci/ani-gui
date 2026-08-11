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

    // The bundled-binary directory joins the search ahead of PATH —
    // packaged builds ship their own yt-dlp (Windows: ffmpeg too),
    // and the bundle is the tool the package validated, exactly as
    // the curl resolver ranks its bundled directory first.
    let path_env = tool_search_path(state, path_env);
    let path_env = path_env.as_str();

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
    let child_path = std::env::join_paths(std::env::split_paths(path_env).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .ok();
    if let Some(exe) = ytdlp {
        let mut cmd = tokio::process::Command::new(exe);
        if let Some(p) = &child_path {
            cmd.env("PATH", p);
        }
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
    if let Some(p) = &child_path {
        cmd.env("PATH", p);
    }
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

/// The downloader's tool-search path: the bundled-binary directory
/// (when packaging ships one) ahead of the caller's PATH. Also what
/// the spawned tool itself sees as PATH, so a bundled yt-dlp finds
/// the bundled ffmpeg for its own repackaging.
fn tool_search_path(state: &AppState, path_env: &str) -> String {
    match &state.bundled_bin {
        Some(dir) => std::env::join_paths(
            std::iter::once(dir.clone()).chain(std::env::split_paths(path_env)),
        )
        .map(|joined| joined.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path_env.to_string()),
        None => path_env.to_string(),
    }
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
#[path = "download_test.rs"]
mod tests;
