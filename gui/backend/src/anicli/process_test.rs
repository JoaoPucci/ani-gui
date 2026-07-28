//! Tests for `crate::anicli::process`. Extracted via `#[path]` so the
//! inline `mod tests { ... }` block doesn't count toward the file's
//! CCN — per `project_crap_inline_test_gotcha`.

use super::*;

/// Serializes tests that mutate process-global env (PATH) with
/// tests that fork subprocesses (whose runtime resolves PATH at
/// spawn time on some kernels). Without this lock the suite flaked
/// at ~40% under `cargo test`'s default parallelism. Tokio mutex
/// because the guard crosses `.await` points.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn locate_ani_cli_with_no_path_and_no_fallback_errors() {
    let _guard = ENV_LOCK.lock().await;
    // Save and clear $PATH so `which` cannot find ani-cli.
    let saved = std::env::var_os("PATH");
    // Use unsafe-free API: the std::env::set_var on stable is safe. The
    // test mutates process global state; the lock above keeps
    // subprocess-spawning tests out while PATH is empty.
    std::env::set_var("PATH", "");
    let r = locate_ani_cli(None);
    if let Some(p) = saved {
        std::env::set_var("PATH", p);
    }
    assert!(matches!(r, Err(AniError::MissingBinary)));
}

/// Build a stub `ani-cli` script that emits `stderr_msg` and exits
/// with `code`. Returned tempdir keeps the file alive for the test.
#[cfg(unix)]
fn stub_ani_cli(stderr_msg: &str, code: i32) -> (tempfile::TempDir, PathBuf) {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("ani-cli");
    let mut f = std::fs::File::create(&path).expect("create stub");
    // POSIX sh: forward stderr_msg to stderr, exit with the requested
    // code. Quoting `stderr_msg` is safe because we only ever pass
    // hard-coded fixture strings here.
    writeln!(f, "#!/bin/sh\necho \"{stderr_msg}\" 1>&2\nexit {code}").expect("write stub");
    let mut perm = f.metadata().expect("perm").permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&path, perm).expect("chmod");
    (td, path)
}

#[cfg(unix)]
fn debug_opts(path: PathBuf) -> DebugOptions {
    let mut opts = DebugOptions::new(path);
    // Pin PATH so the parallel `locate_ani_cli_*` test (which
    // temporarily empties $PATH) can't race-clear our subprocess's
    // PATH and turn the spawn into MissingBinary.
    opts.path_override = Some("/usr/bin:/bin".into());
    opts
}

/// Cover the three exit-classification branches in `run_debug`'s
/// non-zero path: "No results found" → typed NoResults; "Episode
/// not released" → keyed Scraper; any other stderr → catch-all
/// Scraper. Bundled into one test so the parallel
/// `locate_ani_cli_*` test can't race-clear $PATH between sub-
/// cases and turn a spawn into MissingBinary.
///
/// Unix-only because the stub ani-cli script needs shebang
/// interpretation; the classification logic itself is platform-
/// neutral and exercised on Windows via unit-level parser tests.
#[cfg(unix)]
#[tokio::test]
async fn run_debug_classifies_nonzero_exits_by_stderr_pattern() {
    let _guard = ENV_LOCK.lock().await;
    let (_td1, p1) = stub_ani_cli("No results found", 1);
    let r1 = run_debug(&debug_opts(p1), "any", "1", "best", "sub", 1).await;
    assert!(matches!(r1, Err(AniError::NoResults)), "got: {r1:?}");

    let (_td2, p2) = stub_ani_cli("Episode not released", 1);
    let r2 = run_debug(&debug_opts(p2), "any", "999", "best", "sub", 1).await;
    assert!(matches!(r2, Err(AniError::Scraper { .. })), "got: {r2:?}");

    let (_td3, p3) = stub_ani_cli("could not resolve host", 6);
    let r3 = run_debug(&debug_opts(p3), "any", "1", "best", "sub", 1).await;
    assert!(matches!(r3, Err(AniError::Scraper { .. })), "got: {r3:?}");
}

/// Same exit-classification logic in the streaming variant — covers
/// the SSE play endpoint's error paths.
#[cfg(unix)]
#[tokio::test]
async fn run_debug_streaming_classifies_nonzero_exits_by_stderr_pattern() {
    let _guard = ENV_LOCK.lock().await;
    let (_td1, p1) = stub_ani_cli("No results found", 1);
    let r1 = run_debug_streaming(&debug_opts(p1), "any", "1", "best", "sub", 1, |_| {}).await;
    assert!(matches!(r1, Err(AniError::NoResults)), "got: {r1:?}");

    let (_td2, p2) = stub_ani_cli("Episode not released", 1);
    let r2 = run_debug_streaming(&debug_opts(p2), "any", "1", "best", "sub", 1, |_| {}).await;
    assert!(matches!(r2, Err(AniError::Scraper { .. })), "got: {r2:?}");
}

/// ani-cli's `search_anime()` builds its allanime curl POST via
/// shell-string interpolation: `--data "{...\"query\":\"$1\"...}"`.
/// A literal `"` in the title closes the JSON string mid-way and
/// the server returns nothing (manifesting as "No results found").
/// Kitsu's canonical title for Naruto Shippuuden's "Konoha Gakuen"
/// special is the repro case. Our backend strips embedded quotes
/// before handing the title to ani-cli; allanime's fuzzy search
/// matches the de-quoted query to the same `_id` with the same
/// ranking, so `-S 1` still lands on the right candidate.
#[test]
fn sanitize_anicli_query_strips_embedded_double_quotes() {
    let q = r#"Naruto Shippuuden: Shippuu! "Konoha Gakuen" Den"#;
    assert_eq!(
        sanitize_anicli_query(q),
        "Naruto Shippuuden: Shippuu! Konoha Gakuen Den",
    );
}

#[test]
fn sanitize_anicli_query_passes_quote_free_titles_through() {
    assert_eq!(sanitize_anicli_query("One Piece"), "One Piece");
    assert_eq!(sanitize_anicli_query(""), "");
}

/// Stub `ani-cli` that echoes each argv token + selected env vars to
/// stderr (so the streaming line callback captures them) and exits
/// 0. Also stages a no-op `ffmpeg` next to it so spawn_download's
/// pre-spawn `ensure_ffmpeg_in_path` finds something on PATH —
/// without it the test fails on CI runners that don't have ffmpeg
/// installed (macos), even though the stub ani-cli never actually
/// invokes ffmpeg. The stub dir is meant to be prepended to
/// `path_override` so the pre-check sees the fake binary first.
#[cfg(unix)]
fn stub_ani_cli_echo() -> (tempfile::TempDir, PathBuf) {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("ani-cli");
    let mut f = std::fs::File::create(&path).expect("create stub");
    // POSIX sh: walk $@ emitting one `argv:<token>` line per arg,
    // then echo the env var the download path relies on.
    f.write_all(
        b"#!/bin/sh\nfor a in \"$@\"; do printf 'argv:%s\\n' \"$a\" 1>&2; done\nprintf 'env:ANI_CLI_DOWNLOAD_DIR=%s\\n' \"${ANI_CLI_DOWNLOAD_DIR:-NOTSET}\" 1>&2\nexit 0\n",
    )
    .expect("write stub");
    let mut perm = f.metadata().expect("perm").permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&path, perm).expect("chmod");

    // No-op ffmpeg: needs to exist + be executable so the pre-check
    // passes. Never actually invoked — the stub ani-cli exits 0
    // without spawning anything.
    let ffmpeg = td.path().join("ffmpeg");
    std::fs::write(&ffmpeg, b"#!/bin/sh\nexit 0\n").expect("write ffmpeg stub");
    std::fs::set_permissions(&ffmpeg, std::fs::Permissions::from_mode(0o755))
        .expect("chmod ffmpeg");

    (td, path)
}

/// `debug_opts` for download tests — like `debug_opts` but prepends
/// the stub dir (which holds the no-op ffmpeg from
/// `stub_ani_cli_echo`) to `path_override` so the pre-spawn check
/// finds it. The dir is the parent of the stub ani-cli path.
#[cfg(unix)]
fn debug_opts_dl(stub_ani_cli: PathBuf) -> DebugOptions {
    let stub_dir = stub_ani_cli
        .parent()
        .expect("stub has parent dir")
        .to_path_buf();
    let mut opts = DebugOptions::new(stub_ani_cli);
    opts.path_override = Some(format!("{}:/usr/bin:/bin", stub_dir.display()));
    opts
}

#[test]
fn tree_kill_args_unix_addresses_the_process_group() {
    let (prog, args) = tree_kill_args(1234, false).expect("unix tree kill");
    assert_eq!(prog, "kill");
    assert_eq!(args, vec!["-9", "--", "-1234"]);
}

#[test]
fn tree_kill_args_windows_kills_the_tree_by_parent_pid() {
    // Windows is a shipped target (package:win) and kill_on_drop
    // only terminates the Git Bash parent there — cancelling a
    // download must take aria2c / ffmpeg / yt-dlp down with it.
    // taskkill /T walks the child tree by parent pid; /F because
    // the transfer tools ignore the graceful signal mid-write.
    let (prog, args) = tree_kill_args(1234, true).expect("windows tree kill");
    assert_eq!(prog, "taskkill");
    assert_eq!(args, vec!["/PID", "1234", "/T", "/F"]);
}

proptest::proptest! {
    // Contract for any pid on both platforms: the command always
    // names the pid (negated group on unix, bare tree root on
    // windows) and never comes back empty — every supported
    // platform has a tree kill.
    #[test]
    fn tree_kill_args_always_names_the_pid(
        pid in proptest::num::u32::ANY,
        windows in proptest::bool::ANY,
    ) {
        let (prog, args) = tree_kill_args(pid, windows).expect("tree kill exists");
        let want = if windows { pid.to_string() } else { format!("-{pid}") };
        let named = args.iter().any(|a| a == &want);
        proptest::prop_assert!(named, "{} args {:?} missing {}", prog, args, want);
    }
}

/// Stub ani-cli that mirrors the download path's process shape:
/// it launches a long-sleeping descendant (the "yt-dlp"), reports
/// that descendant's pid via a file, and waits on it — like
/// ani-cli fronting a transfer tool.
#[cfg(unix)]
fn stub_ani_cli_with_descendant(pidfile: &std::path::Path) -> (tempfile::TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("ani-cli");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nsleep 30 &\necho $! > '{}'\nwait\n",
            pidfile.display()
        ),
    )
    .expect("write stub");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let ffmpeg = td.path().join("ffmpeg");
    std::fs::write(&ffmpeg, b"#!/bin/sh\nexit 0\n").expect("write ffmpeg stub");
    std::fs::set_permissions(&ffmpeg, std::fs::Permissions::from_mode(0o755))
        .expect("chmod ffmpeg");
    (td, path)
}

#[cfg(unix)]
#[tokio::test]
async fn tree_kill_fires_while_the_parent_is_still_alive() {
    let _guard = ENV_LOCK.lock().await;
    // The Windows contract: taskkill /T discovers descendants by
    // a LIVE parent pid, so the teardown must run before
    // kill_on_drop reaps the shell. Only observable through the
    // probe seam — it stands in for the tree-kill command and
    // records the parent's state (running vs zombie/reaped) at
    // the exact moment the teardown fires.
    let probe_td = tempfile::tempdir().expect("probe tempdir");
    let out = probe_td.path().join("probe.out");
    let probe = probe_td.path().join("probe.sh");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(
            &probe,
            format!(
                "#!/bin/sh\nsleep 0.1\nSTATE=$(ps -o state= -p \"$1\" 2>/dev/null | tr -d ' ')\ncase \"$STATE\" in\n  ''|Z*) echo dead > '{out}' ;;\n  *) echo alive > '{out}' ;;\nesac\nkill -9 -- \"-$1\" 2>/dev/null || true\nexit 0\n",
                out = out.display()
            ),
        )
        .expect("write probe");
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755))
            .expect("chmod probe");
    }
    *TREE_KILL_PROBE.lock().expect("probe lock") = Some(probe);

    let pid_td = tempfile::tempdir().expect("pid tempdir");
    let pidfile = pid_td.path().join("descendant.pid");
    let (_td, stub) = stub_ani_cli_with_descendant(&pidfile);
    let opts = debug_opts(stub);
    let task = tokio::spawn(async move {
        let _ = run_debug_streaming(&opts, "any", "1", "best", "sub", 1, |_| {}).await;
    });

    // Wait for the run to be mid-flight, then abort it.
    {
        let mut waited_ms = 0u32;
        while !pidfile.exists() {
            assert!(waited_ms < 5_000, "descendant never reported its pid");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            waited_ms += 50;
        }
    }
    task.abort();
    let _ = task.await;

    let verdict = {
        let mut waited_ms = 0u32;
        loop {
            if let Ok(s) = std::fs::read_to_string(&out) {
                if !s.trim().is_empty() {
                    break s.trim().to_string();
                }
            }
            assert!(waited_ms < 5_000, "probe never ran");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            waited_ms += 50;
        }
    };
    *TREE_KILL_PROBE.lock().expect("probe lock") = None;
    assert_eq!(
        verdict, "alive",
        "the tree kill must fire while the parent is still alive — a reaped parent \
         hides its descendants from taskkill /T on Windows"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn aborting_a_play_resolution_kills_the_whole_process_tree() {
    let _guard = ENV_LOCK.lock().await;
    // The click-takeover bypass aborts a started background
    // resolve and immediately refires interactively. kill_on_drop
    // reaps the ani-cli shell alone — its in-flight curl /
    // pipeline children must die with it, or the takeover doubles
    // allanime traffic at exactly the moment it exists to avoid.
    let pid_td = tempfile::tempdir().expect("pid tempdir");
    let pidfile = pid_td.path().join("descendant.pid");
    let (_td, stub) = stub_ani_cli_with_descendant(&pidfile);
    let opts = debug_opts(stub);

    let task = tokio::spawn(async move {
        let _ = run_debug_streaming(&opts, "any", "1", "best", "sub", 1, |_| {}).await;
    });

    let pid: i32 = {
        let mut waited_ms = 0u32;
        loop {
            if let Ok(s) = std::fs::read_to_string(&pidfile) {
                if let Ok(p) = s.trim().parse() {
                    break p;
                }
            }
            assert!(waited_ms < 5_000, "descendant never reported its pid");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            waited_ms += 50;
        }
    };

    task.abort();
    let _ = task.await;

    let mut dead = false;
    for _ in 0..40 {
        if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
            dead = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    if !dead {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }
    assert!(
        dead,
        "descendant (pid {pid}) survived the play abort — duplicate allanime traffic"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn aborting_a_download_kills_the_whole_process_tree() {
    let _guard = ENV_LOCK.lock().await;
    // Cancelling a download aborts the SSE task, which drops the
    // spawn_download future. kill_on_drop reaps the ani-cli shell
    // — but the transfer tool it fronted must die WITH it, or the
    // dock reports "cancelled" while an orphaned downloader keeps
    // writing the file and burning bandwidth.
    let pid_td = tempfile::tempdir().expect("pid tempdir");
    let pidfile = pid_td.path().join("descendant.pid");
    let (_td, stub) = stub_ani_cli_with_descendant(&pidfile);
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    let opts = debug_opts_dl(stub);

    let task = tokio::spawn(async move {
        let _ = spawn_download(
            &opts,
            &DownloadRequest {
                query: "any",
                episode: "1",
                quality: "best",
                mode: "sub",
                select_index: 1,
            },
            dl_dir.path(),
            |_| {},
        )
        .await;
    });

    // Wait for the stub to report its descendant's pid.
    let pid: i32 = {
        let mut waited_ms = 0u32;
        loop {
            if let Ok(s) = std::fs::read_to_string(&pidfile) {
                if let Ok(p) = s.trim().parse() {
                    break p;
                }
            }
            assert!(waited_ms < 5_000, "descendant never reported its pid");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            waited_ms += 50;
        }
    };

    task.abort();
    let _ = task.await;

    // The descendant must be gone shortly after the abort. Poll
    // /proc — a surviving entry means the downloader was orphaned.
    let mut dead = false;
    for _ in 0..40 {
        if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
            dead = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    if !dead {
        // Don't leak the 30s sleeper into the test host on failure.
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }
    assert!(
        dead,
        "descendant (pid {pid}) survived the abort — orphaned downloader"
    );
}

/// Stub script + tool set for the capability-gate acceptance
/// tests: the script optionally carries the 4.15 failover marker,
/// and only the named tools exist beside it. PATH is confined to
/// the stub dir so the host's real ffmpeg can't leak in.
#[cfg(unix)]
fn stub_ani_cli_with_tools(marker: bool, tools: &[&str]) -> (tempfile::TempDir, DebugOptions) {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("ani-cli");
    let mut f = std::fs::File::create(&path).expect("create stub");
    // The capable form is the release's download arm verbatim —
    // the classifier accepts nothing less, because an excerpt of
    // the arm is also what a usage block quoting it looks like.
    // `$player_function` is unset, so the arm never executes and
    // the stub's undefined functions are never called.
    let arm_body = if marker {
        "        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'\n        dep_ch \"aria2c\"\n"
    } else {
        "        dep_ch \"ffmpeg\" \"aria2c\" >/dev/null 2>&1 || true\n"
    };
    let script = format!(
        "#!/bin/sh\nversion_number=\"4.15.0\"\ncase \"$player_function\" in\n    download)\n{arm_body}        ;;\nesac\nexit 0\n"
    );
    f.write_all(script.as_bytes()).expect("write stub");
    let mut perm = f.metadata().expect("perm").permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&path, perm).expect("chmod");
    for tool in tools {
        let t = td.path().join(tool);
        std::fs::write(&t, b"#!/bin/sh\nexit 0\n").expect("write tool stub");
        std::fs::set_permissions(&t, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let mut opts = DebugOptions::new(path);
    opts.path_override = Some(td.path().display().to_string());
    (td, opts)
}

/// The auto-updater may rename a new script over the install
/// path between classification and spawn. Executing a snapshot
/// of the exact bytes classified closes that window; the
/// observable guarantee is that the spawned $0 is the snapshot,
/// not the live install path.
#[cfg(unix)]
#[tokio::test]
async fn spawn_download_executes_a_snapshot_of_the_classified_script() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let _guard = ENV_LOCK.lock().await;
    let td = tempfile::tempdir().expect("tempdir");
    let argv0_out = td.path().join("argv0");
    let path = td.path().join("ani-cli");
    let mut f = std::fs::File::create(&path).expect("create stub");
    let body = format!(
        "#!/bin/sh\nversion_number=\"4.15.0\"\ncase \"$player_function\" in\n    download)\n        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'\n        dep_ch \"aria2c\"\n        ;;\nesac\nprintf '%s' \"$0\" >{}\nexit 0\n",
        argv0_out.display()
    );
    f.write_all(body.as_bytes()).expect("write stub");
    let mut perm = f.metadata().expect("perm").permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&path, perm).expect("chmod");
    let tool = td.path().join("yt-dlp");
    std::fs::write(&tool, b"#!/bin/sh\nexit 0\n").expect("tool stub");
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let mut opts = DebugOptions::new(path.clone());
    opts.path_override = Some(td.path().display().to_string());
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await
    .expect("download stub runs");
    let argv0 = std::fs::read_to_string(&argv0_out).expect("argv0 recorded");
    assert_ne!(
        std::path::PathBuf::from(argv0.trim()),
        path,
        "the classified snapshot must execute, not the live install path"
    );
}

/// Temp filesystems may be mounted noexec: the snapshot must be
/// READ by the shell, never exec(2)'d, or every download on such
/// systems dies with EACCES. The observable proxy is that the
/// executed snapshot carries no exec permission at all.
#[cfg(unix)]
#[tokio::test]
async fn spawn_download_snapshot_needs_no_exec_permission() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let _guard = ENV_LOCK.lock().await;
    let td = tempfile::tempdir().expect("tempdir");
    let mode_out = td.path().join("mode");
    let path = td.path().join("ani-cli");
    let mut f = std::fs::File::create(&path).expect("create stub");
    let body = format!(
        "#!/bin/sh\nversion_number=\"4.15.0\"\ncase \"$player_function\" in\n    download)\n        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'\n        dep_ch \"aria2c\"\n        ;;\nesac\nif [ -x \"$0\" ]; then printf executable >{out}; else printf plain >{out}; fi\nexit 0\n",
        out = mode_out.display()
    );
    f.write_all(body.as_bytes()).expect("write stub");
    let mut perm = f.metadata().expect("perm").permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&path, perm).expect("chmod");
    let tool = td.path().join("yt-dlp");
    std::fs::write(&tool, b"#!/bin/sh\nexit 0\n").expect("tool stub");
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let mut opts = DebugOptions::new(path);
    opts.path_override = Some(td.path().display().to_string());
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await
    .expect("download stub runs");
    let mode = std::fs::read_to_string(&mode_out).expect("mode recorded");
    assert_eq!(
        mode, "plain",
        "the snapshot must run without exec permission (noexec-immune)"
    );
}

/// The complete yt-dlp-only path Codex asked to pin: active
/// script read from disk, capability classified, composed PATH
/// scanned — end to end through spawn_download.
#[cfg(unix)]
#[tokio::test]
async fn spawn_download_accepts_ytdlp_only_with_a_capable_script() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, opts) = stub_ani_cli_with_tools(true, &["yt-dlp"]);
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    let r = spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await;
    assert!(r.is_ok(), "yt-dlp alone must satisfy a 4.15 script: {r:?}");
}

/// yt-dlp exits 0 after writing a file it could not repackage, and
/// says so only on stderr:
///
///     WARNING: Possible MPEG-TS in MP4 container or malformed AAC
///     timestamps. Install ffmpeg to fix this automatically
///
/// Measured against a real TS-segment stream with ffmpeg off PATH: the
/// output is raw MPEG-TS (first byte 0x47) carrying an .mp4 name. It
/// plays in mpv and VLC, which sniff content, and fails anywhere that
/// trusts the extension.
///
/// Nothing reads that line today, so the download reports success and
/// the user keeps a file that is not what it claims. The exit code
/// cannot catch it — it is zero — and neither can a one-time check of
/// what the provider serves, because that can change under us. The
/// tool reporting the condition as it happens is the only signal that
/// stays true.
///
/// `FfmpegMissing` is the honest classification: ffmpeg was needed and
/// absent, and the modal's install instructions are already the right
/// advice.
#[cfg(unix)]
#[tokio::test]
async fn spawn_download_fails_when_yt_dlp_could_not_repackage() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, opts) = stub_ani_cli_with_tools(true, &["yt-dlp"]);
    // Rewrite the stub to emit yt-dlp's real warning and exit 0.
    std::fs::write(
        &opts.ani_cli_path,
        // Keeps the recognized 4.15 arm, or the PREFLIGHT rejects
        // before anything spawns and the test passes for the wrong
        // reason — it did exactly that on the first run.
        "#!/bin/sh\nversion_number=\"4.15.0\"\ncase \"$player_function\" in\n    download)\n        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'\n        dep_ch \"aria2c\"\n        ;;\nesac\nprintf '%s\\n' 'WARNING: Show: Possible MPEG-TS in MP4 container or malformed AAC timestamps. Install ffmpeg to fix this automatically' >&2\nexit 0\n",
    )
    .expect("rewrite stub");
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    let r = spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await;
    assert!(
        matches!(r, Err(AniError::FfmpegMissing)),
        "an unrepackaged download must not report success: {r:?}"
    );
}

/// The mirror: an ordinary successful download says nothing about
/// fixups, and must stay successful. A guard that fires on any stderr
/// chatter would break every download.
#[cfg(unix)]
#[tokio::test]
async fn spawn_download_stays_successful_without_the_fixup_warning() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, opts) = stub_ani_cli_with_tools(true, &["yt-dlp"]);
    std::fs::write(
        &opts.ani_cli_path,
        "#!/bin/sh\nversion_number=\"4.15.0\"\ncase \"$player_function\" in\n    download)\n        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'\n        dep_ch \"aria2c\"\n        ;;\nesac\nprintf '%s\\n' '[download] 100% of 58.20KiB in 00:00:00' >&2\nexit 0\n",
    )
    .expect("rewrite stub");
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    let r = spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await;
    assert!(r.is_ok(), "a clean download must stay successful: {r:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn spawn_download_rejects_ytdlp_only_against_a_pre_4_15_script() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, opts) = stub_ani_cli_with_tools(false, &["yt-dlp"]);
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    let r = spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await;
    assert!(
        matches!(r, Err(crate::error::AniError::FfmpegMissing)),
        "a script without the failover must still demand ffmpeg: {r:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn spawn_download_passes_d_flag_and_episode_query_quality() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, stub) = stub_ani_cli_echo();
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    let captured: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    let cap = captured.clone();

    let r = spawn_download(
        &debug_opts_dl(stub),
        &DownloadRequest {
            query: "Naruto Shippuuden",
            episode: "5",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        move |line| cap.lock().expect("lock").push(line.to_string()),
    )
    .await;
    assert!(r.is_ok(), "spawn_download failed: {r:?}");

    let lines = captured.lock().expect("lock").clone();
    let argv: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("argv:"))
        .collect();
    assert!(argv.contains(&"-d"), "argv: {argv:?}");
    assert!(
        argv.windows(2).any(|w| w == ["-e", "5"]),
        "argv missing -e 5: {argv:?}"
    );
    assert!(
        argv.windows(2).any(|w| w == ["-q", "1080"]),
        "argv missing -q 1080: {argv:?}"
    );
    // The query is positional, after the `--` separator.
    assert!(
        argv.contains(&"Naruto Shippuuden"),
        "argv missing query token: {argv:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn spawn_download_includes_dub_flag_when_mode_dub() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, stub) = stub_ani_cli_echo();
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    let captured: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    let cap = captured.clone();
    spawn_download(
        &debug_opts_dl(stub),
        &DownloadRequest {
            query: "any",
            episode: "1",
            quality: "best",
            mode: "dub",
            select_index: 1,
        },
        dl_dir.path(),
        move |line| cap.lock().expect("lock").push(line.to_string()),
    )
    .await
    .expect("ok");
    let lines = captured.lock().expect("lock").clone();
    let argv: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("argv:"))
        .collect();
    assert!(argv.contains(&"--dub"), "argv missing --dub: {argv:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn spawn_download_sets_ani_cli_download_dir_env() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, stub) = stub_ani_cli_echo();
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    let dl_path = dl_dir.path().to_path_buf();
    let captured: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    let cap = captured.clone();
    spawn_download(
        &debug_opts_dl(stub),
        &DownloadRequest {
            query: "any",
            episode: "1",
            quality: "best",
            mode: "sub",
            select_index: 1,
        },
        &dl_path,
        move |line| cap.lock().expect("lock").push(line.to_string()),
    )
    .await
    .expect("ok");
    let lines = captured.lock().expect("lock").clone();
    let env_line = lines
        .iter()
        .find(|l| l.starts_with("env:ANI_CLI_DOWNLOAD_DIR="))
        .expect("env line emitted");
    let value = env_line
        .trim_start_matches("env:ANI_CLI_DOWNLOAD_DIR=")
        .to_string();
    assert_eq!(
        value,
        dl_path.to_str().expect("utf-8 path"),
        "expected ANI_CLI_DOWNLOAD_DIR to point at the chosen dir"
    );
}

/// The snapshot is READ by an interpreter rather than exec(2)'d,
/// so the kernel never consults its shebang — this code has to.
/// A cached or user-customized script declaring `bash` and using
/// bash-only syntax would break under a hard-coded `/bin/sh`,
/// and only for downloads: search and playback route through the
/// ordinary command builder and keep working, which makes the
/// failure look like a download bug rather than an interpreter
/// mismatch.
#[test]
fn snapshot_interpreter_honors_the_declared_shebang() {
    assert_eq!(
        snapshot_interpreter("#!/bin/bash\nmain\n"),
        vec!["/bin/bash".to_string()],
        "a bash shebang must run under bash"
    );
    assert_eq!(
        snapshot_interpreter("#!/usr/bin/env bash\nmain\n"),
        vec!["/usr/bin/env".to_string(), "bash".to_string()],
        "the env form carries its interpreter argument"
    );
    assert_eq!(
        snapshot_interpreter("#!/bin/sh\nmain\n"),
        vec!["/bin/sh".to_string()],
        "the ordinary case is unchanged"
    );
    assert_eq!(
        snapshot_interpreter("#! /bin/dash \nmain\n"),
        vec!["/bin/dash".to_string()],
        "shebang spacing is not part of the path"
    );
}

/// The tail is ONE argument, exactly as the kernel passes it.
/// `fs/binfmt_script.c` takes the first blank-delimited word as
/// the interpreter and hands everything after it over as a single
/// argument — it does no tokenizing, and neither may we.
///
/// `#!/usr/bin/env -S bash -O 'extglob'` is the case that makes
/// the difference visible. Under the kernel, `env` receives the
/// one string `-S bash -O 'extglob'` and its own `--split-string`
/// handling strips the quotes, enabling extglob. Splitting here
/// instead would hand `env` four arguments with the quotes still
/// attached, and bash would reject `'extglob'` as an option name
/// — a script that runs fine for search and playback failing on
/// downloads alone.
#[test]
fn the_shebang_tail_is_a_single_argument() {
    assert_eq!(
        snapshot_interpreter("#!/usr/bin/env -S bash -O 'extglob'\nmain\n"),
        vec![
            "/usr/bin/env".to_string(),
            "-S bash -O 'extglob'".to_string()
        ],
        "the tail keeps its own spacing and quoting"
    );
    assert_eq!(
        snapshot_interpreter("#!/bin/bash   -eu   -o pipefail  \nmain\n"),
        vec!["/bin/bash".to_string(), "-eu   -o pipefail".to_string()],
        "interior spacing is the argument's; the outer blanks are not"
    );
}

/// Anything that isn't a well-formed absolute-path shebang falls
/// back to `/bin/sh` — the previous behaviour. A relative or bare
/// interpreter name is refused rather than resolved: that would
/// spawn through PATH/CWD lookup on a string taken from a file
/// the auto-updater rewrites, which is a worse failure than
/// running a `sh`-compatible script under `sh`.
#[test]
fn snapshot_interpreter_falls_back_to_bin_sh() {
    for (label, contents) in [
        ("no shebang", "main\n"),
        ("empty file", ""),
        ("not a shebang", "# ani-cli\n#!/bin/bash\n"),
        ("relative path", "#!bash\nmain\n"),
        ("bare marker", "#!\nmain\n"),
        ("only spaces", "#!   \nmain\n"),
    ] {
        assert_eq!(
            snapshot_interpreter(contents),
            vec!["/bin/sh".to_string()],
            "{label} must fall back to /bin/sh"
        );
    }
}

proptest::proptest! {
    /// An absolute shebang round-trips: the interpreter, then the
    /// tail verbatim as ONE argument. The tail is generated with
    /// its own interior spacing and quotes so the property fails
    /// on any tokenizing — which is the whole difference between
    /// this and the kernel.
    #[test]
    fn an_absolute_shebang_round_trips(
        dirs in proptest::collection::vec("[a-z][a-z0-9_.-]{0,7}", 1..4),
        tail in "[a-z0-9_'\"-]([a-z0-9_'\" \t-]{0,20}[a-z0-9_'\"-])?",
        has_tail in proptest::bool::ANY,
        lead in "[ \t]{0,3}",
        gap in "[ \t]{1,3}",
        trail in "[ \t]{0,3}",
        rest in "[a-z \n]{0,30}",
    ) {
        let interp = format!("/{}", dirs.join("/"));
        let (line, want) = if has_tail {
            (
                format!("#!{lead}{interp}{gap}{tail}{trail}"),
                vec![interp, tail],
            )
        } else {
            (format!("#!{lead}{interp}{trail}"), vec![interp])
        };
        proptest::prop_assert_eq!(snapshot_interpreter(&format!("{line}\n{rest}")), want);
    }

    /// The kernel passes at most one optional argument, so the
    /// argv is never longer than two however many words the tail
    /// contains. Stated separately because it is the invariant a
    /// future edit is most likely to break by "improving" the
    /// parsing.
    #[test]
    fn the_argv_is_never_longer_than_two(contents in "(?s).{0,80}") {
        proptest::prop_assert!(snapshot_interpreter(&contents).len() <= 2);
    }

    /// The invariant the spawn depends on, over ANY input: the
    /// argv is never empty and its program is always absolute.
    /// `download_command` hands element 0 to `Command::new`, so a
    /// relative program would be resolved through PATH or the
    /// working directory — driven by a string in a file the
    /// auto-updater rewrites.
    #[test]
    fn the_interpreter_is_always_an_absolute_program(contents in "(?s).{0,80}") {
        let argv = snapshot_interpreter(&contents);
        proptest::prop_assert!(!argv.is_empty());
        proptest::prop_assert!(
            argv[0].starts_with('/'),
            "program {:?} would be resolved through PATH",
            argv[0]
        );
    }

    /// No `#!` marker at all: whatever the first line says, it is
    /// script text rather than a declaration, and the fallback is
    /// the whole answer.
    #[test]
    fn a_file_without_a_shebang_falls_back(first in "[^#\n][^\n]{0,30}", rest in "[a-z \n]{0,30}") {
        proptest::prop_assert_eq!(
            snapshot_interpreter(&format!("{first}\n{rest}")),
            vec!["/bin/sh".to_string()]
        );
    }

    /// A shebang naming something that is not an absolute path —
    /// a bare `bash`, a `./local` — is refused rather than
    /// resolved. This is the direction that matters: resolving it
    /// is what turns a rewritten script into a chosen program.
    #[test]
    fn a_non_absolute_shebang_is_refused(
        name in "[a-z.][a-z0-9_./-]{0,12}",
        rest in "[a-z \n]{0,30}",
    ) {
        proptest::prop_assume!(!name.starts_with('/'));
        proptest::prop_assert_eq!(
            snapshot_interpreter(&format!("#!{name}\n{rest}")),
            vec!["/bin/sh".to_string()]
        );
    }
}

/// A script that loads a sibling relative to itself — the shape
/// `. "$(dirname "$0")/helpers.sh"` — breaks if the snapshot is
/// staged in an unrelated directory, and breaks on downloads
/// ONLY, since search and playback still execute the live path.
/// Staging beside the live script keeps `$0`'s directory correct.
#[test]
fn a_snapshot_is_staged_beside_the_live_script() {
    let td = tempfile::tempdir().expect("tempdir");
    let live = td.path().join("ani-cli");
    std::fs::write(&live, b"#!/bin/sh\nexit 0\n").expect("live script");
    let staged = stage_script_snapshot("#!/bin/sh\nexit 0\n", &live).expect("stage");
    assert_eq!(
        staged.path().parent(),
        Some(td.path()),
        "the snapshot must sit in the live script's directory"
    );
    assert_ne!(
        staged.path(),
        live,
        "and must not overwrite the live script"
    );
    let path = staged.path().to_path_buf();
    drop(staged);
    assert!(
        !path.exists(),
        "the staged copy is removed when it is dropped"
    );
}

/// A packaged install can have its script in a read-only
/// directory, so staging beside it has to degrade rather than
/// fail the download. The temp-dir copy loses `$0`-relative
/// resource loading, which is the pre-existing behaviour.
// Unix-only: the scenario IS a POSIX permission bit. Windows
// expresses directory write-protection through ACLs, which
// `Permissions::from_mode` cannot express — the previous version
// of this test failed to compile there rather than being skipped.
#[cfg(unix)]
#[test]
fn a_read_only_script_directory_falls_back_to_a_temp_dir() {
    use std::os::unix::fs::PermissionsExt;
    let td = tempfile::tempdir().expect("tempdir");
    let dir = td.path().join("ro");
    std::fs::create_dir(&dir).expect("mkdir");
    let live = dir.join("ani-cli");
    std::fs::write(&live, b"#!/bin/sh\nexit 0\n").expect("live script");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).expect("chmod");

    // The mode bits are the whole premise, and root ignores them —
    // `cargo test` as UID 0 is ordinary inside a dev container. Ask
    // the filesystem whether the permission actually bites rather
    // than asking who we are: a uid check would also have to reason
    // about capabilities, and this answers the question directly.
    let enforced = std::fs::File::create(dir.join(".probe")).is_err();
    if !enforced {
        let _ = std::fs::remove_file(dir.join(".probe"));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        eprintln!("skipped: 0555 does not block writes for this user (root?)");
        return;
    }

    let staged = stage_script_snapshot("#!/bin/sh\nexit 0\n", &live).expect("stage");
    assert_ne!(
        staged.path().parent(),
        Some(dir.as_path()),
        "a read-only directory must not block the download"
    );
    assert!(
        staged.path().exists(),
        "the fallback copy is still readable"
    );

    // Restore write permission so the tempdir can clean up.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// Two concurrent downloads share a process id, so a pid-tagged
/// name is not unique. They would write the same file: the second
/// can overwrite the first between its classification and its
/// interpreter opening it, so one download classifies one script
/// and executes another — and whichever finishes first unlinks
/// the path the other is still using.
#[test]
fn concurrent_snapshots_do_not_share_a_path() {
    let td = tempfile::tempdir().expect("tempdir");
    let live = td.path().join("ani-cli");
    std::fs::write(&live, b"#!/bin/sh\nexit 0\n").expect("live script");

    let first = stage_script_snapshot("#!/bin/sh\necho first\n", &live).expect("first");
    let second = stage_script_snapshot("#!/bin/sh\necho second\n", &live).expect("second");
    assert_ne!(
        first.path(),
        second.path(),
        "each download needs its own snapshot path"
    );

    // Neither may have clobbered the other's contents.
    let a = std::fs::read_to_string(first.path()).expect("read first");
    let b = std::fs::read_to_string(second.path()).expect("read second");
    assert!(a.contains("first"), "first snapshot kept its own bytes");
    assert!(b.contains("second"), "second snapshot kept its own bytes");

    // Dropping one must not remove the other.
    let surviving = second.path().to_path_buf();
    drop(first);
    assert!(
        surviving.exists(),
        "one download finishing must not unlink another's snapshot"
    );
}

proptest::proptest! {
    /// The classifier keys on a marker inside a stream, so the
    /// property that matters is insertion: the sentence classifies
    /// wherever it sits, under any amount of surrounding download
    /// chatter, and with any video title in yt-dlp's prefix — that
    /// prefix is arbitrary user content, which is why the matcher
    /// does not anchor on the whole line.
    #[test]
    fn the_repackage_warning_classifies_wherever_it_sits(
        before in proptest::collection::vec("[a-z0-9 ]{0,30}", 0..5),
        after in proptest::collection::vec("[a-z0-9 ]{0,30}", 0..5),
        title in "[a-zA-Z0-9 :!'-]{0,40}",
    ) {
        let line = format!(
            "WARNING: {title}: Possible MPEG-TS in MP4 container or \
             malformed AAC timestamps. Install ffmpeg to fix this automatically"
        );
        let stderr = format!("{}\n{line}\n{}", before.join("\n"), after.join("\n"));
        proptest::prop_assert!(yt_dlp_could_not_repackage(&stderr));
    }

    /// Removal is the other half: ordinary download chatter never
    /// classifies, or every download would fail. The filler is
    /// lowercase, digits and punctuation only — it cannot spell the
    /// marker, which carries capitals and a hyphen.
    #[test]
    fn ordinary_download_chatter_never_classifies(
        lines in proptest::collection::vec(r"[a-z0-9 .%/:\[\]-]{0,40}", 0..8),
    ) {
        proptest::prop_assert!(!yt_dlp_could_not_repackage(&lines.join("\n")));
    }

    /// The trailing advice is NOT part of the match. yt-dlp has
    /// reworded it across releases; pinning the whole sentence would
    /// turn a future release into a silent regression — the download
    /// would go back to reporting success on a file it could not
    /// repackage.
    #[test]
    fn a_reworded_tail_still_classifies(tail in "[a-zA-Z0-9 .,'-]{0,60}") {
        let line = format!("WARNING: Show: Possible MPEG-TS in MP4 container {tail}");
        proptest::prop_assert!(yt_dlp_could_not_repackage(&line));
    }

    /// The marker only means anything on the line yt-dlp reports it
    /// on. Everywhere else it is just text, and the text on those
    /// lines is not ours: destinations and progress carry the show
    /// title, which is whatever the provider named the entry. A show
    /// called after the warning would otherwise fail every one of its
    /// own downloads and have the finished file deleted underneath it.
    #[test]
    fn the_marker_in_ordinary_output_never_classifies(
        prefix in r"\[(download|info|hlsnative|generic)\] [A-Za-z: ]{0,20}",
        suffix in "[a-zA-Z0-9 .,'-]{0,40}",
    ) {
        let line = format!("{prefix}Possible MPEG-TS in MP4 container{suffix}");
        proptest::prop_assert!(
            !yt_dlp_could_not_repackage(&line),
            "classified a non-warning line: {line}"
        );
    }
}

/// A show whose title happens to contain yt-dlp's warning sentence.
/// Absurd as a title, ordinary as an input — the name comes from the
/// provider, and the download path echoes it into progress output
/// verbatim. Nothing about that run went wrong.
#[test]
fn a_title_echoed_into_progress_output_is_not_a_fixup_warning() {
    let stderr = concat!(
        "[download] Destination: /home/u/Possible MPEG-TS in MP4 container - 01.mp4\n",
        "[download] 100% of 58.20KiB in 00:00:01 at 41.02KiB/s\n"
    );
    assert!(
        !yt_dlp_could_not_repackage(stderr),
        "a successful download must not be re-read as a failed repackage"
    );
}

/// When the guard fires, the mislabeled file yt-dlp already wrote is
/// still on disk under an `.mp4` name. Reporting failure and leaving
/// it there is the worst of both: the user is told the download
/// failed and still finds something that looks like the episode.
///
/// Deleting is bounded two ways so it can never eat a good file. Only
/// the paths yt-dlp announced as this run's output are considered, and
/// of those only the ones whose own first byte says MPEG-TS. A file
/// this download wrote correctly fails the second test; anything it
/// never claimed fails the first.
#[cfg(unix)]
#[tokio::test]
async fn a_failed_repackage_removes_only_the_files_it_produced() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, opts) = stub_ani_cli_with_tools(true, &["yt-dlp"]);
    let dl_dir = tempfile::tempdir().expect("dl tempdir");

    // Pre-existing: a real MP4 the user already had, and a stray TS
    // from some earlier run. Neither appeared during THIS download.
    let untouched_mp4 = dl_dir.path().join("Old Episode 1.mp4");
    std::fs::write(&untouched_mp4, b"\x00\x00\x00\x20ftypisom").expect("write");
    let untouched_ts = dl_dir.path().join("Old Episode 2.mp4");
    std::fs::write(&untouched_ts, b"\x47\x40\x11\x10").expect("write");

    // The stub announces and writes one good file and one mislabeled
    // one — both claimed by this run, so only the content check
    // separates them — then emits yt-dlp's warning and exits 0.
    std::fs::write(
        &opts.ani_cli_path,
        "#!/bin/sh\nversion_number=\"4.15.0\"\ncase \"$player_function\" in\n    download)\n        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'\n        dep_ch \"aria2c\"\n        ;;\nesac\nprintf '%s\\n' \"[download] Destination: $ANI_CLI_DOWNLOAD_DIR/New Good.mp4\"\nprintf '\\0\\0\\0 ftypisom' >\"$ANI_CLI_DOWNLOAD_DIR/New Good.mp4\"\nprintf '%s\\n' \"[download] Destination: $ANI_CLI_DOWNLOAD_DIR/New Bad.mp4\"\nprintf '\\107\\100\\21\\20' >\"$ANI_CLI_DOWNLOAD_DIR/New Bad.mp4\"\nprintf '%s\\n' 'WARNING: Show: Possible MPEG-TS in MP4 container or malformed AAC timestamps. Install ffmpeg to fix this automatically' >&2\nexit 0\n",
    )
    .expect("rewrite stub");

    let r = spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await;
    assert!(matches!(r, Err(AniError::FfmpegMissing)), "got {r:?}");

    assert!(
        !dl_dir.path().join("New Bad.mp4").exists(),
        "the mislabeled file this download produced must be removed"
    );
    assert!(
        dl_dir.path().join("New Good.mp4").exists(),
        "a real MP4 from the same run is not mislabeled and stays"
    );
    assert!(
        untouched_mp4.exists(),
        "a pre-existing MP4 is never touched"
    );
    assert!(
        untouched_ts.exists(),
        "a pre-existing stray is not ours to delete"
    );
}

/// "Appeared while this download ran" is not the same as "belongs to
/// this download". The dock lets several downloads run at once and
/// they all land in one directory, so a sibling's half-written file
/// is new, is an `.mp4`, and — mid-transfer, before its own repackage
/// — opens on the MPEG-TS sync byte. Every test this cleanup applies
/// says delete, and the sibling is still writing to it.
///
/// What bounds the deletion is yt-dlp naming its own output. It
/// prints `[download] Destination: <path>` for the file it is about
/// to write; nothing else this process wrote is ours to touch.
#[cfg(unix)]
#[tokio::test]
async fn a_failed_repackage_leaves_a_concurrent_downloads_file_alone() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, opts) = stub_ani_cli_with_tools(true, &["yt-dlp"]);
    let dl_dir = tempfile::tempdir().expect("dl tempdir");

    // The stub plays both parts: it announces and writes its own
    // output the way yt-dlp does, and it drops an unannounced
    // TS-headed file to stand in for the sibling transfer that is
    // still running in the same directory.
    std::fs::write(
        &opts.ani_cli_path,
        "#!/bin/sh\nversion_number=\"4.15.0\"\ncase \"$player_function\" in\n    download)\n        dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'\n        dep_ch \"aria2c\"\n        ;;\nesac\nprintf '%s\\n' \"[download] Destination: $ANI_CLI_DOWNLOAD_DIR/Mine.mp4\"\nprintf '\\107\\100\\21\\20' >\"$ANI_CLI_DOWNLOAD_DIR/Mine.mp4\"\nprintf '\\107\\100\\21\\20' >\"$ANI_CLI_DOWNLOAD_DIR/Someone Elses.mp4\"\nprintf '%s\\n' 'WARNING: Show: Possible MPEG-TS in MP4 container or malformed AAC timestamps. Install ffmpeg to fix this automatically' >&2\nexit 0\n",
    )
    .expect("rewrite stub");

    let r = spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await;
    assert!(matches!(r, Err(AniError::FfmpegMissing)), "got {r:?}");

    assert!(
        !dl_dir.path().join("Mine.mp4").exists(),
        "the file this download announced and mislabeled must be removed"
    );
    assert!(
        dl_dir.path().join("Someone Elses.mp4").exists(),
        "another download's in-flight file is not ours to delete"
    );
}

/// The spawn scrubs PATH down to the stub directory, so a stub that
/// has to still be running when the guard fires cannot reach the
/// system `sleep` — it exits immediately with `sleep: not found` and
/// the test measures nothing. Link the real one in beside the tool
/// stubs rather than widening PATH, which would let a real ffmpeg on
/// the developer's machine change what the preflight sees.
#[cfg(unix)]
fn link_sleep_beside(opts: &DebugOptions) {
    let dir = opts.ani_cli_path.parent().expect("stub dir");
    let real = find_in_path("sleep").expect("system sleep on the test process PATH");
    std::os::unix::fs::symlink(real, dir.join("sleep")).expect("link sleep");
}

/// The stub body shared by the two range tests. `$tail` runs after the
/// warning, standing in for ani-cli's `for i in $range` loop carrying
/// on to the next episode.
///
/// The destination goes out from a subshell so its stdio buffer is
/// flushed by that process exiting. A `printf` builtin writing to a
/// pipe is block-buffered, and these stubs are killed mid-run — real
/// yt-dlp flushes progress as it downloads, so buffering here would be
/// an artifact of the stub, not of what is being tested.
#[cfg(unix)]
fn ranged_repackage_stub(tail: &str) -> String {
    format!(
        "#!/bin/sh\n\
         version_number=\"4.15.0\"\n\
         case \"$player_function\" in\n\
         \x20   download)\n\
         \x20       dep_ch_failover \"yt-dlp,ffmpeg\" >/dev/null || die 'Neither yt-dlp nor ffmpeg found'\n\
         \x20       dep_ch \"aria2c\"\n\
         \x20       ;;\n\
         esac\n\
         (printf '%s\\n' \"[download] Destination: $ANI_CLI_DOWNLOAD_DIR/Episode 1.mp4\")\n\
         printf '\\107\\100\\21\\20' >\"$ANI_CLI_DOWNLOAD_DIR/Episode 1.mp4\"\n\
         printf '%s\\n' 'WARNING: Show: Possible MPEG-TS in MP4 container or malformed AAC timestamps. Install ffmpeg to fix this automatically' >&2\n\
         {tail}\n"
    )
}

/// A range download (`-e 1-12`) is ONE ani-cli process looping
/// `for i in $range; do play_episode; done` (ani-cli:498), and the
/// dock offers exactly that — the Range picker defaults to 1..count.
///
/// Reading the warning only after the child exits means episode 1's
/// failure is not acted on until episode 12 has also been fetched: the
/// whole range is downloaded as unusable MPEG-TS, then deleted, and
/// the user waits out the entire transfer to be told it failed. The
/// condition does not improve on its own — the next episode goes down
/// the same yt-dlp-without-ffmpeg path.
///
/// The marker file is deliberately not an `.mp4`: cleanup only removes
/// announced output, so a marker that survived would otherwise be
/// deleted and the test would pass for the wrong reason.
#[cfg(unix)]
#[tokio::test]
async fn a_repackage_warning_stops_the_rest_of_the_range() {
    let _guard = ENV_LOCK.lock().await;
    let (_td, opts) = stub_ani_cli_with_tools(true, &["yt-dlp"]);
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    std::fs::write(
        &opts.ani_cli_path,
        ranged_repackage_stub("sleep 3\n: >\"$ANI_CLI_DOWNLOAD_DIR/episode-2-started\"\nexit 0"),
    )
    .expect("rewrite stub");
    link_sleep_beside(&opts);

    let r = spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1-12",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await;

    assert!(matches!(r, Err(AniError::FfmpegMissing)), "got {r:?}");
    assert!(
        !dl_dir.path().join("episode-2-started").exists(),
        "the range must stop at the first warning, not fetch the rest of the season first"
    );
    assert!(
        !dl_dir.path().join("Episode 1.mp4").exists(),
        "and the episode that did fail must still be cleaned up"
    );
}

/// The other half of the same gap. Cleanup lives after the timeout
/// `?`, so a run that is still going when the wall clock runs out
/// returns `Timeout` and leaves the mislabeled file on disk — the
/// exact state the cleanup exists to prevent, reachable by waiting.
///
/// Stopping the child on the warning normally closes that window, so
/// the no-op tree-kill probe is what makes this reachable at all: it
/// stands in for a kill that did not take. The classification is the
/// point either way — ffmpeg was missing, and saying "timed out"
/// sends the user to look at their connection.
#[cfg(unix)]
#[tokio::test]
async fn a_timed_out_run_still_discards_what_the_warning_condemned() {
    let _guard = ENV_LOCK.lock().await;
    let probe_td = tempfile::tempdir().expect("probe tempdir");
    let probe = probe_td.path().join("noop-kill.sh");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(&probe, b"#!/bin/sh\nexit 0\n").expect("write probe");
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    *TREE_KILL_PROBE.lock().expect("probe lock") = Some(probe);

    let (_td, mut opts) = stub_ani_cli_with_tools(true, &["yt-dlp"]);
    opts.timeout = std::time::Duration::from_millis(1_500);
    let dl_dir = tempfile::tempdir().expect("dl tempdir");
    std::fs::write(
        &opts.ani_cli_path,
        ranged_repackage_stub("sleep 30\nexit 0"),
    )
    .expect("rewrite stub");
    link_sleep_beside(&opts);

    let r = spawn_download(
        &opts,
        &DownloadRequest {
            query: "Some Show",
            episode: "1-12",
            quality: "1080",
            mode: "sub",
            select_index: 1,
        },
        dl_dir.path(),
        |_line| {},
    )
    .await;
    *TREE_KILL_PROBE.lock().expect("probe lock") = None;

    assert!(
        matches!(r, Err(AniError::FfmpegMissing)),
        "a run the warning already condemned is a missing-ffmpeg failure, not a timeout: {r:?}"
    );
    assert!(
        !dl_dir.path().join("Episode 1.mp4").exists(),
        "running out of clock must not strand the mislabeled file"
    );
}

proptest::proptest! {
    /// The manifest parser decides which files cleanup may delete, so
    /// what it accepts is a safety boundary, not a convenience. The
    /// round trip is the whole contract: whatever path yt-dlp names,
    /// that exact path comes back.
    ///
    /// Titles are the interesting input here — they come from the
    /// provider and carry spaces, colons, quotes and brackets, and
    /// `[download] Destination: ` is stripped by prefix rather than by
    /// splitting on `:`, so a colon inside the name must survive.
    ///
    /// Generated names deliberately do not END in whitespace: the
    /// parser trims the tail to shed the `\r` of a CRLF line, which
    /// also costs a trailing space. Real output has neither.
    #[test]
    fn an_announced_path_round_trips(
        dir in "/[a-z]{1,8}(/[a-z]{1,8}){0,3}",
        name in r#"[a-zA-Z0-9 :!'\[\]().-]{0,40}[a-zA-Z0-9)\]]"#,
    ) {
        let want = format!("{dir}/{name}.mp4");
        let got = yt_dlp_destination(&format!("[download] Destination: {want}"));
        proptest::prop_assert_eq!(got, Some(std::path::PathBuf::from(want)));
    }

    /// Whichever way the line arrives — indented by the shell, or
    /// carrying the `\r` a CRLF pipe leaves behind — it names the same
    /// file. Cleanup keys on the path, so a padded line that parsed to
    /// a DIFFERENT path would silently protect the mislabeled file.
    #[test]
    fn surrounding_whitespace_does_not_change_the_path(
        lead in "[ \t]{0,4}",
        trail in "[ \t\r]{0,4}",
    ) {
        let bare = yt_dlp_destination("[download] Destination: /d/Show - 01.mp4");
        let padded = yt_dlp_destination(
            &format!("{lead}[download] Destination: /d/Show - 01.mp4{trail}")
        );
        proptest::prop_assert_eq!(padded, bare);
    }

    /// Removal is the half that matters for safety. Everything yt-dlp
    /// prints that is NOT a destination has to come back `None`, or
    /// cleanup would be handed paths this download never claimed —
    /// which is exactly the bound that keeps it off a concurrent
    /// download's files.
    ///
    /// The generator covers yt-dlp's other line shapes and free text.
    /// The filter excludes only the announcement form itself — not
    /// every line mentioning a destination — so a parser that searched
    /// the line instead of matching its prefix would still be caught
    /// here rather than assumed away.
    #[test]
    fn output_that_is_not_a_destination_never_parses(
        line in r"(\[(download|info|hlsnative|generic)\] )?[a-zA-Z0-9 %/:.,~-]{0,60}",
    ) {
        proptest::prop_assume!(!line.trim().starts_with("[download] Destination: "));
        proptest::prop_assert_eq!(yt_dlp_destination(&line), None);
    }

    /// The tag is part of the marker. yt-dlp prints progress under
    /// several of them and only `[download]` announces output, so a
    /// parser loose about the tag would hand cleanup paths from lines
    /// that promise nothing about what was written.
    #[test]
    fn a_destination_under_another_tag_never_parses(
        tag in r"\[(info|hlsnative|generic|Merger|ExtractAudio)\]",
        path in "/[a-z]{1,10}/[a-z]{1,10}\\.mp4",
    ) {
        let line = format!("{tag} Destination: {path}");
        proptest::prop_assert_eq!(yt_dlp_destination(&line), None, "parsed {}", line);
    }
}

/// The prefix is matched whole, space included. yt-dlp has printed it
/// this way for as long as the download path has existed, and an
/// approximate match is the wrong trade here: parsing a line that is
/// not a destination hands cleanup a path to delete.
#[test]
fn a_near_miss_prefix_is_not_a_destination() {
    // No space after the colon.
    assert_eq!(
        yt_dlp_destination("[download] Destination:/d/Show.mp4"),
        None
    );
    // Different tag.
    assert_eq!(yt_dlp_destination("[info] Destination: /d/Show.mp4"), None);
    // The word alone, mid-sentence.
    assert_eq!(
        yt_dlp_destination("[download] Writing to Destination: /d/Show.mp4"),
        None
    );
}

/// A destination whose own name contains the marker still resolves to
/// itself — the same class of input as the show titled after yt-dlp's
/// fixup warning, and the reason the prefix is stripped once rather
/// than searched for.
#[test]
fn a_path_containing_the_marker_resolves_to_itself() {
    let line = "[download] Destination: /d/[download] Destination: weird.mp4";
    assert_eq!(
        yt_dlp_destination(line),
        Some(std::path::PathBuf::from(
            "/d/[download] Destination: weird.mp4"
        ))
    );
}
