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
    /// provider hits. Same wire forms as [`PlayArgs::alt_titles`].
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
/// Keyed by the target's verbatim lock path rather than by show, so
/// a range download's episodes, which differ, still run as the loop
/// schedules them — see `target_lock` for why the key carries the
/// path as given instead of a folded spelling of it. Entries are
/// kept rather than reaped: one `Arc<Mutex>` per file a session has
/// downloaded is a few dozen bytes, and reaping introduces the race
/// this exists to remove — a waiter holding an `Arc` the map has
/// already dropped locks a mutex nobody else can see.
static TARGET_LOCKS: std::sync::Mutex<
    Option<std::collections::HashMap<std::path::PathBuf, std::sync::Arc<tokio::sync::Mutex<()>>>>,
> = std::sync::Mutex::new(None);

/// Where the lock that crosses app instances lives for `target`.
///
/// The map above is per-process, so a second copy of the app shares
/// none of it. What both instances do share is the filesystem, and a
/// lock file named the same way by both is enough for the kernel to
/// order them — including when one is killed mid-download, since it
/// releases the lock on the dying process's behalf.
///
/// Placed beside the target, in a `.ani-gui-locks/` directory next to
/// the file, and named after the file itself.
///
/// The location cannot depend on anything a process is configured
/// with. Two earlier attempts did: the app cache directory moves with
/// the dev profile, and the profile-independent one still resolved
/// under `$XDG_CACHE_HOME`, which any launcher can set. The
/// destination is what both contenders provably agree on — they were
/// handed the same one, or they would not be racing — and it is
/// writable by construction, since a video is about to land in it,
/// which is what keeps refusing on a lock failure reasonable.
///
/// The *name* carries the target's own, and that is the part worth
/// explaining, because it replaced a digest of a case-folded key.
/// Whether two names are one file is the filesystem's question, and
/// it answers about `Show.mp4.lock` exactly as it answers about
/// `Show.mp4`: same case rules, same canonical normalization, same
/// trailing-dot handling, same anything a volume does that nothing
/// here knows about. Hashing a folded key substituted a case table
/// for that, and the table was wrong three times — first on case,
/// then on final sigma, then on normalization — each found
/// separately because a table can only be wrong one entry at a time.
///
/// Delegating also gets the case-sensitive filesystems right, which
/// folding could not: there `Show.mp4` and `show.mp4` really are two
/// files, and now they take two locks instead of queueing behind one.
///
/// The target's own name, in a sibling directory.
///
/// This is no longer what makes concurrent downloads safe — publishing
/// is, by asking the filesystem for the name rather than reproducing
/// its identity anywhere. The lock is an optimization: it stops two
/// clicks on one episode both spending the bandwidth, and it is
/// allowed to be wrong. Where it is fooled, the second transfer runs
/// and then finds the name taken, which costs a download and loses
/// nothing.
///
/// That reframing is why the naming can stay simple. It carries the
/// target's name verbatim so the filesystem's own answers about case,
/// normalization and the rest apply for free; where they do not —
/// NTFS 8.3 aliases are assigned per directory, so a target and its
/// lock can alias differently — the consequence is now a wasted
/// transfer rather than a deleted file.
///
/// The cost the user sees is a directory in their download folder. It
/// is dot-prefixed, so hidden on Linux and macOS and visible on
/// Windows.
pub(crate) fn target_lock_path(target: &std::path::Path) -> Option<std::path::PathBuf> {
    let parent = target.parent()?;
    Some(parent.join(".ani-gui-locks").join(target.file_name()?))
}

/// How often a download waiting on another instance re-tries the OS
/// lock. Long enough that a queued download is not spinning, short
/// enough that the handover is not what the user notices.
const INSTANCE_LOCK_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// Take the lock that crosses app instances for `target`, waiting for
/// a holder until `deadline`.
///
/// Polled rather than blocked. `flock` parks the thread that calls it,
/// and a parked thread is not a future: cancelling the download drops
/// the `JoinHandle` while the closure stays inside the call, so the
/// dock reports a cancel that left a blocking-pool thread behind. Do
/// that a few times and the pool starves every other blocking caller,
/// invisibly. Waiting between polls on the async side means a
/// cancelled download lands on an await point and is simply dropped.
///
/// Only the open runs on the blocking pool, where it cannot park
/// indefinitely — it either finds the file or fails.
///
/// # Errors
/// [`AniError::Timeout`] when the deadline passes with another
/// instance still holding it — the transfer's own ceiling, since a
/// download blocked on another instance is spending the wait the
/// transfer was given. [`AniError::Io`] when the lock file cannot be
/// created or opened at all; see [`target_lock_path`] for why that is
/// worth refusing over.
pub(crate) async fn acquire_instance_lock(
    target: &std::path::Path,
    deadline: tokio::time::Instant,
) -> Result<std::fs::File> {
    let path = target_lock_path(target).ok_or(AniError::Io)?;
    let file = tokio::task::spawn_blocking(move || -> Result<std::fs::File> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| AniError::Io)?;
        }
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|_| AniError::Io)
    })
    .await
    .map_err(|_| AniError::Io)??;
    loop {
        match fs4::FileExt::try_lock(&file) {
            Ok(()) => return Ok(file),
            Err(fs4::TryLockError::WouldBlock) => {}
            Err(fs4::TryLockError::Error(_)) => return Err(AniError::Io),
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(AniError::Timeout);
        }
        tokio::time::sleep_until(deadline.min(now + INSTANCE_LOCK_POLL)).await;
    }
}

fn target_lock(target: &std::path::Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    // Keyed by the lock path, so this and the file lock share one
    // notion of identity rather than two that can disagree. It is the
    // weaker of the pair: two names a case-insensitive filesystem
    // calls one file get two entries here and meet at the file lock
    // instead, which is exact. Guessing at that equivalence again to
    // catch them a layer earlier is what this just stopped doing.
    let key = target_lock_path(target).unwrap_or_else(|| target.to_path_buf());
    let mut guard = TARGET_LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .get_or_insert_with(std::collections::HashMap::new)
        .entry(key)
        .or_default()
        .clone()
}

/// Owns the scratch file for as long as the transfer might still be
/// using it, and removes it otherwise.
///
/// Cancellation is why this is a type rather than a cleanup call.
/// Aborting the download drops the future wherever it happens to be
/// awaiting, so no branch below that point runs — and the file left
/// behind is not a marker but most of an episode. A drop runs anyway.
///
/// Publication is the one exit that stands the guard down. A
/// successful one ends with the scratch consumed, and a tool that
/// handed over a finished file has already cleaned its own derived
/// names — so the drop's sweep would read every entry in the
/// destination to find nothing, once per episode of a range download,
/// on whatever filesystem the user chose. The flag is set after
/// publication answers, never before it: set ahead of the call, it
/// disarms the guard for the failures too, which is the off-by-one-
/// branch this struct once shipped. And it is set on the scratch
/// being gone rather than on the answer, because publication ignores
/// a failed removal on its success arms — a delivered episode is not
/// an error — and disarming on the report alone would leave that one
/// failure's episode-sized file hidden in the folder for good.
struct Scratch {
    path: std::path::PathBuf,
    published: std::sync::atomic::AtomicBool,
}

impl Scratch {
    fn new(dest: &std::path::Path) -> Self {
        Self {
            path: scratch_path(dest),
            published: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Point at a fresh file, dropping whatever the last one was —
    /// the retry path, where yt-dlp's output is condemned and ffmpeg
    /// needs a name nothing can confuse with it.
    fn renew(&mut self, dest: &std::path::Path) {
        self.discard();
        self.path = scratch_path(dest);
    }

    /// Remove the scratch file and everything a tool derived from its
    /// name.
    ///
    /// The `-o` path is not where a transfer's bytes are. yt-dlp's
    /// `--part` is on by default, so they go to `<scratch>.part`, with
    /// `<scratch>.ytdl` holding fragment-resume state and a
    /// `<scratch>.part-FragN` per fragment in flight beside it. A kill
    /// lands among those, and removing only the `-o` path removes the
    /// one name nothing was written to.
    ///
    /// Enumerating that list here would be the same mistake as
    /// reconstructing which spellings are one file: correct until the
    /// tool adds a suffix, and unfixable without knowing it had. The
    /// scratch name carries a uuid this run generated, so a prefix
    /// sweep of the destination is exact — nothing else can hold that
    /// prefix, and nothing derived from the path can lose it.
    fn discard(&self) {
        let _ = std::fs::remove_file(&self.path);
        let (Some(dir), Some(prefix)) = (self.path.parent(), self.path.file_name()) else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            if entry
                .file_name()
                .as_encoded_bytes()
                .starts_with(prefix.as_encoded_bytes())
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.published.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        self.discard();
    }
}

/// Where a transfer writes before it is anyone's episode.
///
/// Beside the target rather than in a temp directory, because
/// publishing is a rename or a link and both need the same
/// filesystem. Carries `.mp4` because yt-dlp and ffmpeg both choose a
/// container from the extension, and a scratch name they cannot read
/// as MP4 changes what they produce.
fn scratch_path(dest: &std::path::Path) -> std::path::PathBuf {
    dest.join(format!(
        ".ani-gui-{}-{}.part.mp4",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ))
}

/// Said when the download ends because the file is already there —
/// found before starting, or found by publication after another
/// transfer won the race. The two are the same event to the person
/// watching the dock, and the same line.
///
/// A key rather than a sentence, like every report this backend
/// composes for the progress stream: the dock shows these lines
/// verbatim, so a sentence written here is UI copy that bypasses
/// Paraglide and reaches three locales in English. The frontend
/// resolves the key; the sentence lives in the message bundles.
/// Where a report carries a path, the path follows the key after
/// the first space, verbatim, whatever the title brought with it.
const ALREADY_HERE: &str = "status.download.already_here";

/// How long a claim has to sit untouched before it counts as
/// abandoned.
///
/// It separates two things nothing here can tell apart by asking:
/// another publisher's claim, live between its `create_new` and its
/// `rename`, and one whose process died in that window. There is no
/// owner to ask — the path carries no identity — but the two differ
/// enormously in age. A live claim is two syscalls old. An abandoned
/// one is as old as the crash and only gets older.
///
/// A minute is far above the first and far below any wait a person
/// would notice, and well clear of FAT's two-second timestamp
/// granularity. Getting it wrong in the safe direction costs one
/// download reported as already-here; in the unsafe direction it
/// costs somebody's file.
const CLAIM_GRACE: std::time::Duration = std::time::Duration::from_secs(60);

/// How long to wait for another publisher's claim to become an
/// episode.
///
/// A claim exists between a `create_new` and the `rename` after it,
/// which is two syscalls. Standing down for one is standing down for a
/// file that is about to be there — unless its owner died in that
/// window, and then nothing lands at all. Waiting is what tells those
/// apart, and it needs no lock and no guess: the first resolves
/// immediately, the second never does.
const CLAIM_SETTLE: std::time::Duration = std::time::Duration::from_secs(2);

/// How often to look while waiting.
const CLAIM_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// What is sitting at a target path.
///
/// Publication used to ask one boolean of it, and the interesting
/// cases are not two. An empty file is a claim rather than an episode
/// — that is what publication leaves between creating the name and
/// renaming onto it — and whether it may be taken depends on whether
/// anyone is still coming back for it. A directory or a fifo is
/// neither, and treating it as an episode already there reported a
/// download that produced nothing playable.
#[derive(Debug, PartialEq, Eq)]
enum AtTarget {
    /// Free.
    Nothing,
    /// A file with bytes in it. The user's, and not ours to replace.
    Episode,
    /// An empty file young enough that its publisher is still running.
    LiveClaim,
    /// An empty file old enough that nobody is coming back for it.
    AbandonedClaim,
    /// Something that is not a regular file. No episode can go here.
    Obstruction,
    /// The path could not be looked at. Permissions changed, a
    /// network folder went away, the volume errored — the answer is
    /// unknown, which is not the same as free and must never be
    /// mistaken for it.
    Unreadable,
}

fn at_target(path: &std::path::Path) -> AtTarget {
    // Without following links, because following them answers the
    // wrong question twice. A dangling link reports NotFound and the
    // name reads as free, though it is held — the transfer then runs
    // to completion and `hard_link` fails with the same
    // `AlreadyExists` an episode gives. A link that resolves is no
    // better: publishing through it writes wherever it points, which
    // is not the folder the confirm dialog asked about.
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        // Only a missing path is a free one. Every other error was
        // read as free too, which turned an unreadable folder into a
        // finished download: the transfer check reads "not an episode"
        // as the tool having written nothing, and that ends happily.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return AtTarget::Nothing,
        Err(_) => return AtTarget::Unreadable,
    };
    if !meta.is_file() {
        return AtTarget::Obstruction;
    }
    if meta.len() > 0 {
        return AtTarget::Episode;
    }
    // Unreadable or future-dated timestamps count as live, which is
    // the direction that loses a download rather than a file.
    let aged = meta
        .modified()
        .ok()
        .and_then(|m| m.elapsed().ok())
        .is_some_and(|age| age >= CLAIM_GRACE);
    if aged {
        AtTarget::AbandonedClaim
    } else {
        AtTarget::LiveClaim
    }
}

/// Classify `path`, waiting out a claim that is still resolving.
///
/// Returns as soon as the answer is not `LiveClaim`, so the common
/// case — nobody publishing — costs one `stat`. A claim whose owner
/// completes costs the microseconds it takes them. A claim whose owner
/// died costs the full wait, once, and then the caller learns that
/// nothing is coming.
async fn at_target_settled(path: &std::path::Path) -> AtTarget {
    let until = tokio::time::Instant::now() + CLAIM_SETTLE;
    loop {
        let seen = at_target(path);
        if seen != AtTarget::LiveClaim || tokio::time::Instant::now() >= until {
            return seen;
        }
        tokio::time::sleep(CLAIM_POLL).await;
    }
}

/// What happened when a finished transfer went to take its name.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Published {
    /// The file is now at the target, written by this transfer.
    Installed,
    /// Something was already there. Either another transfer got in
    /// first while this one ran, or the episode was on disk before it
    /// started — from here those look the same, and the answer is the
    /// same too: the file present is the one that stays.
    AlreadyThere,
}

/// Install `scratch` at `target` without displacing anything.
///
/// The confirm dialog asks the user for a directory and never for
/// permission to overwrite, so a file already at that name is one they
/// did not agree to lose — whether it arrived a moment ago from
/// another download or last week from this one. That rule used to hold
/// on the ffmpeg path only, by withholding `-y`, while a yt-dlp run
/// wrote over the same file without asking. It holds on both now.
///
/// A hard link is what asks the filesystem for a name only if nobody
/// holds it, and the filesystem answers using its own notion of which
/// names are one file — the notion that matters, and the one nothing
/// here can reconstruct. Where links are unsupported (FAT32 on a
/// memory stick) the claim is a create-new instead, which is the same
/// question asked a different way.
///
/// # Errors
/// [`AniError::Io`] when the finished file cannot be installed and
/// nothing was already there to explain why.
pub(crate) async fn publish(
    scratch: &std::path::Path,
    target: &std::path::Path,
) -> Result<Published> {
    match at_target_settled(target).await {
        // Reported, not taken. Three shapes were tried for taking it
        // and none of them worked, for one reason: deciding a file is
        // abandoned and acting on that decision are separate steps,
        // and this filesystem offers nothing to fuse them. Whatever
        // the act is — unlink, or rename over — it can land on
        // something that stopped being a claim in between.
        //
        // The lock cannot stand in for that. It is named for the
        // target, so two spellings of one name take two lock files and
        // leave both holders believing they are alone; and requiring
        // it blocked recovery entirely wherever no lock can be made.
        //
        // Underneath was a demand for two guarantees at once: never
        // leave a name blocked, and never replace a file this app
        // cannot prove is its own. On a filesystem with no conditional
        // replace they do not both fit, and the second is the one this
        // branch has spent itself establishing — the confirm dialog
        // asks for a directory and never for permission to overwrite.
        //
        // So the app says what is in the way and stops. The caller
        // names the file on the progress stream, which is what turns
        // this from a dead end into one deletion.
        AtTarget::AbandonedClaim => {
            let _ = std::fs::remove_file(scratch);
            return Err(AniError::Io);
        }
        // Still a claim after waiting it out, which means its owner is
        // not coming back — it died between creating the name and
        // renaming onto it. Standing down here would discard this
        // finished transfer and report the episode present, when what
        // is present is an empty file the next attempt will refuse.
        //
        // Taking it is still not allowed, for the reason above. So the
        // download ends unresolved and says so.
        AtTarget::LiveClaim => {
            let _ = std::fs::remove_file(scratch);
            return Err(AniError::Io);
        }
        AtTarget::Obstruction | AtTarget::Unreadable => return Err(AniError::Io),
        AtTarget::Nothing | AtTarget::Episode => {}
    }
    if std::fs::hard_link(scratch, target).is_ok() {
        // The link is the file now; the scratch name is one extra
        // way to reach it and no longer wanted.
        let _ = std::fs::remove_file(scratch);
        return Ok(Published::Installed);
    }
    // Any refusal, `AlreadyExists` included, is handed to the
    // link-free path, whose create-new asks the same question and
    // knows how to read a collision. Reading `AlreadyExists` here as
    // the episode being present repeated the fallback's old mistake
    // on the filesystems the fallback exists for: the kernel answers
    // it for an occupied name before consulting the filesystem's
    // link support at all, so on FAT a link attempt sees another
    // publisher's empty claim exactly as a create-new does — and
    // stood down from it, discarding a finished transfer for a file
    // with nothing in it.
    publish_without_links(scratch, target).await
}

/// Publication where the filesystem has no hard links (FAT32 or exFAT
/// on a memory stick).
///
/// Claiming the name by creating it asks the same question a link
/// does — nobody else can hold it afterwards — and the rename then
/// lands on this run's own empty file rather than anyone's episode.
///
/// A refusal answers less than a link's does, though. `AlreadyExists`
/// from a link means a file with bytes in it holds the name; from a
/// create-new it means only that the name is taken — and on the one
/// filesystem this path serves, the taker can be another publisher's
/// claim, empty and moments old, whose owner may yet die before its
/// rename. Reporting the episode present on that evidence discards a
/// finished transfer in favor of an empty file. So a collision looks
/// at what it actually hit, waiting a claim out the same way the
/// front door does: bytes are the episode this was assumed to be, a
/// claim that persists is a failure to report, and a name gone free
/// again — the claimant failed its rename and cleaned up — is
/// claimed once more.
pub(crate) async fn publish_without_links(
    scratch: &std::path::Path,
    target: &std::path::Path,
) -> Result<Published> {
    let mut reclaimed = false;
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
        {
            Ok(_) => {
                return match std::fs::rename(scratch, target) {
                    Ok(()) => Ok(Published::Installed),
                    Err(_) => {
                        // The claim is an empty file at the episode's name.
                        // Left there it reads as a finished download to
                        // everything downstream — including the next attempt,
                        // which would stand down from it — so a publish that
                        // cannot complete takes its claim with it.
                        let _ = std::fs::remove_file(target);
                        Err(AniError::Io)
                    }
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                match at_target_settled(target).await {
                    AtTarget::Episode => {
                        let _ = std::fs::remove_file(scratch);
                        return Ok(Published::AlreadyThere);
                    }
                    // The claimant failed its rename and removed its
                    // claim on the way out, so the name is free again.
                    // Once, because a name that keeps being claimed and
                    // abandoned while this publisher watches is a fight
                    // to report, not a race that resolved.
                    AtTarget::Nothing if !reclaimed => {
                        reclaimed = true;
                    }
                    // A claim that outlasted the wait — its owner died
                    // between its create and its rename — or something
                    // that is not a file at all. Nothing here may take
                    // it (see `publish`), so the transfer ends
                    // unresolved rather than reported as the episode.
                    _ => {
                        let _ = std::fs::remove_file(scratch);
                        return Err(AniError::Io);
                    }
                }
            }
            Err(_) => return Err(AniError::Io),
        }
    }
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
    // The ceiling belongs to the transfer, not to each step inside
    // it: one absolute instant, and everything below runs against
    // what is left of it. Taken before either lock, because waiting
    // for whoever holds this file is time the transfer is spending —
    // and the two waits are one wait to the user, who is looking at a
    // queued download either way.
    //
    // It sat between them once, so a duplicate click waited out the
    // first download's whole hour and then started an hour of its
    // own, while the identical wait against another app instance
    // ended at the deadline.
    let deadline = tokio::time::Instant::now() + timeout;
    // Held until this function returns, so everything below — the
    // discard, the removal, the `-y` run — happens with no other
    // invocation writing this path.
    let lock = target_lock(&target);
    let Ok(_writing) = tokio::time::timeout_at(deadline, lock.lock()).await else {
        return Err(AniError::Timeout);
    };
    // And the other app instance's download. Taken after the
    // in-process mutex so this process's own tasks queue on the cheap
    // lock and only one of them contends the file. Held to the return
    // — dropping the handle closes the descriptor, which is what
    // releases it.
    // Best-effort, because publishing is what makes this safe. A lock
    // that cannot be taken costs a duplicate transfer — which then
    // finds the name taken and discards its copy — and refusing the
    // download instead would deny an episode the filesystem was
    // perfectly willing to store.
    let _instance = acquire_instance_lock(&target, deadline).await.ok();
    // Nothing at this name can be replaced, so a transfer into a
    // folder that already has the episode cannot change what the user
    // ends up with. It can still spend an episode of their bandwidth
    // and most of the hour the download is allowed, and then throw the
    // result away — so the answer is given before a tool is spawned
    // rather than after one has finished.
    //
    // Below both locks, because above them it would be answering
    // while another download is midway through creating that file,
    // and the interesting case is exactly the one where it appears.
    match at_target_settled(&target).await {
        // Free.
        AtTarget::Nothing => {}
        // An interrupted publication left an empty file at the
        // episode's name. Nothing can install over it safely, so the
        // dock names it and the user removes it — before a transfer
        // rather than after, since the answer cannot change.
        AtTarget::AbandonedClaim => {
            on_line(&format!(
                "status.download.abandoned_claim {}",
                target.display()
            ));
            return Err(AniError::Io);
        }
        // Present. Publication would only discard this transfer, so
        // the answer is given before one is spent.
        AtTarget::Episode => {
            on_line(ALREADY_HERE);
            return Ok(());
        }
        // A claim that outlasted the wait. Its owner is not coming
        // back, and nothing here may take it, so a transfer would end
        // in the same refusal an hour later.
        AtTarget::LiveClaim => {
            on_line(&format!(
                "status.download.claim_pending {}",
                target.display()
            ));
            return Err(AniError::Io);
        }
        // Nothing playable can be installed here, and no amount of
        // transferring changes that.
        AtTarget::Obstruction | AtTarget::Unreadable => return Err(AniError::Io),
    }
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
    // Everything below writes here. Nothing is at the target until a
    // whole file exists, so a half transfer is never sitting under the
    // name the dock, the file manager and every other download read.
    let mut scratch = Scratch::new(dest);
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
            .arg(&scratch.path);
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
            Ok(()) => return finish(&scratch, &target, on_line).await,
            Err(e) => {
                if repackage_failed {
                    // yt-dlp's own report: what it wrote is raw
                    // MPEG-TS under an .mp4 name. It never reached the
                    // target, so this is only a scratch file to drop —
                    // the removal that used to happen here was against
                    // the user's own file, and the `-y` that licensed
                    // the retry to write over it is gone with it.
                    // The warning names the condition and suggests
                    // ffmpeg; it is not a report that ffmpeg is
                    // missing. So an install that has one gets the
                    // retry, and only an install without one is told
                    // to go and install it.
                    if ffmpeg.is_none() {
                        return Err(AniError::FfmpegMissing);
                    }
                    // A fresh name for the retry rather than writing
                    // over the condemned one, so nothing it leaves can
                    // be mistaken for what ffmpeg produces.
                    scratch.renew(dest);
                    on_line("status.download.repackage_retry");
                } else {
                    // v5's && chain: a failing yt-dlp run retries the
                    // whole stream through ffmpeg when one exists.
                    if ffmpeg.is_none() {
                        return Err(e);
                    }
                    scratch.renew(dest);
                    on_line("status.download.retry_ffmpeg");
                }
            }
        }
    }
    let exe = ffmpeg.ok_or(AniError::FfmpegMissing)?;
    let mut cmd = tokio::process::Command::new(exe);
    if let Some(p) = &child_path {
        cmd.env("PATH", p);
    }
    // `-y` unconditionally, and it is no longer a decision. The output
    // is a name this run invented and nothing else can hold, so there
    // is never a user's file behind it to protect — the flag only
    // stops ffmpeg asking a question on a stdin that will answer EOF.
    // What the download does to an episode already on disk is settled
    // at publication instead, where it can tell a file the user
    // already had from one that arrived mid-transfer.
    cmd.arg("-y");
    cmd.arg("-extension_picky")
        .arg("0")
        .arg("-loglevel")
        .arg("error")
        .arg("-stats")
        .arg("-i")
        .arg(master_url)
        .arg("-c")
        .arg("copy")
        .arg(&scratch.path);
    // The warning is yt-dlp's; ffmpeg cannot set the flag.
    match run_tool(cmd, deadline, on_line, &mut false).await {
        Ok(()) => finish(&scratch, &target, on_line).await,
        Err(e) => Err(e),
    }
}

/// Publish a finished transfer and say so on the progress stream.
///
/// The download that loses a race has still put the episode where the
/// user asked for it — by not being the one to write it — so it ends
/// as a success with a line explaining why nothing changed, rather
/// than an error about a file that is present and complete.
async fn finish<F>(scratch: &Scratch, target: &std::path::Path, on_line: &mut F) -> Result<()>
where
    F: FnMut(&str) + Send,
{
    // A tool that exits cleanly having written nothing leaves nothing
    // to install. That has always ended as a success here, and
    // changing it is a separate argument from where the file gets
    // written.
    //
    // An empty file is the same event with one more syscall in it, and
    // publishing one would put a name in the user's folder that reads
    // as an episode to everything downstream — the app manufacturing
    // the very claim it now has to recover from.
    //
    // Asked of the scratch path, which the classifier reads the same
    // way it reads a target: anything but bytes in a regular file is
    // not an episode, wherever it sits.
    match at_target(&scratch.path) {
        AtTarget::Episode => {}
        // Not being able to look at the scratch path is not the same
        // as the tool having written nothing there. Reported as the
        // latter, it ends the download happily with no file published.
        AtTarget::Unreadable => return Err(AniError::Io),
        _ => return Ok(()),
    }
    // Publication decides the file's fate from here: installed under
    // the episode's name, or removed because someone else got there.
    // Both leave the scratch path empty, so the guard outliving this
    // call costs nothing — and covers the third outcome, where the
    // file could not be installed at all.
    let published = publish(&scratch.path, target).await?;
    // Either Ok consumed the scratch — unless removing it failed
    // under the publisher, which reports delivery all the same,
    // because a delivered episode is not an error. So the guard is
    // stood down for a scratch that is provably gone, not on the
    // report: anything else leaves it armed, and the drop pays one
    // more directory read to try again. Standing down here, after
    // the answer, is what keeps it armed for every failure above.
    if at_target(&scratch.path) == AtTarget::Nothing {
        scratch
            .published
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    match published {
        Published::Installed => Ok(()),
        Published::AlreadyThere => {
            on_line(ALREADY_HERE);
            Ok(())
        }
    }
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
