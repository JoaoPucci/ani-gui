//! Spawn the `ani-cli` script as a subprocess.
//!
//! All invocations:
//!
//! - clear the inherited environment except `PATH`, `HOME`, `XDG_*`,
//!   `ANI_CLI_HIST_DIR`, and a few other test-relevant overrides
//! - set `TERM=dumb`, `NO_COLOR=1` to suppress color and cursor escapes
//! - set `kill_on_drop(true)` so cancelled futures don't leak shell PIDs
//! - bound by a wall-clock timeout
//! - read stdout fully, strip ANSI, parse via [`super::parser`]
//!
//! The function signatures here are stubs — the bodies are filled in as
//! M1.2 progresses with TDD coverage.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::anicli::parser::{DebugOutput, SearchResult};
use crate::error::{AniError, Result};

/// How long any single `ani-cli` invocation may run before it is killed.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Strip characters that break ani-cli's `search_anime()` shell-string
/// JSON interpolation. Today that's just `"`: the script builds its
/// allanime curl POST via `--data "{...\"query\":\"$1\"...}"`, so a
/// literal `"` in `$1` closes the JSON string mid-way and the server
/// returns nothing (manifesting as "No results found"). Kitsu's
/// canonical title for the Naruto Shippuuden `"Konoha Gakuen"` special
/// is the repro case.
///
/// Stripping the quote is safe — allanime's fuzzy search matches
/// `Konoha Gakuen` and `"Konoha Gakuen"` to the same `_id` with the
/// same ranking, so `-S 1` lands on the right candidate either way.
///
/// Tracked upstream as a follow-up: the right fix is to JSON-escape
/// `$1` inside ani-cli's `search_anime()` (e.g. via `jq -Rs`). Once
/// that lands and we sync, this sanitiser becomes redundant.
pub(crate) fn sanitize_anicli_query(q: &str) -> String {
    q.replace('"', "")
}

/// Locate the `ani-cli` binary. Looks at `$PATH`, then falls back to a
/// path passed by the caller (typically the Tauri resource directory).
///
/// # Errors
/// Returns [`AniError::MissingBinary`] when no executable is found.
pub fn locate_ani_cli(fallback: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(found) = find_in_path("ani-cli") {
        return Ok(found);
    }
    if let Some(p) = fallback {
        if p.is_file() {
            return Ok(p.clone());
        }
    }
    Err(AniError::MissingBinary)
}

pub(crate) fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
pub(crate) fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
pub(crate) fn is_executable(p: &std::path::Path) -> bool {
    p.is_file()
        && p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("cmd"))
            .unwrap_or(false)
}

/// How `run_debug` finds the `ani-cli` script. Resolved once at startup and
/// reused per invocation.
#[derive(Debug, Clone)]
pub struct DebugOptions {
    /// Absolute path to the `ani-cli` script. Use [`locate_ani_cli`].
    pub ani_cli_path: PathBuf,
    /// Absolute path to `bash.exe` on Windows; `None` on Unix where
    /// the script runs directly via shebang. Resolved once at startup
    /// in [`AppState::build`](crate::app::AppState::build) so every
    /// spawn site uses the same bash. See [`crate::anicli::bash`].
    pub bash_path: Option<PathBuf>,
    /// Optional override for the history directory (`ANI_CLI_HIST_DIR`).
    /// Defaults to the user's `$XDG_STATE_HOME/ani-cli/` per ani-cli.
    pub hist_dir: Option<PathBuf>,
    /// Wall-clock timeout. Defaults to [`DEFAULT_TIMEOUT`].
    pub timeout: Duration,
    /// Override `PATH` (mainly for tests that put a curl shim ahead of
    /// system binaries). Defaults to the inherited `PATH`.
    pub path_override: Option<String>,
    /// Directory shipped alongside the backend binary that holds
    /// bundled POSIX deps the script needs but Git for Windows
    /// doesn't provide (today: `fzf.exe`). Prepended to the spawn's
    /// PATH so `command -v fzf` resolves to the bundled copy.
    /// `None` on Unix and on Windows dev runs without the bundled
    /// `bin/` dir in the cargo target tree.
    pub bundled_bin: Option<PathBuf>,
    /// Directory holding the provisioned `botan` wrapper, appended to
    /// the spawn's PATH so ani-cli's hard botan requirement resolves
    /// even with no system Botan — while a real installation anywhere
    /// earlier on PATH still wins. `None` when provisioning failed.
    pub shim_bin: Option<PathBuf>,
}

impl DebugOptions {
    /// Construct from a located ani-cli path with all defaults.
    #[must_use]
    pub fn new(ani_cli_path: PathBuf) -> Self {
        Self {
            ani_cli_path,
            bash_path: None,
            hist_dir: None,
            timeout: DEFAULT_TIMEOUT,
            path_override: None,
            bundled_bin: None,
            shim_bin: None,
        }
    }
}

/// Run `ani-cli` in debug-player mode and return the parsed output.
///
/// The script is invoked with `ANI_CLI_PLAYER=debug` so it prints the
/// candidate links and selected URL to stdout instead of launching a
/// player. The environment is scrubbed (only safe vars propagate),
/// `TERM=dumb` and `NO_COLOR=1` suppress ANSI noise, and `kill_on_drop`
/// is enabled so cancelled futures don't leak shell PIDs.
///
/// # Errors
/// - [`AniError::Timeout`] if the wall-clock timeout elapses
/// - [`AniError::Scraper`] for non-zero exit with a known stderr pattern
/// - [`AniError::ParseFailed`] if the debug stdout doesn't contain
///   `Selected link:` (the marker the script's debug branch emits)
/// - [`AniError::MissingBinary`] if `ani-cli` cannot be spawned
pub async fn run_debug(
    opts: &DebugOptions,
    query: &str,
    ep: &str,
    quality: &str,
    mode: &str,
    select_index: usize,
) -> Result<DebugOutput> {
    // ani-cli's `-S` flag is 1-based; the caller passes 1 to keep the
    // legacy "first match" behaviour or a higher index after running
    // its own search disambiguation (see `crate::scraper::allanime`).
    let select_str = select_index.max(1).to_string();

    let mut cmd =
        crate::anicli::bash::build_anicli_command(&opts.ani_cli_path, opts.bash_path.as_deref());
    cmd.arg("-S")
        .arg(&select_str)
        .arg("-e")
        .arg(ep)
        .arg("-q")
        .arg(quality);
    if mode == "dub" {
        cmd.arg("--dub");
    }
    // Strip embedded `"` to dodge ani-cli's search_anime JSON-injection
    // bug — see `sanitize_anicli_query` for the full story.
    let safe_query = sanitize_anicli_query(query);
    cmd.arg("--").arg(&safe_query);

    cmd.env_clear();
    // Windows: forward the OS env vars Git Bash needs to bootstrap its
    // MSYS mount table (`/tmp` resolution) and load core DLLs. Without
    // these, the first ani-cli spawn after backend startup fails with
    // `mktemp: ... '/tmp/...': Permission denied`, the script's
    // variables collapse to empty, and the user sees a "Network
    // trouble" toast for what's really a tmp-dir issue. Inert on Unix
    // because `/tmp` is always writable and there's no MSYS layer to
    // bootstrap.
    #[cfg(windows)]
    for (k, v) in crate::anicli::env::windows_env_passthrough(|k: &str| std::env::var_os(k)) {
        cmd.env(k, v);
    }
    // PATH is required so ani-cli can find curl/openssl/fzf/mpv. The
    // helper prepends `opts.bundled_bin` (Windows: bundled fzf.exe)
    // and honours `opts.path_override` (tests inject a curl shim).
    let inherited = std::env::var_os("PATH");
    cmd.env(
        "PATH",
        crate::anicli::env::append_shim_bin(
            crate::anicli::env::compose_anicli_path(
                opts.bundled_bin.as_deref(),
                opts.path_override.as_deref(),
                inherited.as_deref(),
            ),
            opts.shim_bin.as_deref(),
        ),
    );
    if let Some(home) = std::env::var_os("HOME") {
        cmd.env("HOME", home);
    }
    cmd.env("TERM", "dumb");
    cmd.env("NO_COLOR", "1");
    cmd.env("ANI_CLI_PLAYER", "debug");
    if let Some(dir) = &opts.hist_dir {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    } else if let Some(dir) = std::env::var_os("ANI_CLI_HIST_DIR") {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // Own process group + tree guard, same as spawn_download: an
    // aborted resolve must take ani-cli's in-flight curl / pipeline
    // children down too, or the click-takeover refire doubles
    // allanime traffic.
    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = TreeKillChild::new(cmd.spawn().map_err(|_| AniError::MissingBinary)?);

    let stdout_reader = child.child.stdout.take().expect("stdout piped");
    let stderr_reader = child.child.stderr.take().expect("stderr piped");

    let collected = tokio::time::timeout(opts.timeout, async move {
        let stdout_fut = read_to_end(stdout_reader);
        let stderr_fut = read_to_end(stderr_reader);
        let (out, err) = tokio::join!(stdout_fut, stderr_fut);
        let status = child.child.wait().await?;
        // Child reaped — pid/pgid may be recycled past this point.
        child.disarm();
        Result::<(Vec<u8>, Vec<u8>, std::process::ExitStatus)>::Ok((out?, err?, status))
    })
    .await
    .map_err(|_| AniError::Timeout)??;

    let (stdout_bytes, stderr_bytes, exit) = collected;

    if !exit.success() {
        let stderr_text = super::parser::strip_ansi(&stderr_bytes);
        let stdout_text = super::parser::strip_ansi(&stdout_bytes);
        tracing::error!(
            exit = ?exit.code(),
            stderr = %stderr_text,
            stdout = %stdout_text,
            "anicli: non-zero exit",
        );
        return Err(super::parser::classify_failure_stderr(&stderr_text));
    }

    let stdout_text = super::parser::strip_ansi(&stdout_bytes);
    super::parser::parse_debug_output(&stdout_text)
}

async fn read_to_end<R: tokio::io::AsyncRead + Unpin>(mut r: R) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::with_capacity(4096);
    r.read_to_end(&mut buf).await?;
    Ok(buf)
}

/// Variant of [`run_debug`] that calls `on_stderr_line` for every line
/// the script emits on stderr while it runs. Used by the SSE play
/// endpoint to forward `<provider> Links Fetched` progress to the
/// renderer in real time.
///
/// The callback receives lines **with ANSI escapes stripped**, in the
/// order they arrive. It MUST NOT block — the line reader awaits its
/// completion before pulling the next chunk from the pipe, so a slow
/// callback stalls the subprocess.
///
/// On exit, the subprocess's stdout is parsed exactly as in
/// [`run_debug`] and returned. Errors are mapped the same way.
///
/// # Errors
/// Same as [`run_debug`].
pub async fn run_debug_streaming<F>(
    opts: &DebugOptions,
    query: &str,
    ep: &str,
    quality: &str,
    mode: &str,
    select_index: usize,
    mut on_stderr_line: F,
) -> Result<super::parser::DebugOutput>
where
    F: FnMut(&str) + Send,
{
    use tokio::io::{AsyncBufReadExt, BufReader};

    let select_str = select_index.max(1).to_string();

    let mut cmd =
        crate::anicli::bash::build_anicli_command(&opts.ani_cli_path, opts.bash_path.as_deref());
    cmd.arg("-S")
        .arg(&select_str)
        .arg("-e")
        .arg(ep)
        .arg("-q")
        .arg(quality);
    if mode == "dub" {
        cmd.arg("--dub");
    }
    // Strip embedded `"` to dodge ani-cli's search_anime JSON-injection
    // bug — see `sanitize_anicli_query` for the full story.
    let safe_query = sanitize_anicli_query(query);
    cmd.arg("--").arg(&safe_query);

    cmd.env_clear();
    // See run_debug for the rationale; this is the streaming variant
    // and uses the same env-bootstrap on Windows so the first spawn
    // doesn't fall over on /tmp.
    #[cfg(windows)]
    for (k, v) in crate::anicli::env::windows_env_passthrough(|k: &str| std::env::var_os(k)) {
        cmd.env(k, v);
    }
    let inherited = std::env::var_os("PATH");
    cmd.env(
        "PATH",
        crate::anicli::env::append_shim_bin(
            crate::anicli::env::compose_anicli_path(
                opts.bundled_bin.as_deref(),
                opts.path_override.as_deref(),
                inherited.as_deref(),
            ),
            opts.shim_bin.as_deref(),
        ),
    );
    if let Some(home) = std::env::var_os("HOME") {
        cmd.env("HOME", home);
    }
    cmd.env("TERM", "dumb");
    cmd.env("NO_COLOR", "1");
    cmd.env("ANI_CLI_PLAYER", "debug");
    if let Some(dir) = &opts.hist_dir {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    } else if let Some(dir) = std::env::var_os("ANI_CLI_HIST_DIR") {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // Own process group + tree guard, same as spawn_download: the
    // click-takeover bypass aborts this future and refires — ani-cli's
    // in-flight curl / pipeline children must die with the shell or
    // the refire doubles allanime traffic.
    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = TreeKillChild::new(cmd.spawn().map_err(|_| AniError::MissingBinary)?);

    let stdout_reader = child.child.stdout.take().expect("stdout piped");
    let stderr_reader = child.child.stderr.take().expect("stderr piped");

    // Read stderr line-by-line and forward each (ANSI-stripped) line
    // to the caller. Buffer stderr bytes too so the existing
    // post-exit error handling (No results found / Episode not
    // released) keeps working.
    let stderr_collected: std::sync::Arc<std::sync::Mutex<Vec<u8>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let collected_for_reader = stderr_collected.clone();

    let stream_fut = async {
        let mut reader = BufReader::new(stderr_reader);
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = reader.read_line(&mut buf).await?;
            if read == 0 {
                break;
            }
            // Persist the raw bytes for the post-exit error check.
            {
                let mut lock = collected_for_reader.lock().expect("mutex");
                lock.extend_from_slice(buf.as_bytes());
            }
            let stripped = super::parser::strip_ansi(buf.as_bytes());
            for line in stripped.lines() {
                on_stderr_line(line);
            }
        }
        std::io::Result::Ok(())
    };

    let collected = tokio::time::timeout(opts.timeout, async move {
        let stdout_fut = read_to_end(stdout_reader);
        let (out, err_io) = tokio::join!(stdout_fut, stream_fut);
        err_io?;
        let status = child.child.wait().await?;
        // Child reaped — pid/pgid may be recycled past this point.
        child.disarm();
        Result::<(Vec<u8>, std::process::ExitStatus)>::Ok((out?, status))
    })
    .await
    .map_err(|_| AniError::Timeout)??;

    let (stdout_bytes, exit) = collected;
    let stderr_bytes = stderr_collected.lock().expect("mutex").clone();

    if !exit.success() {
        let stderr_text = super::parser::strip_ansi(&stderr_bytes);
        let stdout_text = super::parser::strip_ansi(&stdout_bytes);
        tracing::error!(
            exit = ?exit.code(),
            stderr = %stderr_text,
            stdout = %stdout_text,
            "anicli (streaming): non-zero exit",
        );
        return Err(super::parser::classify_failure_stderr(&stderr_text));
    }

    let stdout_text = super::parser::strip_ansi(&stdout_bytes);
    super::parser::parse_debug_output(&stdout_text)
}

/// Run `ani-cli` in search mode and return the parsed result list. Stub
/// pending either an upstream `--list-only` flag or migrating GUI search
/// to Kitsu metadata (the planned M2 path). See
/// `.planning/cli-contract-deviations.md` for the full rationale.
///
/// # Errors
/// Always returns `Ok(Vec::new())` until the deviation is resolved.
pub async fn run_search(_query: &str, _mode: &str) -> Result<Vec<SearchResult>> {
    let _ = tokio::task::yield_now().await;
    Ok(Vec::new())
}

/// Identifies the show + episode + variant that [`spawn_download`]
/// should hand to `ani-cli -d`. Grouped into a single value so the
/// spawn function stays under clippy's `too_many_arguments` cap and so
/// callers don't shuffle positional `&str`s in the wrong order.
#[derive(Debug, Clone)]
pub struct DownloadRequest<'a> {
    /// Title to pass after `ani-cli`'s `--` separator. Sanitized
    /// internally before reaching the shell.
    pub query: &'a str,
    /// Episode token in `ani-cli`'s `-e` syntax (e.g. `"5"`, `"3-7"`).
    pub episode: &'a str,
    /// Quality bucket (`"best"`, `"1080"`, `"720"`, `"480"`,
    /// `"worst"`).
    pub quality: &'a str,
    /// Audio variant — `"sub"` or `"dub"`. Anything other than
    /// `"dub"` runs the default sub mode.
    pub mode: &'a str,
    /// 1-based index `ani-cli` picks from its search results when
    /// the title is ambiguous. The disambiguator upstream of the
    /// download path ensures this lands on the right show.
    pub select_index: usize,
}

/// Owns the spawned ani-cli child and kills its whole process tree
/// when dropped mid-run. Ownership is the point: a struct's `Drop`
/// body runs BEFORE its fields drop, so the tree walk / group signal
/// fires while the shell is still alive — on Windows `taskkill /T`
/// can only discover curl / yt-dlp / ffmpeg descendants by a live
/// parent pid, and `kill_on_drop`'s SIGKILL (the `Child` field's own
/// drop) must come second under every cancellation mode: task abort,
/// timeout, panic. Disarmed once the child has been waited: past the
/// reap the pid may be recycled and must not be signalled. The signal
/// goes through `kill(1)` / `taskkill(1)` rather than a syscall —
/// the crate forbids unsafe code, and both binaries ship with any
/// host that can run ani-cli. `.status()` (not `.spawn()`) so the
/// helper can't linger as a zombie; it exits in microseconds.
pub(crate) struct TreeKillChild {
    child: tokio::process::Child,
    armed: bool,
}

impl TreeKillChild {
    pub(crate) fn new(child: tokio::process::Child) -> Self {
        Self { child, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    /// The guarded child, for the caller's own I/O and wait. A child
    /// the caller has waited to completion is reaped, so the drop
    /// guard reads `id() == None` and stands down by itself.
    pub(crate) fn child_mut(&mut self) -> &mut tokio::process::Child {
        &mut self.child
    }
}

impl Drop for TreeKillChild {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // `id()` is None once the child has been reaped — nothing to
        // walk in that case even if disarm was missed.
        let Some(pid) = self.child.id() else { return };
        kill_process_tree(pid);
    }
}

/// Take down a spawned downloader's whole process tree.
///
/// Shared by the drop guard and by the in-flight stop below, so both
/// paths use the same platform command and the same test seam.
fn kill_process_tree(pid: u32) {
    #[cfg(test)]
    if let Some(probe) = tree_kill_probe() {
        let _ = std::process::Command::new(probe)
            .arg(pid.to_string())
            .status();
        return;
    }
    if let Some((prog, args)) = tree_kill_args(pid, cfg!(windows)) {
        let _ = std::process::Command::new(prog).args(&args).status();
    }
}

/// Test seam: when a probe is registered, the teardown runs it (child
/// pid as its argument) INSTEAD of the real tree-kill command. The
/// Windows contract — `taskkill /T` can only discover descendants
/// while the parent is still alive — is unobservable on the platforms
/// the suite runs on, so a probe standing in for the kill command is
/// the only way a test can record WHEN the teardown fires relative to
/// the parent's reap. The probe takes over the cleanup duty too.
#[cfg(test)]
static TREE_KILL_PROBE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Serializes the probe's scope: a test that REGISTERS a probe (and
/// so redirects every teardown in the process) and a test that needs
/// the REAL teardown to run must not overlap — a no-op probe held by
/// one would silently swallow the other's kill. Held for the whole
/// test either way.
#[cfg(test)]
pub(crate) static TREE_KILL_PROBE_SCOPE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(test)]
fn tree_kill_probe() -> Option<PathBuf> {
    TREE_KILL_PROBE.lock().expect("probe lock").clone()
}

/// Platform command that takes down a spawned downloader's whole
/// process tree. Unix: `kill -9 -- -PID` — the negative pid addresses
/// the process group created at spawn (`process_group(0)`, pgid ==
/// child pid). Windows: `taskkill /PID <pid> /T /F` — no process
/// group there; /T walks the child tree by parent pid (which is why
/// the guard must fire while the shell is still alive), /F because
/// the transfer tools ignore the graceful signal mid-write.
fn tree_kill_args(pid: u32, windows: bool) -> Option<(&'static str, Vec<String>)> {
    if windows {
        return Some((
            "taskkill",
            vec!["/PID".into(), pid.to_string(), "/T".into(), "/F".into()],
        ));
    }
    Some(("kill", vec!["-9".into(), "--".into(), format!("-{pid}")]))
}

/// Write the classified script bytes to a private temp file, immune
/// to the installer renaming a new script over the live path
/// mid-flight. The snapshot deliberately carries NO exec permission:
/// it is READ by the shell interpreter, never exec(2)'d, so a
/// `noexec` temp mount cannot break it. The returned `TempDir` owns
/// the snapshot for the download's lifetime.
fn stage_script_snapshot(contents: &str, live_path: &Path) -> std::io::Result<StagedScript> {
    // Beside the live script first: a script that loads a sibling
    // relative to itself resolves `$(dirname "$0")` to the same
    // directory it would have under a direct execution. The name is
    // dotted and pid-tagged so it cannot collide with the script
    // itself or with a concurrent download.
    if let Some(dir) = live_path.parent() {
        // `tempfile` picks the name and creates it O_EXCL, so two
        // concurrent downloads cannot land on the same path — a pid
        // is shared by every download in this process.
        let staged = tempfile::Builder::new()
            .prefix(".ani-cli-snapshot-")
            .tempfile_in(dir)
            .and_then(|mut f| {
                use std::io::Write;
                f.as_file_mut().write_all(contents.as_bytes())?;
                Ok(f)
            });
        if let Ok(file) = staged {
            return Ok(StagedScript::BesideLive(file));
        }
    }
    // A packaged install can ship its script in a read-only
    // directory. Degrade rather than fail the download: the temp copy
    // loses `$0`-relative resource loading, which is what shipped
    // before staging was directory-aware.
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("ani-cli");
    std::fs::write(&path, contents)?;
    Ok(StagedScript::Temp { _dir: dir, path })
}

/// Owns the staged snapshot for the download's lifetime. A temp-dir
/// copy is cleaned up by the `TempDir`; a copy written beside the
/// live script has to be removed explicitly, since that directory
/// outlives the download.
enum StagedScript {
    /// Beside the live script, so `$0`'s directory is preserved. The
    /// `NamedTempFile` owns the unique name and unlinks it on drop,
    /// which keeps one download's cleanup off another's snapshot.
    BesideLive(tempfile::NamedTempFile),
    Temp {
        /// Held only for its `Drop`, which removes the directory.
        _dir: tempfile::TempDir,
        path: PathBuf,
    },
}

impl StagedScript {
    fn path(&self) -> &Path {
        match self {
            Self::BesideLive(f) => f.path(),
            Self::Temp { path, .. } => path,
        }
    }
}

/// The interpreter argv a snapshot must run under, taken from its own
/// shebang. Because the snapshot is READ rather than exec(2)'d, the
/// kernel never applies the shebang — so a script declaring `bash`
/// and using bash-only syntax would break under a hard-coded
/// `/bin/sh`, and break on downloads only: search and playback route
/// through the ordinary command builder and keep working.
///
/// Only an absolute interpreter path is honored. A relative or bare
/// name (`#!bash`) falls back to `/bin/sh` rather than being resolved
/// through PATH or the working directory — that lookup would be
/// driven by a string in a file the auto-updater rewrites, which is a
/// worse outcome than running a `sh`-compatible script under `sh`.
/// The tail is kept so the `#!/usr/bin/env bash` form works, and kept
/// WHOLE: Linux's `fs/binfmt_script.c` takes the first blank-delimited
/// word as the interpreter and passes everything after it as a single
/// argument, doing no tokenizing of its own. Splitting it here would
/// break `#!/usr/bin/env -S bash -O 'extglob'`, where `env` is the one
/// that splits the string and strips the quotes.
fn snapshot_interpreter(contents: &str) -> Vec<String> {
    const DEFAULT: &str = "/bin/sh";
    const BLANK: [char; 2] = [' ', '\t'];
    let fallback = || vec![DEFAULT.to_string()];
    let Some(shebang) = contents.lines().next().and_then(|l| l.strip_prefix("#!")) else {
        return fallback();
    };
    let line = shebang.trim_matches(BLANK);
    let (path, tail) = match line.find(BLANK) {
        Some(i) => (&line[..i], line[i..].trim_start_matches(BLANK)),
        None => (line, ""),
    };
    if !path.starts_with('/') {
        return fallback();
    }
    let mut argv = vec![path.to_string()];
    if !tail.is_empty() {
        argv.push(tail.to_string());
    }
    argv
}

/// Build the download spawn. A staged snapshot runs THROUGH its
/// declared interpreter so a `noexec` temp mount can't refuse it
/// (the file carries no exec bit and is never exec(2)'d); the
/// live-path fallback and Windows keep the ordinary command builder,
/// which already routes through bash.
fn download_command(
    spawn_path: &Path,
    bash_path: Option<&Path>,
    interpreter: Option<&[String]>,
) -> tokio::process::Command {
    match interpreter {
        Some([program, args @ ..]) => {
            let mut cmd = tokio::process::Command::new(program);
            cmd.args(args).arg(spawn_path);
            cmd
        }
        _ => crate::anicli::bash::build_anicli_command(spawn_path, bash_path),
    }
}

/// Spawn `ani-cli -d` to download an episode. The `-d` flag flips the
/// script's player-function to `download`, which delegates to yt-dlp /
/// ffmpeg / aria2c depending on the source kind. We point `ani-cli` at
/// the user-chosen output directory via `ANI_CLI_DOWNLOAD_DIR` (which
/// the script reads at line 468 of the upstream).
///
/// Like [`run_debug_streaming`], stderr lines are forwarded to the
/// caller as they arrive — that's where aria2c / yt-dlp / ffmpeg write
/// progress, and the SSE download endpoint relays them to the renderer.
///
/// # Errors
/// - [`AniError::MissingBinary`] if the script can't be spawned.
/// - [`AniError::Scraper`] / [`AniError::NoResults`] /
///   [`AniError::Timeout`] mirror the [`run_debug`] error mapping.
pub async fn spawn_download<F>(
    opts: &DebugOptions,
    req: &DownloadRequest<'_>,
    download_dir: &Path,
    mut on_stderr_line: F,
) -> Result<()>
where
    F: FnMut(&str) + Send,
{
    use tokio::io::{AsyncBufReadExt, BufReader};

    let select_str = req.select_index.max(1).to_string();

    // Read the active script ONCE and execute a snapshot of those
    // exact bytes: the auto-updater may atomically rename a new
    // script over the install path between classification and
    // spawn, and the preflight verdict must describe the script
    // that actually runs. An unreadable script degrades to the
    // live path with the conservative ffmpeg-only classification.
    let script_contents = std::fs::read_to_string(&opts.ani_cli_path).ok();
    let snapshot = script_contents
        .as_deref()
        .and_then(|contents| stage_script_snapshot(contents, &opts.ani_cli_path).ok());
    let spawn_path = snapshot
        .as_ref()
        .map_or(opts.ani_cli_path.clone(), |s| s.path().to_path_buf());

    // Only a snapshot needs the explicit interpreter — the live path
    // still exec(2)'s normally, where the kernel reads the shebang
    // itself. Windows has no shebang semantics, so it keeps the bash
    // builder either way.
    let interpreter: Option<Vec<String>> = if cfg!(unix) && snapshot.is_some() {
        script_contents.as_deref().map(snapshot_interpreter)
    } else {
        None
    };
    let mut cmd = download_command(
        &spawn_path,
        opts.bash_path.as_deref(),
        interpreter.as_deref(),
    );
    cmd.arg("-S")
        .arg(&select_str)
        .arg("-d")
        .arg("-e")
        .arg(req.episode)
        .arg("-q")
        .arg(req.quality);
    if req.mode == "dub" {
        cmd.arg("--dub");
    }
    let safe_query = sanitize_anicli_query(req.query);
    cmd.arg("--").arg(&safe_query);

    cmd.env_clear();
    // See run_debug for rationale; same Windows env-bootstrap on the
    // download path so aria2c / ffmpeg / ani-cli all see the OS env
    // they need to find /tmp and load DLLs.
    #[cfg(windows)]
    for (k, v) in crate::anicli::env::windows_env_passthrough(|k: &str| std::env::var_os(k)) {
        cmd.env(k, v);
    }
    let inherited = std::env::var_os("PATH");
    let composed_path = crate::anicli::env::append_shim_bin(
        crate::anicli::env::compose_anicli_path(
            opts.bundled_bin.as_deref(),
            opts.path_override.as_deref(),
            inherited.as_deref(),
        ),
        opts.shim_bin.as_deref(),
    );
    // Pre-spawn check: the script's download dep check exits the
    // shell instantly when its tools are missing, and the post-exit
    // error mapping below would collapse that into a generic
    // Scraper error. Catch it up front so the SSE stream's first
    // frame is the typed FfmpegMissing the layout can render a
    // clear modal for. Which tools count is decided by the *active*
    // script's own dep line — a stale pre-4.15 cache (auto-update
    // disabled, failing, or not finished) still hard-requires
    // ffmpeg, so yt-dlp alone must not pass against it. An
    // unreadable script degrades to ffmpeg-only, the conservative
    // direction. aria2c is bundled (commit d6c9992), so a missing
    // aria2c falls through and is mapped post-exit below.
    let tool_names =
        crate::anicli::env::download_tool_names(script_contents.as_deref().unwrap_or(""));
    crate::anicli::env::ensure_download_tool_in_path(tool_names, &composed_path, is_executable)?;
    cmd.env("PATH", composed_path);
    if let Some(home) = std::env::var_os("HOME") {
        cmd.env("HOME", home);
    }
    cmd.env("TERM", "dumb");
    cmd.env("NO_COLOR", "1");
    // The whole point of this function: tell ani-cli where to drop the
    // downloaded mp4. Upstream reads the env at line 468.
    cmd.env("ANI_CLI_DOWNLOAD_DIR", download_dir);
    if let Some(dir) = &opts.hist_dir {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    } else if let Some(dir) = std::env::var_os("ANI_CLI_HIST_DIR") {
        cmd.env("ANI_CLI_HIST_DIR", dir);
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // Own process group (pgid == child pid): ani-cli fronts yt-dlp /
    // ffmpeg / aria2c as foreground children, and kill_on_drop only
    // signals the shell itself — an aborted download would orphan the
    // transfer tool mid-write. The group guard below reaps the whole
    // tree instead.
    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = TreeKillChild::new(cmd.spawn().map_err(|_| AniError::MissingBinary)?);
    let stderr_reader = child.child.stderr.take().expect("stderr piped");

    // Stream stderr line-by-line. aria2c / yt-dlp / ffmpeg all write
    // their progress to stderr. Buffer the raw bytes too so the
    // post-exit error mapping can match against the same patterns
    // run_debug uses.
    let stderr_collected: std::sync::Arc<std::sync::Mutex<Vec<u8>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let collected_for_reader = stderr_collected.clone();

    // Acted on while the script is still running, not after it exits.
    // A range download is one process looping over episodes, so a
    // warning on episode 1 means every later episode of that range
    // takes the same doomed path — the whole season would come down
    // as mislabeled MPEG-TS before anything looked at the line.
    let repackage_failed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let failed_for_reader = repackage_failed.clone();
    let child_pid = child.child.id();

    let stream_fut = async {
        let mut reader = BufReader::new(stderr_reader);
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = reader.read_line(&mut buf).await?;
            if read == 0 {
                break;
            }
            {
                let mut lock = collected_for_reader.lock().expect("mutex");
                lock.extend_from_slice(buf.as_bytes());
            }
            let stripped = super::parser::strip_ansi(buf.as_bytes());
            for line in stripped.lines() {
                on_stderr_line(line);
            }
            if !failed_for_reader.load(std::sync::atomic::Ordering::Relaxed)
                && yt_dlp_could_not_repackage(&stripped)
            {
                failed_for_reader.store(true, std::sync::atomic::Ordering::Relaxed);
                // Stop the tree, not just the shell: the transfer tool
                // is a child of it and would keep writing.
                if let Some(pid) = child_pid {
                    kill_process_tree(pid);
                }
            }
        }
        Ok::<(), std::io::Error>(())
    };

    // Stdout has to be drained anyway or the pipe backs up, and it
    // carries the one thing that identifies this download's output:
    // yt-dlp's `[download] Destination:` announcements. Collect them
    // as they stream so cleanup below has an exact manifest instead
    // of a guess about which files in a shared directory are ours.
    let announced: std::sync::Arc<std::sync::Mutex<Vec<PathBuf>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let announced_for_reader = announced.clone();
    let stdout_reader = child.child.stdout.take().expect("stdout piped");
    let drain_stdout = async move {
        let mut reader = BufReader::new(stdout_reader);
        let mut sink = String::new();
        loop {
            sink.clear();
            if reader.read_line(&mut sink).await? == 0 {
                break;
            }
            let stripped = super::parser::strip_ansi(sink.as_bytes());
            for path in stripped.lines().filter_map(yt_dlp_destination) {
                announced_for_reader.lock().expect("mutex").push(path);
            }
        }
        Ok::<(), std::io::Error>(())
    };

    let outcome = tokio::time::timeout(opts.timeout, async {
        let (a, b) = tokio::join!(stream_fut, drain_stdout);
        a?;
        b?;
        let status = child.child.wait().await?;
        Result::<std::process::ExitStatus>::Ok(status)
    })
    .await;

    // Once the warning has been read, how the run ended stops
    // mattering. Exit 0 is not the same as a usable file: yt-dlp
    // writes the fragments it downloaded even when it cannot
    // repackage them into the .mp4 container the name promises, and
    // reports that only on stderr. The result is raw MPEG-TS wearing
    // an .mp4 extension — it plays in mpv and VLC, which sniff
    // content, and fails anywhere that trusts the extension.
    //
    // Nonzero is the same condition seen from the other side, because
    // the stop issued above ends the run by signal. So is running out
    // of clock, which is how the old post-exit placement stranded the
    // file: it returned Timeout before any cleanup could run. All
    // three are one answer — ffmpeg was needed and absent — and all
    // three leave the same file to clear up.
    //
    // Checking the tool's own report rather than the provider's
    // format is deliberate. What allanime serves can change under us,
    // and a decision made once against today's answer would go
    // silently wrong the day it does.
    if repackage_failed.load(std::sync::atomic::Ordering::Relaxed) {
        // The payload is already on disk under an .mp4 name. Failing
        // the download and leaving it there is the worst of both: told
        // it failed, still finds something that looks like the episode.
        let announced = announced.lock().expect("mutex").clone();
        discard_mislabeled(download_dir, &announced);
        return Err(AniError::FfmpegMissing);
    }

    let collected = outcome.map_err(|_| AniError::Timeout)??;
    // The child has been reaped — its pid (and so the pgid) may be
    // recycled; the tree must not be signalled past this point.
    child.disarm();

    if !collected.success() {
        let stderr_bytes = stderr_collected.lock().expect("mutex").clone();
        let stderr_text = super::parser::strip_ansi(&stderr_bytes);
        if stderr_text.contains("No results found") {
            return Err(AniError::NoResults);
        }
        // Defense in depth — the pre-spawn ensure_ffmpeg_in_path
        // catches the common case before ani-cli runs, but if a
        // user somehow has ffmpeg on PATH that isn't a real ffmpeg
        // (corrupted, wrong arch), upstream's dep_ch fires this
        // exact line and we still want the typed error so the
        // layout can render the install modal. ani-cli's dep_ch
        // formats the message via die: `Program "ffmpeg" not found.`.
        if stderr_text.contains("Program \"ffmpeg\" not found") {
            return Err(AniError::FfmpegMissing);
        }
        return Err(AniError::Scraper {
            key: crate::i18n::keys::SCRAPER_PARSE_FAILED,
        });
    }
    Ok(())
}

/// Remove the files this download announced that are not, in fact,
/// MP4s.
///
/// Two independent bounds, so a good file cannot be eaten by either
/// alone: the path must be one yt-dlp named as its own output for
/// this run *and* sit directly in the directory we handed it, and its
/// own first byte must be the MPEG-TS sync byte `0x47`. A real MP4
/// opens with a length-prefixed `ftyp` box and fails the second test
/// even when it is ours; a concurrent download's file was never
/// announced and fails the first, whatever state its bytes are in.
///
/// No announcement means no deletion. That is the right way to fail
/// here — a mislabeled file left behind is recoverable, someone
/// else's deleted mid-transfer is not.
fn discard_mislabeled(dir: &Path, announced: &[PathBuf]) {
    for path in announced {
        if path.parent() != Some(dir) {
            continue;
        }
        let mut buf = [0u8; 1];
        let is_ts = std::fs::File::open(path)
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut buf))
            .is_ok_and(|()| buf[0] == 0x47);
        if is_ts {
            // Best effort: a file we cannot remove is not worth
            // failing the already-failing download over.
            let _ = std::fs::remove_file(path);
        }
    }
}

/// yt-dlp's own report that it left MPEG-TS inside an `.mp4`.
///
/// Anchored to the warning line, not to the stream. yt-dlp emits it
/// through `report_warning`, which prefixes `WARNING:` — measured
/// against 2025.08.11 the line reads:
///
/// ```text
/// WARNING: out: Possible MPEG-TS in MP4 container or malformed
/// AAC timestamps. Install ffmpeg to fix this automatically
/// ```
///
/// Everywhere else in the output the same words would be someone's
/// show title, echoed back by the destination and progress lines. A
/// title is whatever the provider called the entry, so searching the
/// whole stream lets a name decide whether downloads succeed.
///
/// Within the line the match stays on the stable middle: the leading
/// id and the trailing advice are respectively arbitrary and
/// reworded across releases, and pinning either would turn a future
/// yt-dlp into a silent regression.
fn yt_dlp_could_not_repackage(stderr: &str) -> bool {
    stderr.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("WARNING:") && line.contains("Possible MPEG-TS in MP4 container")
    })
}

/// The path yt-dlp announces for the file it is about to write, e.g.
///
/// ```text
/// [download] Destination: /home/u/Anime/Show - 01.mp4
/// ```
///
/// This is the only statement in the output about what belongs to
/// this download, which makes it the manifest cleanup is allowed to
/// act on.
fn yt_dlp_destination(line: &str) -> Option<PathBuf> {
    line.trim()
        .strip_prefix("[download] Destination: ")
        .map(|rest| PathBuf::from(rest.trim_end()))
}

#[cfg(test)]
#[path = "process_test.rs"]
mod tests;
