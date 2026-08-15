//! Download an episode to disk. Mirrors the play command's
//! shape (same disambiguation, same Kitsu-driven select-index logic),
//! but instead of registering a stream session it spawns yt-dlp, or
//! ffmpeg when yt-dlp is absent or fails, to write the file to disk.
//!
//! Everything the dock shows arrives as one stream of text lines,
//! forwarded to the SSE handler in `api::get_download_stream`, and the
//! tool is only its last third. Resolution reports go first —
//! `Searching`, `Matched`, `… links fetched` — because they happen
//! before either tool exists. A range download interleaves its own
//! `Matched` and a `Playing episode N` per episode from the loop in
//! `download_range`. Then the tool's stderr. A change to the protocol
//! is a change to all three, not to yt-dlp's output alone.

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
    /// Canonical Kitsu title (drives the provider search step).
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

/// SSE event body for one line of the download's progress stream —
/// see the module header for the three sources that feed it. Frontend
/// renders the latest line under each active download row.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    /// One line of text. A tool's stderr arrives ANSI-stripped; the
    /// resolution and orchestration lines are composed here and carry
    /// no escapes to strip.
    pub line: String,
}

/// SSE final-event body. `dest_dir` is the directory the file landed
/// in, which is what a "reveal in folder" intent needs.
///
/// Only the directory, because the same response ends a range
/// download: `1-12` writes twelve files and there is no single name to
/// return. A single-episode download lands on `<resolved title>
/// Episode <n>.mp4` however it got there — including a yt-dlp run that
/// could not repackage and was retried through ffmpeg, which discards
/// the mislabeled file and writes the same target. Only a repackage
/// failure with no ffmpeg to retry through fails instead, with
/// [`AniError::FfmpegMissing`].
#[derive(Debug, Clone, Serialize)]
pub struct DownloadResponse {
    /// Directory the file was written to. Renderer feeds this to
    /// `revealInFolder` for the completion toast.
    pub dest_dir: String,
}

/// Drive a download from `args`. Picks the same (title, candidate
/// index) pair as the equivalent /api/play call so what was watched is
/// what gets saved, then spawns the downloader with the chosen
/// destination directory. `on_progress` is invoked for every line of
/// the progress stream — resolution reports, a range run's own
/// per-episode lines, and the tool's stderr alike.
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

    // Refuse before the provider is touched. With neither tool
    // installed the resolution has no use, so walking for it spends
    // the user's wait and the provider's patience to reach an answer
    // that was already knowable — and a walk that fails on the way
    // reports a network error over the real cause.
    //
    // `spawn_download_tool` still checks: it is reachable on its own
    // (the range loop, the tests) and a probe that ran once at the
    // top cannot speak for a tool uninstalled during an hour-long
    // transfer. This is the early refusal, not the authority.
    if !a_download_tool_exists(path_env) {
        return Err(AniError::FfmpegMissing);
    }

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
    let mut forward = |p: crate::commands::progress::ProgressLine| {
        let text = match p {
            crate::commands::progress::ProgressLine::LinksFetched { provider } => {
                format!("{provider} links fetched")
            }
            crate::commands::progress::ProgressLine::Searching { provider } => {
                format!("Searching {provider}...")
            }
            crate::commands::progress::ProgressLine::Matched { title } => {
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
/// yt-dlp is missing or fails, mirroring the script's download()
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
/// One lock per destination file, held for the whole spawn.
///
/// Two downloads of the same episode resolve to the same path — the
/// dock allows the second click — and before this they ran at once,
/// each writing the other's output. The repackage retry sharpened it
/// into deletion: that path removes the target and passes `-y`, so a
/// late arrival can replace a file the first run already finished.
///
/// Keyed by path rather than by show so a range download's episodes,
/// which differ, still run as the loop schedules them. Entries are
/// kept rather than reaped: one `Arc<Mutex>` per file a session has
/// downloaded is a few dozen bytes, and reaping introduces the race
/// this exists to remove — a waiter holding an `Arc` the map has
/// already dropped locks a mutex nobody else can see.
static TARGET_LOCKS: std::sync::Mutex<
    Option<std::collections::HashMap<std::path::PathBuf, std::sync::Arc<tokio::sync::Mutex<()>>>>,
> = std::sync::Mutex::new(None);

fn target_lock(target: &std::path::Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let mut guard = TARGET_LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .get_or_insert_with(std::collections::HashMap::new)
        .entry(target.to_path_buf())
        .or_default()
        .clone()
}

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
    // Held until this function returns, so everything below — the
    // discard, the removal, the `-y` run — happens with no other
    // invocation writing this path.
    let lock = target_lock(&target);
    let _writing = lock.lock().await;
    // The ceiling belongs to the transfer, not to each tool inside
    // it: one absolute instant, and every step below runs against
    // what is left of it.
    let deadline = tokio::time::Instant::now() + timeout;
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
    // Set only where this run wrote the file at the target and was
    // told it is unusable. It licenses `-y` below, and the license
    // does not extend to a path this run never touched.
    let mut replace_target = false;
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
        let mut repackage_failed = false;
        match run_tool(cmd, deadline, on_line, &mut repackage_failed).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                if repackage_failed {
                    // yt-dlp's own report: the fragments live under
                    // the .mp4 name as raw MPEG-TS. The file goes
                    // first either way — whatever happens next must
                    // not land on top of half a transfer in the wrong
                    // container.
                    crate::commands::download_tool::discard_mislabeled(dest, &[target.clone()]);
                    // The warning names the condition and suggests
                    // ffmpeg; it is not a report that ffmpeg is
                    // missing. So an install that has one gets the
                    // retry, and only an install without one is told
                    // to go and install it.
                    if ffmpeg.is_none() {
                        return Err(AniError::FfmpegMissing);
                    }
                    // `discard_mislabeled` recognizes MPEG-TS by its
                    // sync byte, and the warning's other half —
                    // malformed AAC timestamps — leaves a real MP4 it
                    // walks past. What yt-dlp left is condemned by its
                    // own report, so it goes regardless of shape
                    // rather than being written over and half
                    // forgotten. The result is ignored because nothing
                    // downstream depends on it: the ffmpeg run carries
                    // `-y` and replaces whatever survives, which is
                    // what a file this process cannot unlink does.
                    let _ = std::fs::remove_file(&target);
                    replace_target = true;
                    on_line("yt-dlp could not repackage the stream; retrying with ffmpeg");
                } else {
                    // v5's && chain: a failing yt-dlp run retries the
                    // whole stream through ffmpeg when one exists.
                    if ffmpeg.is_none() {
                        return Err(e);
                    }
                    on_line("yt-dlp failed; retrying with ffmpeg");
                }
            }
        }
    }
    let exe = ffmpeg.ok_or(AniError::FfmpegMissing)?;
    let mut cmd = tokio::process::Command::new(exe);
    if let Some(p) = &child_path {
        cmd.env("PATH", p);
    }
    // `-y` only for the retry. There it is required: yt-dlp wrote that
    // file during this run and condemned it, the removal ahead of the
    // retry is best-effort, and ffmpeg asked on a stdin with nothing
    // on it reads EOF and refuses — so a file that could not be
    // unlinked would fail a retry that has a working ffmpeg.
    //
    // Everywhere else the flag is the wrong answer. When ffmpeg is the
    // first tool, anything at that path predates this download, and
    // the confirm dialog asks the user for a directory and never for
    // permission to overwrite. Refusing is the honest outcome there.
    if replace_target {
        cmd.arg("-y");
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
    // The warning is yt-dlp's; ffmpeg cannot set the flag.
    run_tool(cmd, deadline, on_line, &mut false).await
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
            // Executability, not mere existence: a regular file the
            // platform cannot exec (an extraction that missed
            // chmod +x) must not hide a usable tool further along
            // the search.
            .find(|p| crate::scraper::anidb::is_executable(p))
    })
}

/// Whether either tool that can perform a transfer is installed.
///
/// One definition for the two places that ask: `download_with_tools`
/// refuses on it before resolving, and `spawn_download_tool` re-checks
/// at the moment it would spawn. Both need the same answer to the same
/// question, and a second hand-written pair of `find_tool` calls is how
/// they would drift apart.
fn a_download_tool_exists(path_env: &str) -> bool {
    let suffixes = crate::scraper::anidb::EXE_SUFFIXES;
    find_tool(path_env, "yt-dlp", suffixes).is_some()
        || find_tool(path_env, "ffmpeg", suffixes).is_some()
}

/// Run one download tool to completion, streaming stderr lines.
///
/// # Errors
/// [`AniError::Timeout`] past the deadline, [`AniError::Network`] on
/// spawn failure, [`AniError::Scraper`] on a non-zero exit.
async fn run_tool<F>(
    mut cmd: tokio::process::Command,
    deadline: tokio::time::Instant,
    on_line: &mut F,
    repackage_failed: &mut bool,
) -> Result<()>
where
    F: FnMut(&str) + Send,
{
    use tokio::io::{AsyncBufReadExt, BufReader};
    // §5's subprocess environment: nothing the child prints may
    // depend on the terminal that launched the backend. Both tools
    // colorize when the inherited environment says to, and every
    // line here goes straight to the dock.
    cmd.env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // Own process group, so cancellation can address the tool's
    // whole tree: yt-dlp spawns helpers kill_on_drop cannot reach.
    #[cfg(unix)]
    cmd.process_group(0);
    let child = loop {
        // The retry lives before the child exists, so the wait below
        // cannot cover it — the deadline has to, or a tool that
        // never becomes runnable spins here forever.
        if tokio::time::Instant::now() >= deadline {
            return Err(AniError::Timeout);
        }
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
    let mut child = crate::spawn::TreeKillChild::new(child);
    let stderr = child.child_mut().stderr.take().ok_or(AniError::Io)?;
    let drive = async {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(raw)) = lines.next_line().await {
            // Defense in depth behind the environment above: the
            // dock's DownloadProgress promises stripped text, and a
            // tool that colorizes anyway must not reach it.
            let line = crate::spawn::strip_ansi(raw.as_bytes());
            // The run is condemned the moment yt-dlp reports it left
            // MPEG-TS under the .mp4 name: how it ends stops
            // mattering (exit 0 included), and stopping now spares
            // the rest of a transfer whose output is already wrong.
            // The armed guard takes the tool down on return.
            if crate::commands::download_tool::yt_dlp_could_not_repackage(&line) {
                *repackage_failed = true;
                return Err(AniError::FfmpegMissing);
            }
            on_line(&line);
        }
        child.child_mut().wait().await.map_err(|_| AniError::Io)
    };
    let status = tokio::time::timeout_at(deadline, drive)
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
